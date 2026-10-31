//! Box profiles are selected from stored intent, independently of packet version.
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY};
use paintcraft_formats::pcnd;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("paintcraft-box-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn ok(args: &[&str]) -> Value {
    let out = run(args);
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn record(name: &str, value: &Value) {
    if let Some(root) = std::env::var_os("PAINTCRAFT_BOX_RECEIPTS") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(format!("{name}.json")),
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
                bytes: 8 << 20,
                budget: &SharedBudget::new(8 << 20),
                limits: paintcraft_raster::coverage_mask::Limits::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
fn source(retained: bool) -> Document {
    let doc = paintcraft_document::new_document(2, 1, Some(Rgba([20, 115, 200, 255])), 730, POLICY)
        .unwrap();
    if !retained {
        return doc;
    }
    let attachment = pcnd::read(include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap()
    .source_metadata;
    doc.with_source_metadata_checked(attachment, &|| Ok(()))
        .unwrap()
}
fn packet(doc: &Document, operations: Value) -> Value {
    json!({"schema":"paintcraft.batch/v5","atomic":true,"policy":POLICY,"base":{"state_id":doc.state_id.to_string(),"publication_epoch":"0"},"operations":operations})
}
fn add(layer: &str, alias: &str, radius: u8, enabled: bool) -> Value {
    json!({"op":"filter.add","layer_ref":{"id":layer},"ref":alias,"enabled":enabled,"descriptor":{"kind":"box-blur-v1","radius":radius}})
}
fn mask(layer: &str, alias: &str) -> Value {
    json!({"op":"filter.mask.set","layer_ref":{"id":layer},"filter_ref":{"ref":alias},"mask":{"domain":{"x":0,"y":0,"width":2,"height":1},"bands":[{"y0":0,"y1":1,"runs":[{"x0":0,"x1":1,"coverage":128}]}]}})
}
fn features(masked: bool, retained: bool) -> Value {
    let mut rows = vec!["layer-point-filters.v1"];
    if masked {
        rows.push("layer-filter-masks.v1");
    }
    rows.push("layer-spatial-filters.v1");
    if retained {
        rows.push("source-metadata.v1");
    }
    json!(rows)
}
fn apply(input: &Path, output: &Path, operations: &Path, value: &Value) -> Value {
    fs::write(operations, serde_json::to_vec(value).unwrap()).unwrap();
    ok(&[
        "apply",
        input.to_str().unwrap(),
        "--operations",
        operations.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--json",
    ])
}
#[test]
fn actual_cli_stdio_box_inventory_is_distinct_from_container_grammar() {
    for retained in [false, true] {
        for masked in [false, true] {
            let folder = Folder::new();
            let doc = source(retained);
            let layer = doc.roots[0].to_string();
            let input = folder.0.join("source.pcnd");
            let output = folder.0.join("box.pcnd");
            let operations = folder.0.join("ops.json");
            let original = pcnd::write_compact(&doc).unwrap();
            fs::write(&input, &original).unwrap();
            // Disabled radius-zero is still stored spatial intent; enabled radius32 is not elided either.
            let mut ops = vec![add(&layer, "blur", if masked { 32 } else { 0 }, masked)];
            if masked {
                ops.push(mask(&layer, "blur"));
            }
            let batch = packet(&doc, json!(ops));
            let name = format!("mask{}-source{}", u8::from(masked), u8::from(retained));
            record(&format!("{name}-batch"), &batch);
            let report = apply(&input, &output, &operations, &batch);
            assert_eq!(report["schema"], "paintcraft.file-command/v4");
            assert_eq!(report["engine"]["schema"], "paintcraft.result/v4");
            assert_eq!(
                report["engine"]["document_features"],
                features(masked, retained)
            );
            record(&format!("{name}-envelope"), &report);
            record(&format!("{name}-result"), &report["engine"]);
            let bytes = fs::read(&output).unwrap();
            assert_eq!(pcnd::probe(&bytes).unwrap().container_version, 5);
            let saved = read(&bytes);
            let instance = &saved.layers[&doc.roots[0]].filters.as_slice()[0];
            assert_eq!(instance.enabled(), masked);
            assert_eq!(
                instance.descriptor(),
                paintcraft_core::types::filters::Descriptor::BoxBlurV1(
                    paintcraft_core::types::filters::BoxBlurV1::new(if masked { 32 } else { 0 })
                        .unwrap()
                )
            );
            assert_eq!(instance.mask().is_some(), masked);
            assert_eq!(
                saved.layers[&doc.roots[0]].raster,
                doc.layers[&doc.roots[0]].raster
            );
            assert_eq!(saved.source_metadata, doc.source_metadata);
            let info = ok(&["inspect", output.to_str().unwrap(), "--json"]);
            assert_eq!(info["schema"], "paintcraft.inspect/v4");
            assert_eq!(info["document_features"], features(masked, retained));
            assert_eq!(info["assessment"]["container_version"], 5);
            assert_eq!(
                info["assessment"]["profile"],
                "paintcraft.native/binary5-document5-point-filters1-filter-masks1-spatial-filters1"
            );
            assert_eq!(
                info["assessment"]["document_schema"],
                "paintcraft.document/v5"
            );
            assert!(
                info["assessment"]["required_features"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("layer-filter-masks.v1"))
            );
            record(&format!("{name}-inspect"), &info);
            let mut client =
                mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
            client.ok("open_project", json!({"path":input}));
            let before = client.ok("inspect_document", json!({}));
            let result = client.ok("apply_operations", json!({"operations":ops}));
            assert_eq!(result, report["engine"]);
            record(&format!("{name}-mcp-result"), &result);
            let info = client.ok("inspect_document", json!({}));
            assert_eq!(info["schema"], "paintcraft.mcp-inspect/v4");
            assert_eq!(info["document_features"], features(masked, retained));
            assert_eq!(info["undo_depth"], 1);
            record(&format!("{name}-mcp-inspect"), &info);
            let stdio = folder.0.join("stdio.pcnd");
            client.ok("save_project", json!({"path":stdio}));
            assert!(read(&fs::read(&stdio).unwrap()).same_semantics(&saved));
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
            assert_eq!(fs::read(&input).unwrap(), original);
        }
    }
}
#[test]
fn final_document_not_v5_request_selects_result_and_file_profiles() {
    for (point, masked, expected) in [(false, false, 1), (true, false, 2), (true, true, 3)] {
        let folder = Folder::new();
        let doc = source(false);
        let layer = doc.roots[0].to_string();
        let input = folder.0.join("input.pcnd");
        let output = folder.0.join("out.pcnd");
        let operations = folder.0.join("ops.json");
        fs::write(&input, pcnd::write_compact(&doc).unwrap()).unwrap();
        let mut ops = vec![add(&layer, "temporary", 1, true)];
        if point {
            ops.push(json!({"op":"filter.add","layer_ref":{"id":layer},"ref":"keep","descriptor":{"kind":"invert-rgb-v1"}}));
        }
        if masked {
            ops.push(mask(&layer, "keep"));
        }
        ops.push(
            json!({"op":"filter.remove","layer_ref":{"id":layer},"filter_ref":{"ref":"temporary"}}),
        );
        let report = apply(&input, &output, &operations, &packet(&doc, json!(ops)));
        assert_eq!(
            report["schema"],
            format!("paintcraft.file-command/v{expected}")
        );
        assert_eq!(
            report["engine"]["schema"],
            format!("paintcraft.result/v{expected}")
        );
        assert!(
            !report["engine"]["document_features"]
                .as_array()
                .is_some_and(|v| v.contains(&json!("layer-spatial-filters.v1")))
        );
        if expected != 1 {
            record(
                &format!(
                    "final-point{}-mask{}-envelope",
                    u8::from(point),
                    u8::from(masked)
                ),
                &report,
            );
        }
        let saved = read(&fs::read(&output).unwrap());
        assert!(
            !saved
                .layers
                .values()
                .any(|l| l.filters.iter().any(|f| f.descriptor().is_spatial()))
        );
        assert_eq!(
            pcnd::probe(&fs::read(&output).unwrap())
                .unwrap()
                .container_version,
            if masked {
                4
            } else if point {
                3
            } else {
                1
            }
        );
    }
}
#[test]
fn box_schema_exports_and_invalid_transport_preserve_destination_session() {
    for (name, text) in [
        ("batch-v5", include_str!("../schemas/batch-v5.schema.json")),
        (
            "result-v4",
            include_str!("../schemas/result-v4.schema.json"),
        ),
        (
            "file-command-v4",
            include_str!("../schemas/file-command-v4.schema.json"),
        ),
        (
            "inspect-v4",
            include_str!("../schemas/inspect-v4.schema.json"),
        ),
        (
            "mcp-inspect-v4",
            include_str!("../schemas/mcp-inspect-v4.schema.json"),
        ),
    ] {
        let schema = ok(&["schema", name]);
        assert_eq!(schema, serde_json::from_str::<Value>(text).unwrap());
        assert_eq!(schema["additionalProperties"], false);
    }
    let folder = Folder::new();
    let doc = source(false);
    let input = folder.0.join("input.pcnd");
    let output = folder.0.join("sentinel.pcnd");
    let operations = folder.0.join("ops.json");
    let original = pcnd::write_compact(&doc).unwrap();
    fs::write(&input, &original).unwrap();
    fs::write(&output, b"existing destination").unwrap();
    let mut invalid = add(&doc.roots[0].to_string(), "invalid", 1, true);
    invalid["descriptor"]["radius"] = json!(33);
    fs::write(
        &operations,
        serde_json::to_vec(&packet(&doc, json!([invalid]))).unwrap(),
    )
    .unwrap();
    let out = run(&[
        "apply",
        input.to_str().unwrap(),
        "--operations",
        operations.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--overwrite",
        "--json",
    ]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("input.schema")
            || String::from_utf8_lossy(&out.stderr).contains("input.schema")
    );
    assert_eq!(fs::read(&output).unwrap(), b"existing destination");
    assert_eq!(fs::read(&input).unwrap(), original);
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    assert_eq!(
        client.ok("get_batch_schema", json!({})),
        serde_json::from_str::<Value>(include_str!("../schemas/batch-v5.schema.json")).unwrap()
    );
    client.ok("open_project", json!({"path":input}));
    let before = client.ok("inspect_document", json!({}));
    let error = client.call("apply_operations", json!({"operations":[invalid]}));
    assert_eq!(error["isError"], true);
    assert!(error.to_string().contains("input.schema"));
    assert_eq!(client.ok("inspect_document", json!({})), before);
}

#[test]
fn removing_last_preexisting_box_restores_legacy_result_without_aliases() {
    use paintcraft_core::types::filters::{BoxBlurV1, Descriptor};
    use paintcraft_document::{FilterInstance, FilterStack};
    let folder = Folder::new();
    let mut doc = source(false);
    let layer = doc.roots[0];
    let id = uuid::Uuid::from_u128(89);
    doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        id,
        false,
        Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
    )])
    .unwrap();
    let input = folder.0.join("input.pcnd");
    let output = folder.0.join("out.pcnd");
    let operations = folder.0.join("ops.json");
    fs::write(&input, pcnd::write_compact(&doc).unwrap()).unwrap();
    let report = apply(
        &input,
        &output,
        &operations,
        &packet(
            &doc,
            json!([{"op":"filter.remove","layer_ref":{"id":layer.to_string()},"filter_ref":{"id":id.to_string()}}]),
        ),
    );
    assert_eq!(report["schema"], "paintcraft.file-command/v1");
    assert_eq!(report["engine"]["schema"], "paintcraft.result/v1");
    assert!(report["engine"].get("document_features").is_none());
    assert!(report["engine"].get("filter_aliases").is_none());
    assert_eq!(
        pcnd::probe(&fs::read(&output).unwrap())
            .unwrap()
            .container_version,
        1
    );
}

