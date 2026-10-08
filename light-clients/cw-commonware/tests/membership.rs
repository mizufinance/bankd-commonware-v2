//! Membership proof bindings using the Solidity client's real account and storage proofs.

use alloy_primitives::{Address, Bytes, B256};
use cw_commonware::{
    membership::{account_from_proof, verify},
    types::MembershipProof,
    Error,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    router: Address,
    updates: Vec<Update>,
    commitment_path: Bytes,
    absent_path: Bytes,
    commitment: B256,
    account_proof: Vec<Bytes>,
    storage_proof: Vec<Bytes>,
    absent_storage_proof: Vec<Bytes>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Update {
    state_root: B256,
}

impl Fixture {
    fn load() -> Self {
        serde_json::from_str(include_str!("../../../contracts/test/fixtures/lc.json")).unwrap()
    }

    fn proof(&self, storage_proof: &[Bytes]) -> MembershipProof {
        MembershipProof {
            account: account_from_proof(&self.account_proof).unwrap(),
            account_proof: self.account_proof.clone(),
            storage_proof: storage_proof.to_vec(),
        }
    }
}

#[track_caller]
fn assert_invalid_proof(result: Result<(), Error>, prefix: &str) {
    assert!(
        matches!(&result, Err(Error::InvalidProof(reason)) if reason.starts_with(prefix)),
        "{result:?}"
    );
}

#[test]
fn membership_and_non_membership_authenticate_account() {
    let f = Fixture::load();
    let state_root = f.updates[0].state_root;
    for (path, nodes, value) in [
        (
            &f.commitment_path,
            &f.storage_proof,
            Some(f.commitment.as_slice()),
        ),
        (&f.absent_path, &f.absent_storage_proof, None),
    ] {
        let path = [path.to_vec()];
        let proof = f.proof(nodes);
        verify(state_root, f.router, &path, &proof, value).unwrap();

        let mut wrong_root = state_root;
        wrong_root[0] ^= 1;
        assert_invalid_proof(
            verify(wrong_root, f.router, &path, &proof, value),
            "account: ",
        );

        let mut wrong_router = f.router;
        wrong_router.0[0] ^= 1;
        assert_invalid_proof(
            verify(state_root, wrong_router, &path, &proof, value),
            "account: ",
        );

        // The final byte is inside the fixture account's 32-byte code hash. Changing
        // it preserves the RLP structure and storage root but changes the full leaf.
        let mut account = proof.account.to_vec();
        *account.last_mut().unwrap() ^= 1;
        let mut wrong_account = proof.clone();
        wrong_account.account = account.into();
        assert_invalid_proof(
            verify(state_root, f.router, &path, &wrong_account, value),
            "account: ",
        );
    }
}

#[test]
fn membership_binds_the_commitment_path() {
    let f = Fixture::load();
    let state_root = f.updates[0].state_root;
    let proof = f.proof(&f.storage_proof);
    let value = Some(f.commitment.as_slice());
    assert_ne!(f.commitment, B256::ZERO);
    verify(
        state_root,
        f.router,
        &[f.commitment_path.to_vec()],
        &proof,
        value,
    )
    .unwrap();

    assert_invalid_proof(
        verify(
            state_root,
            f.router,
            &[f.absent_path.to_vec()],
            &proof,
            value,
        ),
        "storage: ",
    );
}

#[test]
fn membership_requires_a_32_byte_commitment() {
    let f = Fixture::load();
    let state_root = f.updates[0].state_root;
    let proof = f.proof(&f.storage_proof);
    let path = [f.commitment_path.to_vec()];
    verify(
        state_root,
        f.router,
        &path,
        &proof,
        Some(f.commitment.as_slice()),
    )
    .unwrap();

    for len in [0, 1, 31, 33] {
        let mut value = f.commitment.to_vec();
        value.resize(len, 0xab);
        assert_eq!(
            verify(state_root, f.router, &path, &proof, Some(&value)),
            Err(Error::InvalidValueLength),
            "commitment length {len}"
        );
    }
}

#[test]
fn membership_and_non_membership_require_complete_proofs() {
    let f = Fixture::load();
    let state_root = f.updates[0].state_root;
    for (path, nodes, value) in [
        (
            &f.commitment_path,
            &f.storage_proof,
            Some(f.commitment.as_slice()),
        ),
        (&f.absent_path, &f.absent_storage_proof, None),
    ] {
        let path = [path.to_vec()];
        let proof = f.proof(nodes);
        verify(state_root, f.router, &path, &proof, value).unwrap();
        assert!(proof.account_proof.len() > 1);
        assert!(proof.storage_proof.len() > 1);

        // Check both an empty proof and a proof missing only its final node.
        for retained in [0, proof.account_proof.len() - 1] {
            let mut incomplete = proof.clone();
            incomplete.account_proof.truncate(retained);
            assert_invalid_proof(
                verify(state_root, f.router, &path, &incomplete, value),
                "account: ",
            );
        }
        for retained in [0, proof.storage_proof.len() - 1] {
            let mut incomplete = proof.clone();
            incomplete.storage_proof.truncate(retained);
            assert_invalid_proof(
                verify(state_root, f.router, &path, &incomplete, value),
                "storage: ",
            );
        }
    }
}
