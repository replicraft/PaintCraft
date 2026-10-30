use super::*;
use paintcraft_core::types::{ErrorCode, Rgba, filters::Descriptor};
const PIXELS: [u8; 24] = [
    1, 2, 3, 255, 128, 0, 0, 128, 123, 44, 55, 0, 0, 255, 128, 255, 255, 255, 255, 255, 0, 0, 0,
    255,
];
const INVERT: [u8; 24] = [
    254, 253, 252, 255, 127, 255, 255, 128, 0, 0, 0, 0, 255, 0, 127, 255, 0, 0, 0, 255, 255, 255,
    255, 255,
];
fn fixture() -> Document {
    paintcraft_document::import_image(3, 2, &PIXELS, "independent-preview-plane").unwrap()
}
fn retained_source(profile: bool) -> Document {
    let independent = if profile {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd")).as_slice()
    } else {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/resolution-only.pcnd")).as_slice()
    };
    let mut doc = fixture();
    doc.source_metadata = pcnd::read(independent).unwrap().source_metadata;
    doc
}
fn assert_retained_open_frame(profile: bool) {
    let doc = retained_source(profile);
    let original = doc.source_metadata.as_ref().unwrap();
    if profile {
        assert_eq!(
            original.profile().unwrap(),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../assets/profiles/sRGB2014.icc"
            ))
        );
    } else {
        assert!(original.profile().is_none());
        let resolution = original.descriptor().resolution.unwrap();
        assert_eq!(resolution.horizontal.numerator, 30000);
        assert_eq!(resolution.horizontal.denominator, 100);
        assert_eq!(resolution.vertical.numerator, -3780);
    }
    let before = pcnd::write(&doc).unwrap();
    let (mut service, plan) = open(&doc);
    assert_eq!(
        service.base.source_metadata.as_ref().unwrap().digest(),
        original.digest()
    );
    service
        .prepare(&text(&request(
            &plan,
            1,
            Some(doc.roots[0]),
            invert(&doc, true),
            true,
            [0, 0, 3, 2],
            0,
        )))
        .unwrap();
    let (_, pixels) = finish(&mut service);
    assert_eq!(pixels, INVERT);
    service.retire();
    assert_eq!(pcnd::write(&service.base).unwrap(), before);
    assert_eq!(
        service.base.source_metadata.as_ref().unwrap().profile(),
        original.profile()
    );
}
#[test]
fn retained_real_icc_source_opens_and_renders_without_consuming_provenance() {
    assert_retained_open_frame(true);
}
#[test]
fn retained_resolution_only_source_opens_with_zero_profile_payload() {
    assert_retained_open_frame(false);
}
#[test]
fn retained_metadata_declaration_does_not_authorize_wrong_profile_length_or_missing_charge() {
    use paintcraft_document::source_metadata::BYTES_MAX;
    let doc = retained_source(true);
    let bytes = pcnd::write(&doc).unwrap();
    let value = plan(&doc, &bytes);
    let exact = integer(field(&value, "metadataBytes").unwrap()).unwrap();
    for (name, bad, code) in [
        ("layers", Value::Int(0), ErrorCode::ResourceLimit),
        ("metadataBytes", Value::Int(0), ErrorCode::ResourceLimit),
        (
            "metadataBytes",
            Value::Int((exact + 1) as i64),
            ErrorCode::ConflictStaleBase,
        ),
        ("metadataCharge", Value::Int(1), ErrorCode::ResourceLimit),
        (
            "metadataBytes",
            Value::Int(BYTES_MAX as i64),
            ErrorCode::ResourceLimit,
        ),
    ] {
        let mut fields = value.as_object().unwrap().clone();
        fields.insert(name.into(), bad);
        assert_eq!(
            Service::open(&bytes, &text(&Value::Object(fields)))
                .err()
                .unwrap()
                .code,
            code,
            "{name}"
        );
    }
    let doc = retained_source(false);
    let bytes = pcnd::write(&doc).unwrap();
    let mut fields = plan(&doc, &bytes).as_object().unwrap().clone();
    fields.insert("metadataCharge".into(), Value::Int(0));
    assert_eq!(
        Service::open(&bytes, &text(&Value::Object(fields)))
            .err()
            .unwrap()
            .code,
        ErrorCode::ResourceLimit
    );
}
fn plan(doc: &Document, bytes: &[u8]) -> Value {
    let metadata = doc.source_metadata.as_ref();
    Value::object([
        (
            "owner",
            Value::object([
                ("session", Value::str(Uuid::from_u128(1).to_string())),
                ("state", Value::str(doc.state_id.to_string())),
                ("epoch", Value::str("0")),
                ("frontend", Value::Int(3)),
            ]),
        ),
        ("draft", Value::str(Uuid::from_u128(2).to_string())),
        ("document", Value::str(doc.document_id.to_string())),
        ("sourceSha256", Value::str(hex(&Sha256::digest(bytes)))),
        (
            "size",
            Value::Array(vec![
                Value::Int(i64::from(doc.canvas.0)),
                Value::Int(i64::from(doc.canvas.1)),
            ]),
        ),
        ("layers", Value::Int(doc.layers.len() as i64)),
        (
            "residentBytes",
            Value::Int(
                doc.layers
                    .values()
                    .map(|l| {
                        l.raster.resident_tiles() as i64
                            * paintcraft_raster::RasterLayer::TILE_BYTES as i64
                    })
                    .sum(),
            ),
        ),
        (
            "metadataBytes",
            Value::Int(metadata.and_then(|m| m.profile()).map_or(0, |p| p.len()) as i64),
        ),
        (
            "metadataCharge",
            Value::Int(metadata.map_or(0, |m| m.storage_usage().1) as i64),
        ),
        (
            "filterDigest",
            Value::str(hex(&filter_bake::inspect(doc, &|| Ok(()))
                .unwrap()
                .digest())),
        ),
    ])
}
fn open(doc: &Document) -> (Service, Value) {
    let bytes = pcnd::write(doc).unwrap();
    let plan = plan(doc, &bytes);
    (Service::open(&bytes, &text(&plan)).unwrap(), plan)
}
fn request(
    plan: &Value,
    generation: u64,
    target: Option<Uuid>,
    operation: Value,
    render: bool,
    region: [i64; 4],
    level: u8,
) -> Value {
    Value::object([
        ("owner", field(plan, "owner").unwrap().clone()),
        ("draft", field(plan, "draft").unwrap().clone()),
        ("sourceSha256", field(plan, "sourceSha256").unwrap().clone()),
        ("generation", Value::str(generation.to_string())),
        ("operation", operation),
        (
            "target",
            target.map_or(Value::Null, |id| Value::str(id.to_string())),
        ),
        (
            "view",
            Value::object([
                (
                    "region",
                    Value::Array(region.into_iter().map(Value::Int).collect()),
                ),
                ("level", Value::Int(i64::from(level))),
            ]),
        ),
        ("render", Value::Bool(render)),
    ])
}
fn invert(doc: &Document, enabled: bool) -> Value {
    FilterOp::Add {
        layer: LayerRef::Id(doc.roots[0]),
        alias: "preview".into(),
        descriptor: Descriptor::InvertRgbV1,
        enabled,
        to_index: None,
    }
    .to_value()
    .unwrap()
}
fn finish(service: &mut Service) -> (Value, Vec<u8>) {
    while !service.step().unwrap() {}
    service.take_frame().unwrap()
}
#[test]
fn exact_private_appearance_and_source_reuse_across_edits_cancel_and_base() {
    let doc = fixture();
    let before = pcnd::write(&doc).unwrap();
    let (mut s, p) = open(&doc);
    for generation in 0..20 {
        let enabled = generation % 2 == 0;
        let req = request(
            &p,
            generation,
            Some(doc.roots[0]),
            invert(&doc, enabled),
            true,
            [0, 0, 3, 2],
            0,
        );
        let binding = s.prepare(&text(&req)).unwrap();
        assert_eq!(binding.get("kind"), Some(&Value::str("frame")));
        let (_, pixels) = finish(&mut s);
        let base = [
            1, 2, 3, 255, 128, 0, 0, 128, 0, 0, 0, 0, 0, 255, 128, 255, 255, 255, 255, 255, 0, 0,
            0, 255,
        ];
        assert_eq!(
            pixels,
            if enabled {
                INVERT.as_slice()
            } else {
                base.as_slice()
            }
        );
        assert_eq!(pcnd::write(&s.base).unwrap(), before);
        s.retire();
        assert!(s.frame.is_none() && s.candidate.is_none());
    }
    s.prepare(&text(&request(
        &p,
        20,
        None,
        Value::Null,
        true,
        [0, 0, 3, 2],
        0,
    )))
    .unwrap();
    let (_, pixels) = finish(&mut s);
    assert_eq!(&pixels[..4], &PIXELS[..4]);
    assert_eq!(
        s.base.layers[&doc.roots[0]].raster.get(2, 0),
        Rgba([123, 44, 55, 0])
    );
}
#[test]
fn prepare_only_admits_candidate_without_pixels_and_refuses_forged_context_refs_or_generation() {
    let doc = fixture();
    let (mut s, p) = open(&doc);
    let req = request(
        &p,
        1,
        Some(doc.roots[0]),
        invert(&doc, true),
        false,
        [0, 0, 3, 2],
        0,
    );
    assert_eq!(
        s.prepare(&text(&req)).unwrap().get("kind"),
        Some(&Value::str("prepared"))
    );
    assert!(s.frame.is_none());
    assert!(s.candidate.is_some());
    assert_eq!(
        s.prepare(&text(&req)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let mut forged = req.as_object().unwrap().clone();
    forged.insert("draft".into(), Value::str(Uuid::from_u128(99).to_string()));
    assert_eq!(
        s.prepare(&text(&Value::Object(forged))).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let op = FilterOp::Add {
        layer: LayerRef::Alias("ink".into()),
        alias: "effect".into(),
        descriptor: Descriptor::InvertRgbV1,
        enabled: true,
        to_index: None,
    }
    .to_value()
    .unwrap();
    assert_eq!(
        s.prepare(&text(&request(
            &p,
            2,
            Some(doc.roots[0]),
            op,
            false,
            [0, 0, 3, 2],
            0
        )))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn sparse_canonical_width_and_zoomed_roi_do_not_materialize_the_logical_canvas() {
    let doc = paintcraft_document::new_document(
        1_048_576,
        1,
        Some(Rgba([1, 2, 3, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let (mut s, p) = open(&doc);
    s.prepare(&text(&request(
        &p,
        0,
        Some(doc.roots[0]),
        invert(&doc, true),
        true,
        [0, 0, 1_048_576, 1],
        10,
    )))
    .unwrap();
    assert!(!s.step().unwrap()); //32 output samples per cooperative patch.
    s.retire();
    assert!(s.frame.is_none());
    s.prepare(&text(&request(
        &p,
        1,
        None,
        Value::Null,
        true,
        [65_537, 0, 17, 1],
        0,
    )))
    .unwrap();
    let (binding, pixels) = finish(&mut s);
    assert_eq!(binding.get("width"), Some(&Value::Int(17)));
    assert_eq!(pixels, [1, 2, 3, 255].repeat(17));
}
#[test]
fn source_mismatch_and_incompatible_quotes_refuse_before_decoder_allocation() {
    let doc = fixture();
    let (_, p) = open(&doc);
    let mut map = p.as_object().unwrap().clone();
    map.insert("layers".into(), Value::Int(100_000));
    assert_eq!(
        Service::open(b"not a project", &text(&Value::Object(map)))
            .err()
            .unwrap()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut bytes = pcnd::write(&doc).unwrap();
    bytes[0] ^= 1;
    assert_eq!(
        Service::open(&bytes, &text(&p)).err().unwrap().code,
        ErrorCode::ConflictStaleBase
    );
    let mut map = p.as_object().unwrap().clone();
    map.insert("residentBytes".into(), Value::Int(1));
    assert_eq!(
        Source::parse(Value::Object(map)).unwrap_err().code,
        ErrorCode::InputSchema
    );
}
