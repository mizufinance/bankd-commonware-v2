use cw_commonware::{
    cert::{read_varint, write_varint, Finalization},
    Error,
};

#[test]
fn varints_match_canonical_wire_vectors() {
    let cases: &[(&[u8], u64)] = &[
        (&[0x00], 0),
        (&[0x01], 1),
        (&[0x7f], 127),
        (&[0x80, 0x01], 128),
        (&[0xff, 0x7f], 16_383),
        (&[0x80, 0x80, 0x01], 16_384),
        (
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
            1 << 63,
        ),
        (
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01],
            u64::MAX,
        ),
    ];
    for &(wire, value) in cases {
        let mut data = vec![0xaa];
        data.extend_from_slice(wire);
        data.extend_from_slice(&[0x55, 0xaa]);
        let mut off = 1;
        assert_eq!(read_varint(&data, &mut off), Some(value), "{wire:02x?}");
        assert_eq!(off, 1 + wire.len());
        assert_eq!(&data[off..], &[0x55, 0xaa]);

        let mut encoded = Vec::new();
        write_varint(value, &mut encoded);
        assert_eq!(encoded, wire);
    }
}

#[test]
fn rejects_nonminimal_truncated_and_overflowing_varints() {
    let cases: &[&[u8]] = &[
        &[],
        &[0x80],
        &[0x80; 9],
        &[0x80, 0x00],
        &[0x81, 0x00],
        &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00],
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02],
        &[
            0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00,
        ],
        &[
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x81, 0x01,
        ],
    ];
    for &wire in cases {
        assert_eq!(read_varint(wire, &mut 0), None, "{wire:02x?}");
    }
}

#[test]
fn malformed_varints_report_the_certificate_field() {
    let cases: &[(&[u8], &str)] = &[
        (&[0x80, 0x00], "epoch"),
        (&[0x00, 0x80, 0x00], "view"),
        (&[0x00, 0x00, 0x80, 0x00], "parent"),
    ];
    for &(prefix, field) in cases {
        let mut raw = prefix.to_vec();
        raw.extend_from_slice(&[0; 128]);
        assert_eq!(
            Finalization::decode(&raw),
            Err(Error::MalformedCertificate(field))
        );
    }
}

#[test]
fn certificate_requires_the_exact_payload_and_signature_tail() {
    let mut raw = vec![0x7f, 0x80, 0x01, 0x00];
    raw.extend_from_slice(&[0x11; 32]);
    raw.extend_from_slice(&[0x22; 48]);
    raw.extend_from_slice(&[0x33; 48]);
    assert_eq!(
        Finalization::decode(&raw),
        Ok(Finalization {
            epoch: 127,
            view: 128,
            parent: 0,
            payload: [0x11; 32],
            vote_signature: [0x22; 48],
        }),
    );
    for len in [4 + 31, 4 + 32 + 47, raw.len() - 1] {
        assert_eq!(
            Finalization::decode(&raw[..len]),
            Err(Error::MalformedCertificate("length"))
        );
    }
    raw.push(0);
    assert_eq!(
        Finalization::decode(&raw),
        Err(Error::MalformedCertificate("length"))
    );
}
