use paintcraft_pdn::{DecodeLimits, Diagnostic, decode_layers};
use sha2::{Digest, Sha256};
const ONE: &[u8] = include_bytes!("fixtures/paintdotjs-1-layer.pdn");
const TWO: &[u8] = include_bytes!("fixtures/paintdotjs-2-layer.pdn");
#[test]
fn independent_writer_documents_decode_real_template_graphs_and_authored_pixels() {
    for (bytes, hash, count) in [
        (
            ONE,
            "ceb65149e16686fd3383102789e5bad15715275482b073bcb485c6c3d39b43d4",
            1,
        ),
        (
            TWO,
            "c30d43f95dfbcef153ae76f5577e71f8feffad61519cb67de298da4299ec7725",
            2,
        ),
    ] {
        assert_eq!(
            paintcraft_core::types::policy::hex(&Sha256::digest(bytes)),
            hash
        );
        let decoded = decode_layers(bytes, &DecodeLimits::default(), &|| Ok(())).unwrap();
        assert_eq!((decoded.width, decoded.height), (3, 2));
        assert_eq!(decoded.saved_with, [0, 0, 0, 0]);
        assert_eq!(decoded.header_version.as_deref(), Some("5.112.9563.32325"));
        assert_eq!(decoded.diagnostics, [Diagnostic::MetadataUnretained]);
        assert_eq!(decoded.layers.len(), count);
        assert_eq!(decoded.layers[0].name, "Base 日本語");
        assert!(decoded.layers[0].visible && decoded.layers[0].is_background);
        assert_eq!(decoded.layers[0].opacity, 201);
        assert_eq!(
            paintcraft_core::types::policy::hex(&Sha256::digest(&decoded.layers[0].rgba)),
            "d6931877bd07347276a7149f0ff06c97a16de2b64c416385f72e649facad7c56"
        );
        if count == 2 {
            assert_eq!(decoded.layers[1].name, "Overlay é");
            assert!(!decoded.layers[1].visible && !decoded.layers[1].is_background);
            assert_eq!(decoded.layers[1].opacity, 73);
            assert_eq!(
                paintcraft_core::types::policy::hex(&Sha256::digest(&decoded.layers[1].rgba)),
                "4f570b69e8ee688b9e255dab9f8842d18f632314066f17580b70460852dc50c8"
            );
        }
    }
}

