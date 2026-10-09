use cw_commonware::{
    verify::{dkg_outcome_identity, G2_COMPRESSED},
    Error,
};
use serde_json::Value;

// These are parser-only extraction prefixes, not complete DKG outcomes or signed headers.
fn extraction_prefix(epoch: &[u8], coeffs: &[u8]) -> (Vec<u8>, [u8; G2_COMPRESSED]) {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../contracts/test/fixtures/lc.json")).unwrap();
    let decode =
        |name: &str| hex::decode(fixture[name].as_str().unwrap().trim_start_matches("0x")).unwrap();
    let outcome = decode("dkgOutcome");
    let identity: [u8; G2_COMPRESSED] = decode("epoch1KeyCompressed").try_into().unwrap();
    assert_eq!(outcome[0], 0x01, "fixture epoch must occupy one byte");

    let mut prefix = epoch.to_vec();
    // Reuse the fixture's opaque summary[32], mode u8, and total u32 BE.
    prefix.extend_from_slice(&outcome[1..1 + 32 + 1 + 4]);
    prefix.extend_from_slice(coeffs);
    prefix.extend_from_slice(&identity);
    (prefix, identity)
}

#[test]
fn locates_identity_after_variable_length_fields() {
    let epochs: &[(&[u8], u64)] = &[
        (&[0x7f], 127),
        (&[0x80, 0x01], 128),
        (
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01],
            u64::MAX,
        ),
    ];
    let coeff_counts: &[&[u8]] = &[&[0x01], &[0x7f], &[0x80, 0x01]];
    for &(epoch_wire, epoch) in epochs {
        for &coeff_wire in coeff_counts {
            let (prefix, identity) = extraction_prefix(epoch_wire, coeff_wire);
            assert_eq!(
                dkg_outcome_identity(&prefix),
                Ok((epoch, identity)),
                "epoch {epoch_wire:02x?}, coefficient count {coeff_wire:02x?}"
            );
        }
    }
}

#[test]
fn rejects_every_truncated_extraction_prefix() {
    // Both variable-length fields span two bytes, so the loop also cuts inside each varint.
    let (prefix, identity) = extraction_prefix(&[0x80, 0x01], &[0x80, 0x01]);
    assert_eq!(dkg_outcome_identity(&prefix), Ok((128, identity)));
    for len in 0..prefix.len() {
        assert_eq!(
            dkg_outcome_identity(&prefix[..len]),
            Err(Error::MalformedDkgOutcome),
            "truncated at {len} of {} bytes",
            prefix.len()
        );
    }
}

#[test]
fn rejects_zero_coefficients_even_with_a_complete_identity() {
    let (prefix, _) = extraction_prefix(&[0x01], &[0x00]);
    assert_eq!(
        dkg_outcome_identity(&prefix),
        Err(Error::MalformedDkgOutcome)
    );
}

#[test]
fn malformed_varints_report_a_dkg_error() {
    // Generic varint validity is covered in cert.rs; check mapping at both DKG field positions.
    let cases: &[(&[u8], &[u8])] = &[(&[0x81, 0x00], &[0x01]), (&[0x01], &[0x81, 0x00])];
    for &(epoch, coeffs) in cases {
        let (prefix, _) = extraction_prefix(epoch, coeffs);
        assert_eq!(
            dkg_outcome_identity(&prefix),
            Err(Error::MalformedDkgOutcome)
        );
    }
}

#[test]
fn extraction_stops_after_the_first_coefficient() {
    // TempoHeaderLib.readDkgOutcome intentionally skips the remaining outcome fields.
    // Even with three declared coefficients, this extractor only requires the first one.
    let (mut prefix, identity) = extraction_prefix(&[0x01], &[0x03]);
    assert_eq!(dkg_outcome_identity(&prefix), Ok((1, identity)));
    prefix.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(dkg_outcome_identity(&prefix), Ok((1, identity)));
}
