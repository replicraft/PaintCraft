use super::*;
const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../crates/services/paintcraft-formats/tests/tiff-fixtures/unsupported-first-rgba16.tif"
));
const EXPECTED: [u8; 16] = [
    0, 0, 1, 0, 171, 152, 118, 128, 255, 0, 127, 255, 1, 128, 255, 1,
];
fn capture(source: &[u8], selection: Option<wire::Selection>) -> wire::Header {
    let doc =
        paintcraft_document::new_document(1, 1, None, 0, paintcraft_document::POLICY).unwrap();
    wire::Header::capture(
        source,
        paintcraft_formats::codec_worker::Owner {
            session: doc.document_id,
            state: doc.state_id,
            epoch: 3,
            frontend: 3,
        },
        doc.document_id,
        wire::PIXELS_MAX,
        selection,
    )
    .unwrap()
}
fn run(header: wire::Header, source: &[u8]) -> Vec<u8> {
    let mut bytes = header.encode().to_vec();
    bytes.extend_from_slice(source);
    execute(&bytes).unwrap()
}

#[test]
fn actual_worker_normalizes_each_bound_orientation_and_refuses_inventory_transform_forgery() {
    let source = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../crates/services/paintcraft-formats/tests/tiff-orientation-fixtures/rgba8-le-eight-orientations.tif"
    ));
    let oracle = paintcraft_core::types::json::parse(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../../../crates/services/paintcraft-formats/tests/tiff-orientation-fixtures/oracle.json"))).unwrap();
    let expected = oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .get("pages")
        .unwrap()
        .as_array()
        .unwrap();
    let request = capture(source, None);
    let frame = run(request, source);
    let wire::Frame::Success {
        page_count,
        payload,
        ..
    } = wire::decode_response(&frame, &request, request.owner, env!("CARGO_PKG_VERSION")).unwrap()
    else {
        panic!("inventory expected")
    };
    let pages = wire::decode_inventory(payload, page_count, source.len() as u64).unwrap();
    assert_eq!(pages.len(), 8);
    for (index, page) in pages.into_iter().enumerate() {
        assert_eq!(page.orientation, index as u32 + 1);
        let selection = wire::Selection {
            page,
            page_count,
            source_sha256: request.source_sha256,
        };
        let request = capture(source, Some(selection));
        let frame = run(request, source);
        let wire::Frame::Success {
            width,
            height,
            report,
            payload,
            ..
        } = wire::decode_response(&frame, &request, request.owner, env!("CARGO_PKG_VERSION"))
            .unwrap()
        else {
            panic!("decode expected")
        };
        let rgba: Vec<u8> = expected[index]
            .get("rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap() as u8)
            .collect();
        assert_eq!(payload, rgba);
        assert_eq!((width, height), page.output_dimensions());
        let report = tiff_pages::Assessment::decode(report).unwrap();
        assert_eq!(report.source_dimensions(), (3, 2));
        assert_eq!(report.descriptor().orientation, page.orientation);
        let mut stale = selection;
        stale.page.orientation = if page.orientation == 1 { 2 } else { 1 };
        let request = capture(source, Some(stale));
        let frame = run(request, source);
        assert!(
            matches!(wire::decode_response(&frame,&request,request.owner,env!("CARGO_PKG_VERSION")).unwrap(), wire::Frame::Refused(error) if error.code == ErrorCode::ConflictStaleBase)
        );
    }
}
#[test]
fn actual_whole_tiff_worker_inventories_unsupported_first_then_decodes_later_page() {
    let inventory = capture(SOURCE, None);
    let output = run(inventory, SOURCE);
    let wire::Frame::Success {
        page_count,
        payload,
        ..
    } = wire::decode_response(
        &output,
        &inventory,
        inventory.owner,
        env!("CARGO_PKG_VERSION"),
    )
    .unwrap()
    else {
        panic!("inventory expected")
    };
    let pages = wire::decode_inventory(payload, page_count, SOURCE.len() as u64).unwrap();
    assert_eq!(page_count, 2);
    assert!(!pages[0].supported);
    assert!(pages[1].supported);
    let selection = wire::Selection {
        page: pages[1],
        page_count,
        source_sha256: inventory.source_sha256,
    };
    let selected = capture(SOURCE, Some(selection));
    let output = run(selected, SOURCE);
    let wire::Frame::Success {
        width,
        height,
        report,
        payload,
        ..
    } = wire::decode_response(
        &output,
        &selected,
        selected.owner,
        env!("CARGO_PKG_VERSION"),
    )
    .unwrap()
    else {
        panic!("selected image expected")
    };
    assert_eq!((width, height), (2, 2));
    assert_eq!(payload, EXPECTED);
    let assessment = tiff_pages::Assessment::decode(report).unwrap();
    assessment
        .validate_result(
            selected.source_sha256,
            selected.source_bytes,
            1,
            width,
            height,
            payload,
        )
        .unwrap();
    assert_eq!(assessment.descriptor().ifd_offset, pages[1].ifd_offset);
    assert_eq!(
        assessment.losses(),
        [
            "depth-16-reduced-to-8",
            "tiff-metadata-ignored",
            tiff_pages::LOSS_OTHER_PAGES
        ]
    );
    assert_eq!(assessment.source_depth(), 16);
    let mut changed = selection;
    changed.page.ifd_offset += 1;
    let stale = capture(SOURCE, Some(changed));
    let output = run(stale, SOURCE);
    let wire::Frame::Refused(error) =
        wire::decode_response(&output, &stale, stale.owner, env!("CARGO_PKG_VERSION")).unwrap()
    else {
        panic!("stale selection expected")
    };
    assert_eq!(error.code, ErrorCode::ConflictStaleBase);
}
#[test]
fn full_source_hash_and_distinct_operation_prevent_payload_slice_or_decoder_fallback() {
    let header = capture(SOURCE, None);
    let mut request = header.encode().to_vec();
    request.extend_from_slice(SOURCE);
    request[wire::REQUEST_HEADER] ^= 1;
    assert!(execute(&request).is_err());
    for magic in [
        b"PCDEC002",
        b"PCDDSQ02",
        b"PCDDSQ01",
        b"PCMSPQ02",
        b"PCMSPQ01",
    ] {
        request[..8].copy_from_slice(magic);
        assert!(execute(&request).is_err());
    }
    assert_eq!(SOURCE.len() as u64, header.source_bytes);
}
