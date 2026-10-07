use cw_commonware::{cert::Finalization, verify::finalize_message};

#[test]
fn encodes_finalization_message() {
    let fin = Finalization {
        epoch: 1,
        view: 300,
        parent: 299,
        payload: *b"0123456789abcdef0123456789abcdef",
        vote_signature: [0x42; 48],
    };

    assert_eq!(
        finalize_message(b"TEMPO", &fin),
        b"\x0eTEMPO_FINALIZE\x01\xac\x02\xab\x02\
          0123456789abcdef0123456789abcdef"
    );
}

#[test]
fn namespace_length_includes_suffix_at_varint_boundary() {
    let fin = Finalization {
        epoch: 0,
        view: 128,
        parent: 127,
        payload: *b"0123456789abcdef0123456789abcdef",
        vote_signature: [0x42; 48],
    };

    // Adding the nine-byte suffix makes these namespace lengths 127 and 128.
    assert_eq!(
        finalize_message(&[b'x'; 118], &fin),
        b"\x7f\
          xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
          xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
          xxxxxxxxxxxxxxxxxx_FINALIZE\x00\x80\x01\x7f\
          0123456789abcdef0123456789abcdef"
    );
    assert_eq!(
        finalize_message(&[b'x'; 119], &fin),
        b"\x80\x01\
          xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
          xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
          xxxxxxxxxxxxxxxxxxx_FINALIZE\x00\x80\x01\x7f\
          0123456789abcdef0123456789abcdef"
    );
}

#[test]
fn encodes_empty_namespace_and_maximum_u64_fields() {
    let fin = Finalization {
        epoch: u64::MAX,
        view: u64::MAX,
        parent: u64::MAX,
        payload: *b"0123456789abcdef0123456789abcdef",
        vote_signature: [0x42; 48],
    };

    assert_eq!(
        finalize_message(b"", &fin),
        b"\x09_FINALIZE\
          \xff\xff\xff\xff\xff\xff\xff\xff\xff\x01\
          \xff\xff\xff\xff\xff\xff\xff\xff\xff\x01\
          \xff\xff\xff\xff\xff\xff\xff\xff\xff\x01\
          0123456789abcdef0123456789abcdef"
    );
}