#[test]
fn real_cli_stdio_box_exports_use_spatial_neighbours_and_explicit_bake_consent() {
    // Independent integer oracle, radius1 finite-source CLAMP, three horizontal
    // samples and three identical clamped rows: red:red:green / red:green:green.
    let original = [255, 0, 0, 255, 0, 255, 0, 255];
    let expected = [170, 85, 0, 255, 85, 170, 0, 255];
    let folder = Folder::new();
    let doc = paintcraft_document::import_image(
        2,
        1,
        &original,
        &paintcraft_document::hex(&Sha256::digest(original)),
    )
    .unwrap();
    let layer = doc.roots[0].to_string();
    let input = folder.0.join("input.pcnd");
    let output = folder.0.join("box.pcnd");
    let operations = folder.0.join("ops.json");
    fs::write(&input, pcnd::write_compact(&doc).unwrap()).unwrap();
    let ops = json!([add(&layer, "blur", 1, true)]);
    apply(&input, &output, &operations, &packet(&doc, ops.clone()));
    let png = folder.0.join("cli.png");
    let refused = run(&[
        "render",
        output.to_str().unwrap(),
        "-o",
        png.to_str().unwrap(),
        "--json",
    ]);
    assert!(!refused.status.success());
    assert!(!png.exists());
    assert!(String::from_utf8_lossy(&refused.stdout).contains("explicit consent to bake"));
    ok(&[
        "render",
        output.to_str().unwrap(),
        "-o",
        png.to_str().unwrap(),
        "--allow-loss",
        "filters-baked",
        "--json",
    ]);
    assert_eq!(
        paintcraft_formats::image::decode(&fs::read(&png).unwrap())
            .unwrap()
            .rgba,
        expected
    );
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("open_project", json!({"path":input}));
    client.ok("apply_operations", json!({"operations":ops}));
    let before = client.ok("inspect_document", json!({}));
    let png = folder.0.join("mcp.png");
    let refused = client.call("export_image", json!({"path":png}));
    assert_eq!(refused["isError"], true);
    assert!(!png.exists());
    client.ok(
        "export_image",
        json!({"path":png,"allow_losses":["filters-baked"]}),
    );
    assert_eq!(
        paintcraft_formats::image::decode(&fs::read(png).unwrap())
            .unwrap()
            .rgba,
        expected
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let saved = read(&fs::read(output).unwrap());
    assert_eq!(
        saved.layers[&doc.roots[0]].raster,
        doc.layers[&doc.roots[0]].raster
    );
}
