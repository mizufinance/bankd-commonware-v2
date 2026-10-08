//! Decoder-only mutations of real headers; modified headers are not re-signed.

use alloy_rlp::{encode, Header, PayloadView};
use cw_commonware::{
    header::{ConsensusContext, TempoHeader},
    Error,
};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../contracts/test/fixtures/lc.json")).unwrap()
}

fn hex_bytes(value: &Value) -> Vec<u8> {
    hex::decode(value.as_str().unwrap().trim_start_matches("0x")).unwrap()
}

fn list_items(mut encoded: &[u8]) -> Vec<Vec<u8>> {
    let PayloadView::List(items) = Header::decode_raw(&mut encoded).unwrap() else {
        panic!("expected a fixture list");
    };
    assert!(encoded.is_empty());
    items.into_iter().map(<[u8]>::to_vec).collect()
}

// Items are already RLP-encoded, so only the surrounding list needs encoding.
fn encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let mut encoded = Vec::new();
    Header {
        list: true,
        payload_length: items.iter().map(Vec::len).sum(),
    }
    .encode(&mut encoded);
    encoded.extend(items.concat());
    encoded
}

#[track_caller]
fn assert_malformed(encoded: &[u8], reason: &'static str) {
    assert_eq!(
        TempoHeader::decode(encoded),
        Err(Error::MalformedHeader(reason))
    );
}

#[test]
fn decodes_fixture_headers_and_legacy_outer_lists() {
    let fixture = fixture();
    let epoch_length = fixture["epochLength"].as_u64().unwrap();
    for update in fixture["updates"].as_array().unwrap() {
        let encoded = hex_bytes(&update["headerRlp"]);
        let number = update["height"].as_u64().unwrap();
        let mut expected = TempoHeader {
            number,
            timestamp: update["timestamp"].as_u64().unwrap(),
            state_root: hex_bytes(&update["stateRoot"]).try_into().unwrap(),
            extra_data: if (number + 1) % epoch_length == 0 {
                hex_bytes(&fixture["dkgOutcome"])
            } else {
                Vec::new()
            },
            context: Some(ConsensusContext {
                epoch: update["epoch"].as_u64().unwrap(),
                view: update["view"].as_u64().unwrap(),
                parent_view: update["parentView"].as_u64().unwrap(),
            }),
        };
        assert_eq!(TempoHeader::decode(&encoded).unwrap(), expected);

        let outer = list_items(&encoded);
        expected.context = None;
        assert_eq!(
            TempoHeader::decode(&encode_list(&outer[..4])).unwrap(),
            expected
        );
    }
}

#[test]
fn rejects_malformed_containers() {
    let encoded = hex_bytes(&fixture()["updates"][0]["headerRlp"]);
    let outer = list_items(&encoded);
    assert_malformed(&encode_list(&outer[..3]), "outer list length");
    let mut oversized = outer.clone();
    oversized.push(encode(0u64));
    assert_malformed(&encode_list(&oversized), "outer list length");

    assert_malformed(&[0x80], "expected list");
    assert_malformed(&encoded[..encoded.len() - 1], "rlp");
    assert_malformed(&[encoded.as_slice(), &[0x80]].concat(), "expected list");
    // Complete outer list, but its single item is truncated.
    assert_malformed(&[0xc1, 0x81], "rlp item");

    for index in [3, 4] {
        let mut mutated = outer.clone();
        mutated[index] = encode(0u64);
        assert_malformed(&encode_list(&mutated), "expected list");
    }
    let inner = list_items(&outer[3]);
    for count in [3, 8, 11, 12] {
        let mut mutated = outer.clone();
        mutated[3] = encode_list(&inner[..count]);
        assert_malformed(&encode_list(&mutated), "inner list length");
    }
    let mut context = list_items(&outer[4]);
    context.push(encode(0u64));
    for count in [3, 5] {
        let mut mutated = outer.clone();
        mutated[4] = encode_list(&context[..count]);
        assert_malformed(&encode_list(&mutated), "context length");
    }
}

#[test]
fn rejects_invalid_consumed_fields_and_accepts_maximum_integers() {
    let encoded = hex_bytes(&fixture()["updates"][0]["headerRlp"]);
    let mut outer = list_items(&encoded);
    // Container 3 is the inner header; container 4 is the consensus context.
    for (container, field, item, reason) in [
        (3, 3, encode([0u8; 31]), "state root"),
        (3, 3, encode([0u8; 33]), "state root"),
        (3, 3, vec![0xc0], "rlp bytes"),
        (3, 12, vec![0xc0], "rlp bytes"),
        (3, 8, vec![0xc0], "rlp uint"),
        (3, 11, vec![0xc0], "rlp uint"),
        (4, 0, vec![0xc0], "rlp uint"),
        // Leading zeroes are invalid integers, even when the RLP string is canonical.
        (3, 8, vec![0x00], "rlp uint"),
        (3, 11, vec![0x82, 0x00, 0x01], "rlp uint"),
        (4, 1, vec![0x82, 0x00, 0x01], "rlp uint"),
        // A single byte below 0x80 must not have a string-length prefix.
        (3, 8, vec![0x81, 0x01], "rlp item"),
        (3, 11, vec![0x81, 0x01], "rlp item"),
        (4, 2, vec![0x81, 0x01], "rlp item"),
        (3, 8, encode(1u128 << 64), "rlp uint"),
        (3, 11, encode(1u128 << 64), "rlp uint"),
        (4, 0, encode(1u128 << 64), "rlp uint"),
    ] {
        let mut fields = list_items(&outer[container]);
        fields[field] = item;
        let mut mutated = outer.clone();
        mutated[container] = encode_list(&fields);
        assert_eq!(
            TempoHeader::decode(&encode_list(&mutated)),
            Err(Error::MalformedHeader(reason)),
            "container {container}, field {field}"
        );
    }

    let mut inner = list_items(&outer[3]);
    inner[8] = encode(u64::MAX);
    inner[11] = encode(u64::MAX);
    outer[3] = encode_list(&inner);
    let mut context = list_items(&outer[4]);
    for field in &mut context[..3] {
        *field = encode(u64::MAX);
    }
    outer[4] = encode_list(&context);
    let decoded = TempoHeader::decode(&encode_list(&outer)).unwrap();
    assert_eq!((decoded.number, decoded.timestamp), (u64::MAX, u64::MAX));
    assert_eq!(
        decoded.context,
        Some(ConsensusContext {
            epoch: u64::MAX,
            view: u64::MAX,
            parent_view: u64::MAX,
        })
    );
}
