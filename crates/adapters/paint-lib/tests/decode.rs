use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_paint::{DecodeLimits, Diagnostic, decode_layers};
use sha2::{Digest, Sha256};
use std::cell::Cell;
const SAMPLE: &[u8] = include_bytes!("fixtures/mspaintlib-sample.paint");
fn decode(bytes: &[u8]) -> paintcraft_core::types::Result<paintcraft_paint::Decoded<'_>> {
    decode_layers(bytes, &DecodeLimits::default(), &|| Ok(()))
}
#[test]
fn real_pinned_sample_matches_independent_python_zlib_pixel_hashes() {
    assert_eq!(
        paintcraft_core::types::policy::hex(&Sha256::digest(SAMPLE)),
        "a3e947fee374cca80e0e648152667109ac61b57808c9673eb806f52865f141f6"
    );
    let decoded = decode(SAMPLE).unwrap();
    assert_eq!((decoded.width, decoded.height), (333, 271));
    assert_eq!(decoded.canvas_fill, [0x6b00, 0x6b00, 0x6b00, 0xffff]);
    assert_eq!(decoded.diagnostics, [Diagnostic::LayerPropertiesUnverified]);
    assert_eq!(decoded.layers.len(), 4);
    for (index, hash) in [
        "e288e2acca6bf956be0e9425825db07c2a61f1454fa0c598a478c6a31c64d91e",
        "3144e443e13acabb1e3f314ee5f35bf584e529396df8c8ad947566c72ae78b1c",
        "3038d999fb638535386716b61ffd61ae4f8976e887aab3a44b5e99586771dc12",
        "057c6a5d7ad1fd8c7ae44caf4fe4ee75725e49c6f46ad77394ee8f05df8ef1de",
    ]
    .into_iter()
    .enumerate()
    {
        let layer = &decoded.layers[index];
        assert_eq!(layer.item_id, index as u32 + 1);
        assert_eq!(layer.item_name, "");
        assert_eq!(
            (layer.width, layer.height, layer.origin),
            (333, 271, (0, 0))
        );
        assert_eq!(layer.rgba.len(), 360972);
        assert_eq!(
            paintcraft_core::types::policy::hex(&Sha256::digest(&layer.rgba)),
            hash
        );
    }
}
fn changed(offset: usize, bytes: &[u8]) -> Vec<u8> {
    let mut output = SAMPLE.to_vec();
    output[offset..offset + bytes.len()].copy_from_slice(bytes);
    output
}
#[test]
fn malformed_item_graphs_and_unqualified_properties_never_return_partial_layers() {
    for (offset, value) in [
        (138, 1u16.to_be_bytes().to_vec()), // duplicate infe ID
        (230, 1u16.to_be_bytes().to_vec()), // external data reference
        (228, 2u16.to_be_bytes().to_vec()), // unsupported construction method
        (560, 5u16.to_be_bytes().to_vec()), // cyclic dimg input
        (562, 1u16.to_be_bytes().to_vec()), // duplicate dimg input
        (498, vec![0xff]),                  // invalid essential property association
        (586, 1u16.to_be_bytes().to_vec()), // dimension/overlay mismatch
        (410, b"xxxx".to_vec()),            // unqualified uncC profile
    ] {
        assert!(
            decode(&changed(offset, &value)).is_err(),
            "accepted mutation at {offset}"
        );
    }
    let mut unknown = SAMPLE.to_vec();
    unknown[326..330].copy_from_slice(b"myst");
    assert_eq!(
        decode(&unknown).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn compressed_lengths_aliasing_and_truncations_are_rejected() {
    // First iloc extent offset/length are file-relative and never allowed to point
    // into metadata, beyond mdat, or alias another item's compressed stream.
    for (offset, value) in [
        (234, 32u32),
        (234, 76534),
        (238, 0),
        (238, 1),
        (250, 678),
        (238, 26122),
    ] {
        assert!(
            decode(&changed(offset, &value.to_be_bytes())).is_err(),
            "accepted mutation at {offset}"
        );
    }
    for end in (0..678).chain([679, 1000, SAMPLE.len() - 1]) {
        assert!(
            decode(&SAMPLE[..end]).is_err(),
            "accepted truncation at {end}"
        );
    }
    let mut corruption = SAMPLE.to_vec();
    corruption[678] = 0xff;
    assert!(decode(&corruption).is_err());
}
#[test]
fn source_graph_geometry_and_aggregate_decode_budgets_are_independent() {
    for limits in [
        DecodeLimits {
            items: 4,
            ..Default::default()
        },
        DecodeLimits {
            layers: 3,
            ..Default::default()
        },
        DecodeLimits {
            dimension: 332,
            ..Default::default()
        },
        DecodeLimits {
            pixels: 333 * 271 * 4 - 1,
            ..Default::default()
        },
        DecodeLimits {
            decoded_bytes: 333 * 271 * 4 * 4 - 1,
            ..Default::default()
        },
        DecodeLimits {
            container: paintcraft_paint::Limits {
                metadata_bytes: 1024,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        assert_eq!(
            decode_layers(SAMPLE, &limits, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}
#[test]
fn cancellation_occurs_during_graph_pixel_and_swizzle_work() {
    let total = Cell::new(0usize);
    decode_layers(SAMPLE, &DecodeLimits::default(), &|| {
        total.set(total.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(total.get() > 50);
    for stop in [1, 20, total.get() / 2, total.get() - 1, total.get()] {
        let calls = Cell::new(0usize);
        let result = decode_layers(SAMPLE, &DecodeLimits::default(), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "injected"))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "checkpoint {stop}"
        );
    }
}

#[test]
fn component_declarations_control_swizzle_and_reject_packing_guesses() {
    let original = decode(SAMPLE).unwrap();
    let mut bytes = SAMPLE.to_vec();
    // uncC's component indexes become R,G,B,A while compressed samples remain BGRA.
    for (index, offset) in [418, 423, 428, 433].into_iter().enumerate() {
        bytes[offset..offset + 2].copy_from_slice(&(index as u16).to_be_bytes());
    }
    let alternate = decode(&bytes).unwrap();
    for (source, changed) in original.layers.iter().zip(&alternate.layers) {
        for (left, right) in source
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(changed.rgba.as_chunks::<4>().0)
        {
            assert_eq!(*right, [left[2], left[1], left[0], left[3]]);
        }
    }
    for (offset, value) in [
        (418, vec![0, 1]),
        (420, vec![15]),
        (438, vec![1]),
        (439, vec![0]),
        (441, vec![1]),
        (446, vec![0, 0, 0, 4]),
    ] {
        assert!(
            decode(&changed(offset, &value)).is_err(),
            "unsupported packing at {offset}"
        );
    }
}
