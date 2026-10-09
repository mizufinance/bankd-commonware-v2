use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::{MnemonicBuilder, PrivateKeySigner},
};
use alloy_network::{ReceiptResponse, TransactionBuilder};
use alloy_primitives::Bytes;
use alloy_rpc_types_eth::TransactionRequest;
use std::env;
use tempo_alloy::rpc::TempoTransactionReceipt;
use tempo_contracts::precompiles::{IFeeManager, ITIP20};
use tempo_precompiles::{PATH_USD_ADDRESS, TIP_FEE_MANAGER_ADDRESS};
use tempo_revm::handler::FEE_ESCROW_ADDRESS;

use crate::utils::TestNodeBuilder;

#[tokio::test(flavor = "multi_thread")]
async fn test_fee_in_native_brl() -> eyre::Result<()> {
    reth_tracing::init_test_tracing();

    let source = if let Ok(rpc_url) = env::var("RPC_URL") {
        crate::utils::NodeSource::ExternalRpc(rpc_url.parse()?)
    } else {
        crate::utils::NodeSource::LocalNode(include_str!("../assets/test-genesis.json").to_string())
    };
    let (http_url, _local_node) = crate::utils::setup_test_node(source).await?;

    let wallet = MnemonicBuilder::from_phrase(crate::utils::TEST_MNEMONIC).build()?;
    let caller = wallet.address();
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(http_url);

    // Bankd's genesis funds native BRL for gas.
    let balance = provider.get_account_info(caller).await?.balance;
    assert!(balance > U256::ZERO);
    let escrow_before = provider.get_balance(FEE_ESCROW_ADDRESS).await?;

    let fee_manager = IFeeManager::new(TIP_FEE_MANAGER_ADDRESS, provider.clone());
    let fee_token_address = fee_manager.userTokens(caller).call().await?;

    // Get the balance of the fee token before the tx
    let fee_token = ITIP20::new(fee_token_address, provider.clone());
    let initial_balance = fee_token.balanceOf(caller).call().await?;

    let tx = TransactionRequest::default().from(caller).to(caller);

    let pending_tx = provider.send_transaction(tx).await?;
    let tx_hash = pending_tx.watch().await?;
    let receipt = provider
        .raw_request::<_, TempoTransactionReceipt>("eth_getTransactionReceipt".into(), (tx_hash,))
        .await?;

    // The configured TIP-20 fee token is untouched; gas goes to the native fee escrow.
    let balance_after = fee_token.balanceOf(caller).call().await?;

    let cost = U256::from(receipt.gas_used) * U256::from(receipt.effective_gas_price());
    assert!(cost > U256::ZERO);
    assert_eq!(balance_after, initial_balance);
    assert_eq!(provider.get_balance(caller).await?, balance - cost);
    assert_eq!(
        provider.get_balance(FEE_ESCROW_ADDRESS).await?,
        escrow_before + cost
    );

    assert!(receipt.status());
    assert!(receipt.logs().is_empty());
    assert_eq!(receipt.fee_token, None);

    Ok(())
}

