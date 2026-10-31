//! Actual catalog packets retain source pixels and use conditional profiles.
use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::pcnd;
use serde_json::{Value as Json, json};

pub(super) fn read_catalog(bytes: &[u8]) -> paintcraft_document::Document {
    let metadata = paintcraft_raster::Admission::new(0).paint_metadata_budget();
    let coverage = paintcraft_core::types::resources::default_coverage_budget();
    pcnd::read_with_admission_checked(
        bytes,
        2,
        65_536,
        1,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: paintcraft_document::source_metadata::BYTES_MAX,
                budget: &metadata,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 256 << 20,
                budget: &coverage,
                limits: paintcraft_raster::coverage_mask::Limits::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}

const BUNDLE: &str = "a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a";
fn descriptor() -> Json {
    json!({"kind":"levels-rgb-v1","input_black":0,"input_white":255,
        "gamma_hundredths":100,"output_black":0,"output_white":255})
}
fn packet(state: &str, operations: Json) -> Json {
    json!({"schema":"paintcraft.batch/catalog-v1","atomic":true,"policy":POLICY,
        "base":{"state_id":state,"publication_epoch":"0"},
        "required_filters":["levels-rgb-v1"],"operations":operations})
}
fn applied(input: &str, ops: &str, output: &str, value: &Json) -> Run {
    fs::write(ops, serde_json::to_vec(value).unwrap()).unwrap();
    run(&["apply", input, "--operations", ops, "-o", output, "--json"])
}
fn record(name: &str, value: &Json) {
    if let Some(root) = std::env::var_os("PAINTCRAFT_CATALOG_RECEIPTS") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
fn parsed(run: &Run) -> Json {
    assert_eq!(run.code, 0, "{} {}", run.stdout, run.stderr);
    serde_json::from_str(&run.stdout).unwrap()
}

#[test]
fn catalog_cli_disabled_levels_save_inspect_remove_and_closed_legacy_refusals() {
    let dir = Scratch::new("catalog-cli");
    let input = dir.path("source.pcnd");
    let output = dir.path("levels.pcnd");
    let removed = dir.path("removed.pcnd");
    let ops = dir.path("ops.json");
    let doc = new_document(2, 1, Some(Rgba([17, 83, 201, 0])), 0, POLICY).unwrap();
    let layer = doc.roots[0];
    let original = pcnd::write_compact(&doc).unwrap();
    fs::write(&input, &original).unwrap();
    let add = json!({"op":"filter.add","layer_ref":{"id":layer.to_string()},
        "ref":"tone","enabled":false,"descriptor":descriptor()});
    let batch = packet(&doc.state_id.to_string(), json!([add]));
    let receipt = parsed(&applied(&input, &ops, &output, &batch));
    record("cli-envelope", &receipt);
    record("cli-result", &receipt["engine"]);
    assert_eq!(receipt["schema"], "paintcraft.file-command/catalog-v1");
    assert_eq!(receipt["engine"]["schema"], "paintcraft.result/catalog-v1");
    assert_eq!(
        receipt["engine"]["required_filters"],
        json!(["levels-rgb-v1"])
    );
    assert_eq!(
        receipt["engine"]["document_features"],
        json!(["layer-point-filters.v1", "layer-filter-catalog.v1"])
    );
    let bytes = fs::read(&output).unwrap();
    assert_eq!(bytes[4], 6);
    let saved = read_catalog(&bytes);
    assert_eq!(saved.document_id, doc.document_id);
    assert_eq!(saved.layers[&layer].raster, doc.layers[&layer].raster);
    let filter = saved.layers[&layer].filters.iter().next().unwrap();
    assert!(!filter.enabled());
    assert_eq!(
        filter.descriptor().to_value(),
        json::parse(&serde_json::to_vec(&descriptor()).unwrap()).unwrap()
    );
    let inspected = parsed(&run(&["inspect", &output, "--json"]));
    record("cli-inspect", &inspected);
    assert_eq!(inspected["schema"], "paintcraft.inspect/catalog-v1");
    assert_eq!(inspected["required_filters"], json!(["levels-rgb-v1"]));
    assert_eq!(
        inspected["assessment"]["required_filters"],
        json!([{"profile_id":"levels-rgb-v1","codec_id":"params-v1"}])
    );
    assert_eq!(
        inspected["document"]["state_id"],
        receipt["engine"]["state_id"]
    );
    assert_eq!(
        inspected["document"]["layers"][0]["filters"][0]["enabled"],
        false
    );

    let mut removal = packet(
        &saved.state_id.to_string(),
        json!([{"op":"filter.remove",
        "layer_ref":{"id":layer.to_string()},"filter_ref":{"id":filter.id().to_string()}}]),
    );
    removal["required_filters"] = json!([]);
    let receipt = parsed(&applied(&output, &ops, &removed, &removal));
    assert_eq!(receipt["schema"], "paintcraft.file-command/v1");
    assert_eq!(receipt["engine"]["schema"], "paintcraft.result/v1");
    assert!(receipt["engine"].get("required_filters").is_none());
    let plain = read_catalog(&fs::read(&removed).unwrap());
    assert!(plain.layers[&layer].filters.is_empty());
    assert_eq!(plain.layers[&layer].raster, doc.layers[&layer].raster);
    assert_eq!(
        parsed(&run(&["inspect", &removed, "--json"]))["schema"],
        "paintcraft.inspect/v1"
    );

    for mode in ["legacy", "missing", "unused"] {
        let dest = dir.path(&format!("refused-{mode}.pcnd"));
        let mut invalid = batch.clone();
        match mode {
            "legacy" => {
                invalid["schema"] = json!("paintcraft.batch/v5");
                invalid.as_object_mut().unwrap().remove("required_filters");
            }
            "missing" => {
                invalid.as_object_mut().unwrap().remove("required_filters");
            }
            "unused" => invalid["required_filters"] = json!(["invert-rgb-v1", "levels-rgb-v1"]),
            _ => unreachable!(),
        }
        let result = applied(&input, &ops, &dest, &invalid);
        assert_ne!(result.code, 0, "{mode}: {}", result.stdout);
        assert!(!Path::new(&dest).exists());
        assert_eq!(fs::read(&input).unwrap(), original);
    }
    assert_eq!(fs::read(&output).unwrap(), bytes);
}

#[test]
fn catalog_cli_discovery_matches_every_immutable_shipping_schema() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas/catalog")
        .join(BUNDLE);
    for (choice, name) in [
        ("catalog", "manifest.json"),
        ("batch-catalog", "batch.schema.json"),
        ("result-catalog", "result.schema.json"),
        ("file-command-catalog", "file-command.schema.json"),
        ("inspect-catalog", "inspect.schema.json"),
        ("mcp-inspect-catalog", "mcp-inspect.schema.json"),
    ] {
        let actual = parsed(&run(&["schema", choice]));
        let expected: Json = serde_json::from_slice(&fs::read(root.join(name)).unwrap()).unwrap();
        assert_eq!(actual, expected);
        if name != "manifest.json" {
            assert!(
                actual["$id"]
                    .as_str()
                    .unwrap()
                    .starts_with(&format!("urn:paintcraft:schema:catalog-v1:{BUNDLE}:"))
            );
        }
    }
}

#[test]
fn catalog_cli_active_nondefault_levels_inventory_includes_disabled_legacy_intent() {
    let dir = Scratch::new("catalog-cli-mixed");
    let input = dir.path("source.pcnd");
    let output = dir.path("mixed.pcnd");
    let ops = dir.path("ops.json");
    let doc = new_document(2, 1, Some(Rgba([16, 64, 144, 127])), 0, POLICY).unwrap();
    let original = pcnd::write_compact(&doc).unwrap();
    fs::write(&input, &original).unwrap();
    let layer = doc.roots[0].to_string();
    let mut desc = descriptor();
    desc["gamma_hundredths"] = json!(200);
    desc["output_black"] = json!(255);
    desc["output_white"] = json!(0);
    let mut batch = packet(
        &doc.state_id.to_string(),
        json!([
            {"op":"filter.add","layer_ref":{"id":layer},"ref":"legacy","enabled":false,"descriptor":{"kind":"invert-rgb-v1"}},
            {"op":"filter.add","layer_ref":{"id":layer},"ref":"tone","descriptor":desc}
        ]),
    );
    batch["required_filters"] = json!(["invert-rgb-v1", "levels-rgb-v1"]);
    let receipt = parsed(&applied(&input, &ops, &output, &batch));
    assert_eq!(
        receipt["engine"]["required_filters"],
        batch["required_filters"]
    );
    let inspected = parsed(&run(&["inspect", &output, "--json"]));
    assert_eq!(inspected["required_filters"], batch["required_filters"]);
    assert_eq!(
        inspected["document"]["layers"][0]["filters"][1]["descriptor"],
        desc
    );
    assert_eq!(
        inspected["document"]["layers"][0]["filters"][1]["enabled"],
        true
    );
    let saved = read_catalog(&fs::read(&output).unwrap());
    assert_eq!(
        saved.layers[&doc.roots[0]].raster,
        doc.layers[&doc.roots[0]].raster
    );
    assert_eq!(fs::read(&input).unwrap(), original);
    record("cli-mixed-inspect", &inspected);
}

#[test]
fn catalog_cli_render_active_levels_requires_bake_and_matches_literal_png() {
    use paintcraft_core::types::filters::{Descriptor, LevelsRgbV1};
    use paintcraft_document::{FilterInstance, FilterStack};
    let dir = Scratch::new("catalog-render");
    let source = dir.path("levels.pcnd");
    let png = dir.path("rendered.png");
    let mut doc = new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY).unwrap();
    let layer = doc.roots[0];
    doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        uuid::Uuid::from_u128(0x1e7e15),
        true,
        Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap()),
    )])
    .unwrap();
    let bytes = pcnd::write_compact(&doc).unwrap();
    assert_eq!(bytes[4], 6);
    fs::write(&source, &bytes).unwrap();
    let before = parsed(&run(&["inspect", &source, "--json"]));
    let refused = run(&["render", &source, "-o", &png, "--json"]);
    assert_ne!(refused.code, 0, "{}", refused.stdout);
    assert!(refused.stdout.contains("explicit consent to bake"));
    assert!(!Path::new(&png).exists());
    assert_eq!(fs::read(&source).unwrap(), bytes);
    parsed(&run(&[
        "render",
        &source,
        "-o",
        &png,
        "--allow-loss",
        "filters-baked",
        "--json",
    ]));
    // Independent gamma2 square-root literals, including half-up rounding.
    assert_eq!(
        decode_png(&png),
        (2, 1, vec![128, 64, 181, 255, 128, 64, 181, 255])
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(parsed(&run(&["inspect", &source, "--json"])), before);
    assert_eq!(
        read_catalog(&bytes).layers[&layer].raster,
        doc.layers[&layer].raster
    );
}
