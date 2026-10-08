use cosmwasm_std::{MemoryStorage, Order, Storage};
use cw_commonware::{
    state::{self, Any, ProtoHeight, WasmClientState, WasmConsensusState},
    types::{ClientState, ConsensusState, EpochKey},
    Error,
};
use prost::Message;
use serde_json::{json, Value};

fn client() -> ClientState {
    ClientState {
        router: [0x11; 20].into(),
        namespace: vec![0xab, 0xcd].into(),
        epoch_length: 64,
        latest_height: 65,
        frozen: false,
        keys: vec![EpochKey {
            epoch: 1,
            key: [0x33; 96].into(),
        }],
    }
}

fn wrap(type_url: &str, value: Vec<u8>) -> Vec<u8> {
    Any {
        type_url: type_url.into(),
        value,
    }
    .encode_to_vec()
}

#[test]
fn client_updates_preserve_or_replace_checksum() {
    let mut storage = MemoryStorage::default();
    let mut cs = client();
    let original_checksum = vec![1, 0, 128, 255, 19];
    state::save_client_state(&mut storage, &cs, Some(original_checksum.clone())).unwrap();
    cs.latest_height = 129;
    cs.frozen = true;
    cs.keys[0].epoch = 2;

    for (checksum, expected) in [
        (None, original_checksum),
        (Some(vec![9, 8, 7]), vec![9, 8, 7]),
    ] {
        state::save_client_state(&mut storage, &cs, checksum).unwrap();
        let raw = storage.get(b"clientState").unwrap();
        let any = Any::decode(raw.as_slice()).unwrap();
        assert_eq!(any.type_url, "/ibc.lightclients.wasm.v1.ClientState");
        let wasm = WasmClientState::decode(any.value.as_slice()).unwrap();
        assert_eq!(wasm.checksum, expected);
        assert_eq!(
            wasm.latest_height,
            Some(ProtoHeight {
                revision_number: 0,
                revision_height: 129,
            })
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&wasm.data).unwrap(),
            json!({
                "router": format!("0x{}", "11".repeat(20)),
                "namespace": "0xabcd",
                "epoch_length": 64,
                "latest_height": 129,
                "frozen": true,
                "keys": [{"epoch": 2, "key": format!("0x{}", "33".repeat(96))}],
            })
        );
        assert_eq!(state::client_state(&storage).unwrap(), cs);
    }
}

#[test]
fn checksum_reuse_requires_an_existing_decodable_wrapper() {
    for original in [
        None,
        Some(vec![0x80]), // Truncated protobuf varint in the outer Any.
        Some(wrap("/ibc.lightclients.wasm.v1.ClientState", vec![0x80])),
    ] {
        let mut storage = MemoryStorage::default();
        storage.set(b"sentinel", b"untouched");
        if let Some(raw) = &original {
            storage.set(b"clientState", raw);
        }
        let before: Vec<_> = storage.range(None, None, Order::Ascending).collect();
        let read_error = state::client_state(&storage).unwrap_err();
        let write_error = state::save_client_state(&mut storage, &client(), None).unwrap_err();
        if original.is_none() {
            assert_eq!(read_error, Error::ClientStateNotFound);
            assert_eq!(write_error, Error::ClientStateNotFound);
        } else {
            assert!(matches!(read_error, Error::Decode(_)));
            assert!(matches!(write_error, Error::Decode(_)));
        }
        assert_eq!(
            storage
                .range(None, None, Order::Ascending)
                .collect::<Vec<_>>(),
            before
        );
    }
}

#[test]
fn malformed_client_json_does_not_prevent_checksum_reuse() {
    let mut storage = MemoryStorage::default();
    let raw = wrap(
        "/ibc.lightclients.wasm.v1.ClientState",
        WasmClientState {
            data: b"{".to_vec(),
            checksum: vec![3, 1, 4],
            latest_height: None,
        }
        .encode_to_vec(),
    );
    storage.set(b"clientState", &raw);
    assert!(matches!(
        state::client_state(&storage),
        Err(Error::Decode(_))
    ));

    state::save_client_state(&mut storage, &client(), None).unwrap();
    assert_eq!(state::client_state(&storage).unwrap(), client());
    let raw = storage.get(b"clientState").unwrap();
    let any = Any::decode(raw.as_slice()).unwrap();
    let wasm = WasmClientState::decode(any.value.as_slice()).unwrap();
    assert_eq!(wasm.checksum, vec![3, 1, 4]);
}

#[test]
fn consensus_reads_distinguish_absence_from_corruption() {
    let mut storage = MemoryStorage::default();
    assert_eq!(state::consensus_state(&storage, 7).unwrap(), None);
    for raw in [
        vec![0x80],
        wrap("/ibc.lightclients.wasm.v1.ConsensusState", vec![0x80]),
        wrap(
            "/ibc.lightclients.wasm.v1.ConsensusState",
            WasmConsensusState {
                data: b"{".to_vec(),
            }
            .encode_to_vec(),
        ),
    ] {
        storage.set(b"consensusStates/0-7", &raw);
        assert!(matches!(
            state::consensus_state(&storage, 7),
            Err(Error::Decode(_))
        ));
    }
}

#[test]
fn consensus_boundary_heights_have_distinct_ibc_go_keys() {
    let mut storage = MemoryStorage::default();
    let cases = [
        (
            0,
            b"consensusStates/0-0".as_slice(),
            ConsensusState {
                timestamp: 17,
                state_root: [0x44; 32].into(),
            },
        ),
        (
            u64::MAX,
            b"consensusStates/0-18446744073709551615".as_slice(),
            ConsensusState {
                timestamp: 29,
                state_root: [0x55; 32].into(),
            },
        ),
    ];
    for (height, _, cs) in &cases {
        state::save_consensus_state(&mut storage, *height, cs).unwrap();
    }
    for (height, key, cs) in cases {
        assert_eq!(state::consensus_key(height), key);
        let raw = storage.get(key).unwrap();
        let any = Any::decode(raw.as_slice()).unwrap();
        assert_eq!(any.type_url, "/ibc.lightclients.wasm.v1.ConsensusState");
        let wasm = WasmConsensusState::decode(any.value.as_slice()).unwrap();
        assert_eq!(
            serde_json::from_slice::<ConsensusState>(&wasm.data).unwrap(),
            cs
        );
        assert_eq!(state::consensus_state(&storage, height).unwrap(), Some(cs));
    }
}