#[test_case::test_case(false ; "default_builder")]
#[test_case::test_case(true ; "legacy_parallel_request")]
#[tokio::test(flavor = "multi_thread")]
async fn test_native_fee_without_token_preference(parallel: bool) -> eyre::Result<()> {
    reth_tracing::init_test_tracing();

    let setup = TestNodeBuilder::new()
        .with_parallel_builder(parallel)
        .build_http_only()
        .await?;
    let http_url = setup.http_url;

    let wallet = MnemonicBuilder::from_phrase(crate::utils::TEST_MNEMONIC).build()?;
    let caller = wallet.address();
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(http_url.clone());

    // Create new random wallet
    let new_wallet = PrivateKeySigner::random();
    let new_address = new_wallet.address();

    // Transfer pathUSD to the new wallet
    let path_usd = ITIP20::new(PATH_USD_ADDRESS, provider.clone());
    let transfer_amount = U256::from(1_000_000u64);
    path_usd
        .transfer(new_address, transfer_amount)
        .send()
        .await?
        .get_receipt()
        .await?;

    // TIP-20 tokens cannot fund Bankd gas; give the new account native BRL too.
    let native_funding = U256::from(1_000_000_000_000_000_000u64);
    provider
        .send_transaction(
            TransactionRequest::default()
                .to(new_address)
                .value(native_funding),
        )
        .await?
        .get_receipt()
        .await?;

    // Create provider with the new wallet
    let new_provider = ProviderBuilder::new()
        .wallet(new_wallet)
        .connect_http(http_url);

    // Native gas works without setting a TIP-20 fee-token preference.
    let balance = new_provider.get_account_info(new_address).await?.balance;
    assert_eq!(balance, native_funding);

    // Ensure the fee token is not set for the user
    let fee_manager = IFeeManager::new(TIP_FEE_MANAGER_ADDRESS, provider.clone());
    let fee_token_address = fee_manager.userTokens(new_address).call().await?;
    assert_eq!(fee_token_address, Address::ZERO);

    // Get the balance of the fee token before the tx
    let initial_balance = path_usd.balanceOf(new_address).call().await?;

    let tx = TransactionRequest::default().from(new_address).to(caller);
    let pending_tx = new_provider.send_transaction(tx).await?;
    let tx_hash = pending_tx.watch().await?;
    let receipt = new_provider
        .raw_request::<_, TempoTransactionReceipt>("eth_getTransactionReceipt".into(), (tx_hash,))
        .await?;

    // Gas is deducted only from the native balance.
    let balance_after = path_usd.balanceOf(new_address).call().await?;
    let cost = U256::from(receipt.gas_used) * U256::from(receipt.effective_gas_price());
    assert!(cost > U256::ZERO);
    assert_eq!(balance_after, initial_balance);
    assert_eq!(new_provider.get_balance(new_address).await?, balance - cost);

    assert!(receipt.status());
    assert!(receipt.logs().is_empty());
    assert_eq!(receipt.fee_token, None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_failed_transaction_charges_native_brl() -> eyre::Result<()> {
    reth_tracing::init_test_tracing();

    let source = if let Ok(rpc_url) = env::var("RPC_URL") {
        crate::utils::NodeSource::ExternalRpc(rpc_url.parse()?)
    } else {
        crate::utils::NodeSource::LocalNode(include_str!("../assets/test-genesis.json").to_string())
    };
    let (http_url, _local_node) = crate::utils::setup_test_node(source).await?;

    let wallet = MnemonicBuilder::from_phrase(crate::utils::TEST_MNEMONIC).build()?;
    let caller = wallet.address();
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(http_url);

    // Failed execution still pays gas in native BRL.
    let balance = provider.get_account_info(caller).await?.balance;
    assert!(balance > U256::ZERO);
    let escrow_before = provider.get_balance(FEE_ESCROW_ADDRESS).await?;

    let fee_manager = IFeeManager::new(TIP_FEE_MANAGER_ADDRESS, provider.clone());
    let fee_token_address = fee_manager.userTokens(caller).call().await?;

    // Get the balance of the fee token before the tx
    let fee_token = ITIP20::new(fee_token_address, provider.clone());
    let initial_balance = fee_token.balanceOf(caller).call().await?;

    let tx = TransactionRequest::default()
        .into_create()
        .input(Bytes::from_static(&[0xef]).into())
        .gas_limit(1_000_000);
    let pending_tx = provider.send_transaction(tx).await?;
    let tx_hash = pending_tx.watch().await?;
    let receipt = provider
        .raw_request::<_, TempoTransactionReceipt>("eth_getTransactionReceipt".into(), (tx_hash,))
        .await?;

    // A failed create charges native gas without emitting a TIP-20 fee transfer.
    let balance_after = fee_token.balanceOf(caller).call().await?;

    let cost = U256::from(receipt.gas_used) * U256::from(receipt.effective_gas_price());
    assert!(cost > U256::ZERO);
    assert_eq!(balance_after, initial_balance);
    assert_eq!(provider.get_balance(caller).await?, balance - cost);
    assert_eq!(
        provider.get_balance(FEE_ESCROW_ADDRESS).await?,
        escrow_before + cost
    );

    assert!(!receipt.status());
    assert!(receipt.logs().is_empty());
    assert_eq!(receipt.fee_token, None);

    Ok(())
}
