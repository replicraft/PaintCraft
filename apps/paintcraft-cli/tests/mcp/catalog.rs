//! Real stdio catalog discovery, atomic edits, persistence and downgrade.
use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::pcnd;
use std::{fs, path::Path};

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

const BUNDLE: &str = "cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453";

#[test]
fn catalog_stdio_levels_inventory_save_reopen_noop_and_remove_downgrade() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-mcp-catalog-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&folder).unwrap();
    let source = folder.join("source.pcnd");
    let output = folder.join("levels.pcnd");
    let removed = folder.join("removed.pcnd");
    let doc = new_document(2, 1, Some(Rgba([19, 77, 183, 0])), 0, POLICY).unwrap();
    let layer = doc.roots[0];
    let original = pcnd::write_compact(&doc).unwrap();
    fs::write(&source, &original).unwrap();
    let mut c = Client::new();
    c.ok("open_project", json!({"path":source}));
    let before = c.ok("inspect_document", json!({}));
    let descriptor = json!({"kind":"levels-rgb-v1","input_black":0,"input_white":255,
        "gamma_hundredths":100,"output_black":0,"output_white":255});
    let receipt = c.ok("apply_operations", json!({"operations":[{"op":"filter.add",
        "layer_ref":{"id":layer.to_string()},"ref":"tone","enabled":false,"descriptor":descriptor}]}));
    assert_eq!(receipt["schema"], "paintcraft.result/catalog-v1");
    assert_eq!(receipt["required_filters"], json!(["levels-rgb-v1"]));
    assert_ne!(receipt["state_id"], before["state_id"]);
    let id = receipt["filter_aliases"]["tone"]["filter_id"].clone();
    let inspected = c.ok("inspect_document", json!({}));
    if let Some(root) = std::env::var_os("PAINTCRAFT_CATALOG_RECEIPTS") {
        let root = std::path::PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        for (name, value) in [("mcp-result", &receipt), ("mcp-inspect", &inspected)] {
            fs::write(
                root.join(format!("{name}.json")),
                serde_json::to_vec_pretty(value).unwrap(),
            )
            .unwrap();
        }
    }
    assert_eq!(inspected["schema"], "paintcraft.mcp-inspect/catalog-v1");
    assert_eq!(inspected["required_filters"], json!(["levels-rgb-v1"]));
    assert_eq!(
        inspected["document_features"],
        json!(["layer-point-filters.v1", "layer-filter-catalog.v1"])
    );
    assert_eq!(
        inspected["layers"][0]["filters"],
        json!([{"id":id,"enabled":false,"descriptor":descriptor}])
    );
    let noop = c.ok(
        "apply_operations",
        json!({"operations":[{"op":"filter.update",
        "layer_ref":{"id":layer.to_string()},"filter_ref":{"id":id},"descriptor":descriptor}]}),
    );
    assert_eq!(noop["committed"], false);
    assert_eq!(noop["schema"], "paintcraft.result/catalog-v1");
    assert_eq!(c.ok("inspect_document", json!({})), inspected);
    c.ok("save_project", json!({"path":output}));
    let saved = fs::read(&output).unwrap();
    assert_eq!(saved[4], 6);
    let decoded = read_catalog(&saved);
    assert_eq!(decoded.layers[&layer].raster, doc.layers[&layer].raster);
    c.ok("open_project", json!({"path":output}));
    let reopened = c.ok("inspect_document", json!({}));
    assert_eq!(reopened["state_id"], inspected["state_id"]);
    assert_eq!(reopened["layers"], inspected["layers"]);
    let removed_receipt = c.ok(
        "apply_operations",
        json!({"operations":[{"op":"filter.remove",
        "layer_ref":{"id":layer.to_string()},"filter_ref":{"id":id}}]}),
    );
    assert_eq!(removed_receipt["schema"], "paintcraft.result/v1");
    assert!(removed_receipt.get("required_filters").is_none());
    let plain = c.ok("inspect_document", json!({}));
    assert!(plain.get("schema").is_none());
    assert!(plain.get("required_filters").is_none());
    assert!(plain["layers"][0].get("filters").is_none());
    c.ok("save_project", json!({"path":removed}));
    assert_eq!(fs::read(&removed).unwrap()[4], 1);
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(fs::read(&output).unwrap(), saved);
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn catalog_stdio_discovery_keeps_legacy_batch_and_matches_immutable_bundle() {
    let mut c = Client::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas/catalog")
        .join(BUNDLE);
    for (kind, name) in [
        ("manifest", "manifest.json"),
        ("batch", "batch.schema.json"),
        ("result", "result.schema.json"),
        ("file-command", "file-command.schema.json"),
        ("inspect", "inspect.schema.json"),
        ("mcp-inspect", "mcp-inspect.schema.json"),
    ] {
        let expected: Value = serde_json::from_slice(&fs::read(root.join(name)).unwrap()).unwrap();
        assert_eq!(c.ok("get_catalog_schema", json!({"kind":kind})), expected);
    }
    let expected: Value = serde_json::from_slice(
        &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas/batch-v5.schema.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(c.ok("get_batch_schema", json!({})), expected);
}

#[test]
fn catalog_stdio_export_active_levels_requires_bake_and_matches_literal_png() {
    use paintcraft_core::types::filters::{Descriptor, LevelsRgbV1};
    use paintcraft_document::{FilterInstance, FilterStack};
    let folder = std::env::temp_dir().join(format!(
        "paintcraft-mcp-catalog-render-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&folder).unwrap();
    let source = folder.join("levels.pcnd");
    let png = folder.join("export.png");
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
    let mut c = Client::new();
    c.ok("open_project", json!({"path":source}));
    let before = c.ok("inspect_document", json!({}));
    let refused = c.call("export_image", json!({"path":png}));
    assert_eq!(refused["isError"], true);
    assert!(refused.to_string().contains("explicit consent to bake"));
    assert!(!png.exists());
    assert_eq!(c.ok("inspect_document", json!({})), before);
    c.ok(
        "export_image",
        json!({"path":png,"allow_losses":["filters-baked"]}),
    );
    let decoded = paintcraft_formats::image::decode(&fs::read(&png).unwrap()).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 1));
    // Literal expected pixels; no product Levels evaluator supplies this oracle.
    assert_eq!(decoded.rgba, [128, 64, 181, 255, 128, 64, 181, 255]);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(
        read_catalog(&bytes).layers[&layer].raster,
        doc.layers[&layer].raster
    );
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}