fn decode(bytes: &[u8]) -> paintcraft_core::types::Result<paintcraft_pdn::Decoded<'_>> {
    decode_layers(bytes, &DecodeLimits::default(), &|| Ok(()))
}
fn chunks(bytes: &[u8], start: usize, count: usize) -> (Vec<std::ops::Range<usize>>, usize) {
    let mut position = start + 5;
    let mut ranges = Vec::new();
    for _ in 0..count {
        let length =
            u32::from_be_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
        ranges.push(position..position + 8 + length);
        position += 8 + length;
    }
    (ranges, position)
}
#[test]
fn deferred_chunks_use_declared_indexes_and_reject_duplicates_crc_and_tail_bytes() {
    const PIXELS: usize = 3168; // Pinned independent writer manifest, not a stream scan.
    let (records, end) = chunks(TWO, PIXELS, 3);
    let mut reordered = TWO[..PIXELS + 5].to_vec();
    for index in [2, 0, 1] {
        reordered.extend_from_slice(&TWO[records[index].clone()]);
    }
    reordered.extend_from_slice(&TWO[end..]);
    assert_eq!(decode(&reordered).unwrap(), decode(TWO).unwrap());
    for value in [0u32, 3] {
        let mut invalid = TWO.to_vec();
        invalid[records[1].start..records[1].start + 4].copy_from_slice(&value.to_be_bytes());
        assert!(decode(&invalid).is_err());
    }
    let mut corrupt = TWO.to_vec();
    corrupt[records[0].end - 8] ^= 1;
    assert!(decode(&corrupt).is_err());
    let mut corrupt = TWO.to_vec();
    corrupt[records[0].end - 1] ^= 1;
    assert!(decode(&corrupt).is_err());
    let mut tail = TWO.to_vec();
    tail.push(0);
    assert!(decode(&tail).is_err());
    let mut zero = TWO.to_vec();
    zero[PIXELS + 1..PIXELS + 5].copy_from_slice(&[0; 4]);
    assert!(decode(&zero).is_err());
}
#[test]
fn raw_deferred_chunks_preserve_straight_rgba_including_hidden_rgb() {
    const PIXELS: usize = 3168;
    let source = decode(TWO).unwrap();
    let mut raw = TWO[..PIXELS].to_vec();
    for layer in &source.layers {
        raw.push(1);
        raw.extend(8u32.to_be_bytes());
        for index in [2usize, 0, 1] {
            raw.extend((index as u32).to_be_bytes());
            raw.extend(8u32.to_be_bytes());
            for pixel in layer.rgba[index * 8..index * 8 + 8].as_chunks::<4>().0 {
                raw.extend([pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
    }
    assert_eq!(decode(&raw).unwrap(), source);
}
#[test]
fn xml_graph_disagreement_unqualified_types_blends_and_invalid_utf8_are_refused() {
    let mut dimensions = TWO.to_vec();
    let width = dimensions
        .windows(9)
        .position(|p| p == b"width=\"3\"")
        .unwrap();
    dimensions[width + 7] = b'4';
    assert!(decode(&dimensions).is_err());
    let mut unknown = TWO.to_vec();
    let name = unknown
        .windows(13)
        .position(|p| p == b"NormalBlendOp")
        .unwrap();
    unknown[name] = b'X';
    assert_eq!(
        decode(&unknown).unwrap_err().code,
        paintcraft_core::types::ErrorCode::CapabilityUnsupported
    );
    let mut blend = TWO.to_vec();
    let value = blend.windows(7).position(|p| p == b"value__").unwrap() + 7 + 2 + 4;
    assert_eq!(&blend[value..value + 4], &[0; 4]);
    blend[value..value + 4].copy_from_slice(&1i32.to_le_bytes());
    assert_eq!(
        decode(&blend).unwrap_err().code,
        paintcraft_core::types::ErrorCode::CapabilityUnsupported
    );
    // Same four bytes, but claiming UInt32 rather than Int32 is an unqualified
    // field layout, even when the numeric value happens to be zero.
    let mut declared_type = TWO.to_vec();
    let kind = declared_type
        .windows(7)
        .position(|p| p == b"value__")
        .unwrap()
        + 8;
    assert_eq!(declared_type[kind], 8);
    declared_type[kind] = 15;
    assert_eq!(
        decode(&declared_type).unwrap_err().code,
        paintcraft_core::types::ErrorCode::CapabilityUnsupported
    );

    let mut name = TWO.to_vec();
    let at = name
        .windows("Base 日本語".len())
        .position(|p| p == "Base 日本語".as_bytes())
        .unwrap();
    name[at] = 0xff;
    assert!(decode(&name).is_err());
}
#[test]
fn hostile_source_graph_and_aggregate_output_limits_fail_without_a_document() {
    use paintcraft_core::types::ErrorCode;
    for limits in [
        DecodeLimits {
            graph_bytes: 32,
            ..Default::default()
        },
        DecodeLimits {
            graph_metadata_bytes: 1,
            ..Default::default()
        },
        DecodeLimits {
            objects: 1,
            ..Default::default()
        },
        DecodeLimits {
            array_items: 0,
            ..Default::default()
        },
        DecodeLimits {
            members: 0,
            ..Default::default()
        },
        DecodeLimits {
            decoded_pixels: 11,
            ..Default::default()
        },
        DecodeLimits {
            decoded_bytes: 47,
            ..Default::default()
        },
        DecodeLimits {
            chunks: 2,
            ..Default::default()
        },
    ] {
        assert_eq!(
            decode_layers(TWO, &limits, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}
#[test]
fn every_file_prefix_is_rejected_and_cancellation_reaches_graph_and_pixel_work() {
    use paintcraft_core::types::{Error, ErrorCode};
    use std::cell::Cell;
    for end in 0..ONE.len() {
        assert!(decode(&ONE[..end]).is_err(), "accepted prefix {end}");
    }
    let total = Cell::new(0usize);
    decode_layers(TWO, &DecodeLimits::default(), &|| {
        total.set(total.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(total.get() > 100);
    for stop in [1, 50, total.get() / 2, total.get() - 1, total.get()] {
        let count = Cell::new(0usize);
        let result = decode_layers(TWO, &DecodeLimits::default(), &|| {
            count.set(count.get() + 1);
            if count.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "injected"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}
