//! Real transports retain masks and expose conditionally closed inspection/results.
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY};
use paintcraft_formats::pcnd;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/persistent-filter-masks-cli")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn cli(args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn record(name: &str, value: &Value) {
    if let Some(path) = std::env::var_os("PAINTCRAFT_MASK_RECEIPTS") {
        let path = PathBuf::from(path);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
fn read(bytes: &[u8]) -> Document {
    pcnd::read_with_admission_checked(
        bytes,
        u64::MAX,
        u64::MAX,
        paintcraft_document::LAYER_COUNT_MAX,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &SharedBudget::new(8 << 20),
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 32 << 20,
                budget: &SharedBudget::new(32 << 20),
                limits: paintcraft_raster::coverage_mask::Limits::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
fn mask_operation(layer: &str, filter: Value) -> Value {
    json!({"op":"filter.mask.set","layer_ref":{"id":layer},"filter_ref":filter,"mask":{
        "domain":{"x":-1,"y":-1,"width":5,"height":3},
        "bands":[{"y0":0,"y1":1,"runs":[{"x0":0,"x1":1,"coverage":128},{"x0":2,"x1":3,"coverage":255}]}]}})
}
fn receipt(value: &Value, version: u8) {
    assert_eq!(value["schema"], format!("paintcraft.result/v{version}"));
    if version == 3 {
        assert_eq!(
            value["document_features"],
            json!(["layer-point-filters.v1", "layer-filter-masks.v1"])
        );
    } else {
        assert!(value.get("document_features").is_none());
    }
}
#[test]
fn real_cli_and_stdio_masks_persist_inspect_undo_and_keep_source_pixels() {
    let folder = Folder::new();
    let input = folder.0.join("source.pcnd");
    let output = folder.0.join("masked.pcnd");
    let operations = folder.0.join("operations.json");
    let doc =
        paintcraft_document::new_document(4, 2, Some(Rgba([10, 40, 70, 255])), 77, POLICY).unwrap();
    let original = pcnd::write_compact(&doc).unwrap();
    fs::write(&input, &original).unwrap();
    let layer = doc.roots[0].to_string();
    let ops = json!([
        {"op":"filter.add","layer_ref":{"id":layer},"ref":"tone","descriptor":{"kind":"invert-rgb-v1"}},
        mask_operation(&layer,json!({"ref":"tone"}))
    ]);
    let packet = json!({"schema":"paintcraft.batch/v3","atomic":true,"policy":POLICY,"base":{"state_id":doc.state_id.to_string(),"publication_epoch":"0"},"operations":ops});
    record("batch-v3", &packet);
    fs::write(&operations, serde_json::to_vec(&packet).unwrap()).unwrap();
    let report = cli(&[
        "apply",
        input.to_str().unwrap(),
        "--operations",
        operations.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(report["schema"], "paintcraft.file-command/v3");
    receipt(&report["engine"], 3);
    record("envelope-v3", &report);
    record("result-v3", &report["engine"]);
    let masked = read(&fs::read(&output).unwrap());
    assert_eq!(
        pcnd::probe(&fs::read(&output).unwrap())
            .unwrap()
            .container_version,
        4
    );
    assert_eq!(
        masked.layers[&doc.roots[0]].raster,
        doc.layers[&doc.roots[0]].raster
    );
    let coverage = masked.layers[&doc.roots[0]].filters.as_slice()[0]
        .mask()
        .unwrap()
        .coverage();
    assert_eq!(coverage.domain().x0, -1);
    assert_eq!(
        [
            coverage.coverage(0, 0),
            coverage.coverage(1, 0),
            coverage.coverage(2, 0)
        ],
        [128, 0, 255]
    );
    assert_eq!(
        paintcraft_render::render_flat(&masked).unwrap(),
        [
            128, 128, 128, 255, 10, 40, 70, 255, 245, 215, 185, 255, 10, 40, 70, 255, 10, 40, 70,
            255, 10, 40, 70, 255, 10, 40, 70, 255, 10, 40, 70, 255,
        ]
    );
    let info = cli(&["inspect", output.to_str().unwrap(), "--json"]);
    assert_eq!(info["schema"], "paintcraft.inspect/v2");
    assert_eq!(
        info["assessment"]["document_schema"],
        "paintcraft.document/v4"
    );
    assert_eq!(
        info["document"]["layers"][0]["filters"][0]["mask"]["schema"],
        "a8-bands/v1"
    );
    assert!(
        info["document"]["layers"][0]["filters"][0]["mask"]
            .get("payload")
            .is_none()
    );
    record("inspect-v2", &info);
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("open_project", json!({"path":input}));
    let before = client.ok("inspect_document", json!({}));
    assert!(before.get("schema").is_none());
    let result = client.ok("apply_operations", json!({"operations":ops}));
    receipt(&result, 3);
    record("mcp-result-v3", &result);
    assert_eq!(result["state_id"], report["engine"]["state_id"]);
    assert_eq!(result["filter_aliases"], report["engine"]["filter_aliases"]);
    let info = client.ok("inspect_document", json!({}));
    assert_eq!(info["schema"], "paintcraft.mcp-inspect/v2");
    assert_eq!(info["undo_depth"], 1);
    record("mcp-inspect-v2", &info);
    let saved = folder.0.join("stdio.pcnd");
    client.ok("save_project", json!({"path":saved}));
    assert!(read(&fs::read(&saved).unwrap()).same_semantics(&masked));
    client.ok("history", json!({}));
    assert_eq!(
        client.ok("inspect_document", json!({}))["state_id"],
        before["state_id"]
    );
    client.ok("history", json!({"redo":true}));
    assert_eq!(
        client.ok("inspect_document", json!({}))["state_id"],
        result["state_id"]
    );
    let filter = result["filter_aliases"]["tone"]["filter_id"].clone();
    let removal =
        json!({"op":"filter.mask.remove","layer_ref":{"id":layer},"filter_ref":{"id":filter}});
    let removed = client.ok("apply_operations", json!({"operations":[removal]}));
    receipt(&removed, 1);
    record("removed-result-v1", &removed);
    assert!(
        client
            .ok("inspect_document", json!({}))
            .get("schema")
            .is_none()
    );
    assert_eq!(fs::read(&input).unwrap(), original);
}
#[test]
fn shipped_mask_profiles_export_exact_closed_json_and_legacy_grammar_stays_closed() {
    for (name, expected) in [
        ("batch-v3", include_str!("../schemas/batch-v3.schema.json")),
        (
            "result-v3",
            include_str!("../schemas/result-v3.schema.json"),
        ),
        (
            "file-command-v3",
            include_str!("../schemas/file-command-v3.schema.json"),
        ),
        (
            "inspect-v2",
            include_str!("../schemas/inspect-v2.schema.json"),
        ),
        (
            "mcp-inspect-v2",
            include_str!("../schemas/mcp-inspect-v2.schema.json"),
        ),
    ] {
        let schema = cli(&["schema", name]);
        assert_eq!(schema, serde_json::from_str::<Value>(expected).unwrap());
        assert_eq!(schema["additionalProperties"], false);
    }
    for version in [1, 2] {
        let doc = paintcraft_document::new_document(1, 1, Some(Rgba([0; 4])), 0, POLICY).unwrap();
        let packet = json!({"schema":format!("paintcraft.batch/v{version}"),"atomic":true,"policy":POLICY,"base":{"state_id":doc.state_id.to_string(),"publication_epoch":"0"},"operations":[mask_operation(&doc.roots[0].to_string(),json!({"id":uuid::Uuid::nil().to_string()}))]});
        assert_eq!(
            paintcraft_commands::Batch::from_json(&serde_json::to_vec(&packet).unwrap())
                .unwrap_err()
                .code,
            paintcraft_core::types::ErrorCode::InputSchema
        );
    }
}

// Synthetic native-capacity specimen: no original BMP/PNG provenance claim.
// Canonical PCND4 metadata: base44 + one layer29 + source flag1 + filter
// count1 + Invert instance18 + stored-zero mask8 + attachment header192.
fn exact_masked_metadata_cap() -> Document {
    use paintcraft_core::types::{Rect, filters::Descriptor};
    use paintcraft_document::{
        FilterInstance, FilterMask, FilterStack, SourceMetadata, source_metadata::BYTES_MAX,
    };
    use paintcraft_raster::coverage_mask::{A8Mask, Limits};
    let mut doc =
        paintcraft_document::new_document(2, 2, Some(Rgba([255; 4])), 73, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.name = "x".into();
    let zero = A8Mask::from_bands(
        Rect::new(0, 0, 2, 2).unwrap(),
        &[],
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    layer.filters = FilterStack::new(vec![
        FilterInstance::new(uuid::Uuid::from_u128(1), true, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(true, zero))),
    ])
    .unwrap();
    assert_eq!(pcnd::write_compact(&doc).unwrap().len(), 101 + 32);
    let specimen = pcnd::read(include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap()
    .source_metadata
    .unwrap();
    let mut profile = specimen.profile().unwrap().to_vec();
    let size = BYTES_MAX as usize - 101 - 192;
    profile.resize(size, 0);
    profile[..4].copy_from_slice(&(size as u32).to_be_bytes());
    let mut descriptor = *specimen.descriptor();
    descriptor.source_dimensions = (2, 2);
    descriptor.output_dimensions = (2, 2);
    let source = SourceMetadata::capture(
        descriptor,
        Some(&profile),
        BYTES_MAX,
        &SharedBudget::new(16 << 20),
        &|| Ok(()),
    )
    .unwrap();
    let doc = doc
        .with_source_metadata_checked(Some(source), &|| Ok(()))
        .unwrap();
    assert_eq!(
        pcnd::write_compact(&doc).unwrap().len(),
        BYTES_MAX as usize + 32
    );
    doc
}
#[test]
fn real_cli_stdio_mask_growth_at_exact_cap_preserves_source_destination_and_history() {
    let folder = Folder::new();
    let doc = exact_masked_metadata_cap();
    let input = folder.0.join("cap.pcnd");
    let destination = folder.0.join("sentinel.pcnd");
    let original = pcnd::write_compact(&doc).unwrap();
    fs::write(&input, &original).unwrap();
    fs::write(&destination, b"unchanged destination").unwrap();
    let operation = json!({"op":"filter.mask.set","layer_ref":{"id":doc.roots[0].to_string()},"filter_ref":{"id":uuid::Uuid::from_u128(1).to_string()},"mask":{"domain":{"x":0,"y":0,"width":2,"height":2},"bands":[{"y0":0,"y1":1,"runs":[{"x0":0,"x1":1,"coverage":255}]}]}});
    let packet = json!({"schema":"paintcraft.batch/v3","atomic":true,"policy":POLICY,"base":{"state_id":doc.state_id.to_string(),"publication_epoch":"0"},"operations":[operation]});
    let operations = folder.0.join("operations.json");
    fs::write(&operations, serde_json::to_vec(&packet).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("apply")
        .arg(&input)
        .arg("--operations")
        .arg(operations)
        .arg("-o")
        .arg(&destination)
        .args(["--overwrite", "--json"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("resource.limit"),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(&destination).unwrap(), b"unchanged destination");
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("open_project", json!({"path":input}));
    let before = client.ok("inspect_document", json!({}));
    let refused = client.call("apply_operations", json!({"operations":[operation]}));
    assert_eq!(refused["isError"], true);
    assert!(refused.to_string().contains("resource.limit"), "{refused}");
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&input).unwrap(), original);
}
