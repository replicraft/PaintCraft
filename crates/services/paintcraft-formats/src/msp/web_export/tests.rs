use super::*;
use paintcraft_core::types::Rgba;
fn document(w: u32, h: u32) -> Document {
    paintcraft_document::new_document(
        w,
        h,
        Some(Rgba([0, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
}
fn request(doc: &Document) -> (Header, Vec<u8>) {
    let plan = SourcePlan::capture(doc).unwrap();
    let source = plan.snapshot(doc).unwrap();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: doc.state_id,
        epoch: 3,
        frontend: 3,
    };
    let header = Header::capture(&plan, &source, owner, Uuid::new_v4()).unwrap();
    let mut bytes = header.encode().to_vec();
    bytes.extend_from_slice(&source);
    (header, bytes)
}
#[test]
fn real_worker_uses_exact_monochrome_msp2_and_bound_preview() {
    let mut doc = document(9, 3);
    let rgba: Vec<_> = (0..27)
        .flat_map(|n| {
            let c = if n % 3 == 0 { 255 } else { 0 };
            [c, c, c, 255]
        })
        .collect();
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster = paintcraft_raster::RasterLayer::from_rgba(
        9,
        3,
        Rgba([0, 0, 0, 255]),
        &rgba,
        &mut paintcraft_raster::Admission::new(1 << 20),
    )
    .unwrap();
    let (header, bytes) = request(&doc);
    let response = execute(&bytes, "test", &|| Ok(())).unwrap();
    match decode_response(&response, &header, header.owner, "test").unwrap() {
        Frame::Success {
            encoded,
            preview,
            preview_size,
            losses,
        } => {
            assert_eq!(
                encoded,
                super::super::encode_v2_rgba_exact(9, 3, &rgba, &|| Ok(())).unwrap()
            );
            assert_eq!(
                &encoded[32..],
                &[3, 0, 3, 0, 3, 0, 2, 0x92, 0, 2, 0x92, 0, 2, 0x92, 0]
            );
            assert_eq!(preview, rgba);
            assert_eq!(preview_size, (9, 3));
            assert!(losses.is_empty());
        }
        _ => panic!("unexpected refusal"),
    }
    assert_eq!(pcnd::write(&doc).unwrap(), bytes[REQUEST_HEADER..]);
}
#[test]
fn independent_whole_file_bound_covers_row_table_and_all_allowed_shapes() {
    // Repeat packets consume at least three source bytes and emit three;
    // literals emit n+1. Consecutive literals require a full 255-byte literal
    // or an intervening >=3-byte run. Thus n+ceil(n/4) bounds each fresh row.
    let mut maximum = (0, 0, 0);
    for width in 1..=65535u64 {
        let height = 65535.min(PIXELS_MAX / width);
        let row = width.div_ceil(8);
        let bound = 32 + height * (2 + row + row.div_ceil(4));
        if bound > maximum.0 {
            maximum = (bound, width, height);
        }
    }
    assert_eq!(maximum, (903410, 65, 64527));
    assert!(maximum.0 < ENCODED_BYTES_MAX as u64);
    assert!(32 + 65535u64 * (2 + 1 + 1) < ENCODED_BYTES_MAX as u64);
}
#[test]
fn request_reserved_version_hash_and_owner_mismatches_fail_closed() {
    let (header, mut bytes) = request(&document(4, 3));
    assert_eq!(Header::decode(&header.encode()).unwrap(), header);
    for at in [0, 8, 10, 113, 115, 127] {
        let mut frame = header.encode();
        frame[at] ^= 1;
        assert!(Header::decode(&frame).is_err(), "{at}");
    }
    bytes[REQUEST_HEADER] ^= 1;
    let response = execute(&bytes, "test", &|| Ok(())).unwrap();
    assert!(matches!(
        decode_response(&response, &header, header.owner, "test").unwrap(),
        Frame::Refused(_)
    ));
    let mut owner = header.owner;
    owner.epoch += 1;
    assert_eq!(
        decode_response(&response, &header, owner, "test")
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn non_monochrome_is_completed_refusal_not_implicit_conversion() {
    let doc = paintcraft_document::new_document(
        3,
        2,
        Some(Rgba([128, 128, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let (header, bytes) = request(&doc);
    let response = execute(&bytes, "test", &|| Ok(())).unwrap();
    let Frame::Refused(error) = decode_response(&response, &header, header.owner, "test").unwrap()
    else {
        panic!()
    };
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
}
#[test]
fn complete_msp_inventory_and_binding_are_required_even_with_updated_digests() {
    let (header, bytes) = request(&document(9, 3));
    let response = execute(&bytes, "test", &|| Ok(())).unwrap();
    for at in [
        0, 8, 11, 12, 20, 28, 32, 73, 89, 121, 122, 194, 198, 263, 265, 266, 287,
    ] {
        let mut changed = response.clone();
        changed[at] ^= 1;
        assert!(
            decode_response(&changed, &header, header.owner, "test").is_err(),
            "byte {at}"
        );
    }
    // A correctly rehashed but incomplete row stream is still invalid output.
    let mut changed = response.clone();
    changed[RESPONSE_HEADER + 32] = 0;
    let n = u32::from_be_bytes(changed[24..28].try_into().unwrap()) as usize;
    let digest = Sha256::digest(&changed[RESPONSE_HEADER..RESPONSE_HEADER + n]);
    changed[129..161].copy_from_slice(&digest);
    assert!(decode_response(&changed, &header, header.owner, "test").is_err());
    let mut changed = response.clone();
    changed[RESPONSE_HEADER + n] = 42;
    let digest = Sha256::digest(&changed[RESPONSE_HEADER + n..]);
    changed[161..193].copy_from_slice(&digest);
    assert!(decode_response(&changed, &header, header.owner, "test").is_err());
    assert!(decode_response(&response, &header, header.owner, "other").is_err());
}
#[test]
fn actual_full_canvas_and_positive_u16_axis_boundaries_keep_pixels_and_refuse_excess() {
    for (w, h) in [(2048, 2048), (65535, 1), (1, 65535)] {
        let (header, bytes) = request(&document(w, h));
        let output = execute(&bytes, "test", &|| Ok(())).unwrap();
        let Frame::Success {
            encoded,
            preview,
            preview_size: size,
            ..
        } = decode_response(&output, &header, header.owner, "test").unwrap()
        else {
            panic!()
        };
        assert!(encoded.len() <= ENCODED_BYTES_MAX);
        assert_eq!(size, preview_size((w, h)));
        assert!(
            preview
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p == &[0, 0, 0, 255])
        );
        let decoded = super::super::decode(encoded, &|| Ok(())).unwrap();
        assert_eq!(
            (decoded.header().width(), decoded.header().height()),
            (w, h)
        );
        assert!(decoded.packed().iter().all(|b| *b == 0));
    }
    for (w, h) in [(65536, 1), (1, 65536), (2049, 2048)] {
        assert_eq!(
            SourcePlan::capture(&document(w, h)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}
#[test]
fn actual_layer_metadata_source_binding_and_cancellation_refuse_before_acceptance() {
    let mut doc = document(3, 2);
    let layer = doc.layers[&doc.roots[0]].clone();
    for _ in 1..9 {
        let mut next = layer.clone();
        next.id = Uuid::new_v4();
        doc.roots.push(next.id);
        doc.layers.insert(next.id, next);
    }
    assert_eq!(
        SourcePlan::capture(&doc).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    // Even a hand-constructed header cannot avoid the reader's early layer cap.
    let source = pcnd::write(&doc).unwrap();
    let (mut header, _) = request(&document(3, 2));
    header.source_len = source.len() as u32;
    header.source_sha256 = Sha256::digest(&source).into();
    header.owner.state = doc.state_id;
    header.source_flags = 1;
    let mut bytes = header.encode().to_vec();
    bytes.extend_from_slice(&source);
    let response = execute(&bytes, "test", &|| Ok(())).unwrap();
    assert!(matches!(
        decode_response(&response, &header, header.owner, "test").unwrap(),
        Frame::Refused(Error {
            code: ErrorCode::ResourceLimit,
            ..
        })
    ));
    let mut doc = document(3, 2);
    doc.title = Some("x".repeat(65537));
    assert!(SourcePlan::capture(&doc).is_err());
    let (header, bytes) = request(&document(3, 2));
    let calls = std::cell::Cell::new(0);
    let output = execute(&bytes, "test", &|| {
        calls.set(calls.get() + 1);
        if calls.get() > 5 {
            Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert!(matches!(
        decode_response(&output, &header, header.owner, "test").unwrap(),
        Frame::Refused(Error {
            code: ErrorCode::OperationCancelled,
            ..
        })
    ));
}
#[test]
fn logical_copy_quote_is_below_declared_reservation_without_rss_claim() {
    let copies = (256u64 << 20)
        + RESIDENT_BYTES_MAX
        + 5 * SOURCE_BYTES_MAX as u64
        + 5 * ENCODED_BYTES_MAX as u64
        + 8 * PREVIEW_BYTES_MAX as u64
        + (1 << 20);
    assert_eq!(copies, 511442944);
    assert!(copies < RESERVATION_BYTES);
}
#[test]
fn actual_skinny_nonuniform_sources_price_all_padded_tiles_and_keep_edge_samples() {
    for (w, h) in [(65535, 1), (1, 65535)] {
        let mut doc = document(w, h);
        let rgba: Vec<_> = (0..65535)
            .flat_map(|i| {
                let c = if i % 2 == 0 { 0 } else { 255 };
                [c, c, c, 255]
            })
            .collect();
        doc.layers.get_mut(&doc.roots[0]).unwrap().raster =
            paintcraft_raster::RasterLayer::from_rgba(
                w,
                h,
                Rgba([0, 0, 0, 255]),
                &rgba,
                &mut paintcraft_raster::Admission::new(64 << 20),
            )
            .unwrap();
        assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 256);
        let (header, bytes) = request(&doc);
        let output = execute(&bytes, "test", &|| Ok(())).unwrap();
        let Frame::Success {
            encoded,
            preview,
            preview_size: size,
            ..
        } = decode_response(&output, &header, header.owner, "test").unwrap()
        else {
            panic!()
        };
        let bitmap = super::super::decode(encoded, &|| Ok(())).unwrap();
        let expected: Vec<_> = if h == 1 {
            (0..8192)
                .map(|i| if i == 8191 { 0x54 } else { 0x55 })
                .collect()
        } else {
            (0..65535)
                .map(|i| if i % 2 == 0 { 0 } else { 0x80 })
                .collect()
        };
        assert_eq!(bitmap.packed(), expected);
        let expected_preview: Vec<_> = (0..size.1)
            .flat_map(|y| {
                (0..size.0).flat_map(move |x| {
                    let sample = u64::from(x) * u64::from(w) / u64::from(size.0)
                        + u64::from(y) * u64::from(h) / u64::from(size.1);
                    let c = if sample % 2 == 0 { 0 } else { 255 };
                    [c, c, c, 255]
                })
            })
            .collect();
        assert_eq!(preview, expected_preview);
    }
}
