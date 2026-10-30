//! Genuine CLI/stdio consumers compared with the independent proposed-profile oracle.
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY};
use paintcraft_formats::{image, pcnd};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
const ORACLE: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-effects/tests/point-fixtures/oracle.json");
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/point-profile-consumers")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn oracle() -> Value {
    assert_eq!(
        paintcraft_document::hex(&Sha256::digest(ORACLE)),
        "be91b91d90b86f06183afe0ed760ddff14f7e75855344257546a9bb4473e0c94"
    );
    serde_json::from_slice(ORACLE).unwrap()
}
fn record(name: &str, value: &Value) {
    if let Some(root) = std::env::var_os("PAINTCRAFT_POINT_RECEIPTS") {
        let path = PathBuf::from(root);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn rgba(value: &Value) -> [u8; 4] {
    std::array::from_fn(|c| value[c].as_u64().unwrap().try_into().unwrap())
}
fn descriptor(row: &Value) -> Value {
    let kind = row["kind"].as_str().unwrap();
    match kind {
        "black-white-rgb-v1" => json!({"kind":kind,"strength":row["parameter"]}),
        "threshold-luma-v1" => json!({"kind":kind,"level":row["parameter"]}),
        "posterize-rgb-v1" => json!({"kind":kind,"levels":row["parameter"]}),
        "solarize-rgb-v1" => json!({"kind":kind}),
        _ => panic!("independent fixture kind"),
    }
}
fn source(index: usize, pixel: [u8; 4]) -> (Document, Vec<u8>) {
    let mut raw = pixel.to_vec();
    raw.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    let doc = if pixel[3] == 255 {
        // Exercise implicit source defaults with no allocated raster tiles.
        let doc =
            paintcraft_document::new_document(2, 1, Some(Rgba(pixel)), index as u64 + 700, POLICY)
                .unwrap();
        assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
        doc
    } else {
        paintcraft_document::import_image(
            2,
            1,
            &raw,
            &paintcraft_document::hex(&Sha256::digest(&raw)),
        )
        .unwrap()
    };
    (doc, raw)
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
fn add(layer: &str, desc: &Value) -> Value {
    json!({"op":"filter.add","layer_ref":{"id":layer},"ref":"tone","descriptor":desc})
}
fn packet(doc: &Document, operations: Value) -> Value {
    json!({"schema":"paintcraft.batch/v4","atomic":true,"policy":POLICY,"base":{"state_id":doc.state_id.to_string(),"publication_epoch":"0"},"operations":operations})
}
// Composite output at alpha zero intentionally hides RGB. The opaque companion
// exposes the oracle kernel result; saved native rasters separately preserve
// every original hidden channel. This is not a raw-layer appearance API.
fn expected_plane(expected: [u8; 4]) -> Vec<u8> {
    let mut visible = if expected[3] == 0 {
        vec![0; 4]
    } else {
        expected.to_vec()
    };
    visible.extend_from_slice(&[expected[0], expected[1], expected[2], 255]);
    visible
}
fn check_project(doc: &Document, original: &Document, desc: &Value, expected: &[u8]) {
    assert_eq!(
        doc.layers[&doc.roots[0]].raster,
        original.layers[&original.roots[0]].raster
    );
    let filter = &doc.layers[&doc.roots[0]].filters.as_slice()[0];
    assert_eq!(
        serde_json::from_slice::<Value>(&paintcraft_core::types::json::canonical(
            &filter.descriptor().to_value()
        ))
        .unwrap(),
        *desc
    );
    assert_eq!(paintcraft_render::render_flat(doc).unwrap(), expected);
}
#[test]
fn actual_cli_stdio_all_literal_profiles_keep_sources_ids_and_exact_copies() {
    let folder = Folder::new();
    let oracle = oracle();
    for (index, row) in oracle["literal_controls"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let desc = descriptor(row);
        let (original, _) = source(index, rgba(&row["rgba"]));
        let expected = expected_plane(rgba(&row["expected"]));
        let input = folder.0.join(format!("{index}-input.pcnd"));
        let output = folder.0.join(format!("{index}-cli.pcnd"));
        let bytes = pcnd::write_compact(&original).unwrap();
        fs::write(&input, &bytes).unwrap();
        let layer = original.roots[0].to_string();
        let ops = json!([add(&layer, &desc)]);
        let batch = packet(&original, ops.clone());
        let operations = folder.0.join(format!("{index}-ops.json"));
        fs::write(&operations, serde_json::to_vec(&batch).unwrap()).unwrap();
        let report = success(run(&[
            "apply",
            input.to_str().unwrap(),
            "--operations",
            operations.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]));
        assert_eq!(report["schema"], "paintcraft.file-command/v2");
        assert_eq!(report["engine"]["schema"], "paintcraft.result/v2");
        assert!(report["engine"].get("document_features").is_none());
        let doc = read(&fs::read(&output).unwrap());
        assert_eq!(
            pcnd::probe(&fs::read(&output).unwrap())
                .unwrap()
                .container_version,
            3
        );
        check_project(&doc, &original, &desc, &expected);
        let info = success(run(&["inspect", output.to_str().unwrap(), "--json"]));
        assert_eq!(info["schema"], "paintcraft.inspect/v3");
        assert!(info.get("document_features").is_none());
        let png = folder.0.join(format!("{index}-cli.png"));
        if index == 0 {
            let refused = run(&[
                "render",
                output.to_str().unwrap(),
                "-o",
                png.to_str().unwrap(),
                "--json",
            ]);
            assert!(!refused.status.success());
            assert!(!png.exists());
            let refusal: Value = serde_json::from_slice(&refused.stdout).unwrap();
            assert_eq!(refusal["engine"]["code"], "capability.unsupported");
            assert!(
                refusal["engine"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("explicit consent to bake")
            );
        }
        success(run(&[
            "render",
            output.to_str().unwrap(),
            "-o",
            png.to_str().unwrap(),
            "--allow-loss",
            "filters-baked,hidden-rgb",
            "--json",
        ]));
        assert_eq!(
            image::decode(&fs::read(png).unwrap()).unwrap().rgba,
            expected
        );
        let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        client.ok("open_project", json!({"path":input}));
        let result = client.ok("apply_operations", json!({"operations":ops}));
        assert_eq!(result["schema"], "paintcraft.result/v2");
        assert_eq!(result["state_id"], report["engine"]["state_id"]);
        assert_eq!(result["filter_aliases"], report["engine"]["filter_aliases"]);
        let inspect = client.ok("inspect_document", json!({}));
        assert_eq!(inspect["schema"], "paintcraft.mcp-inspect/v3");
        let saved = folder.0.join(format!("{index}-mcp.pcnd"));
        client.ok("save_project", json!({"path":saved}));
        let mcp_doc = read(&fs::read(saved).unwrap());
        assert!(mcp_doc.same_semantics(&doc));
        check_project(&mcp_doc, &original, &desc, &expected);
        let snapshot = client.ok("inspect_document", json!({}));
        if index == 0 {
            let refused = client.call("export_image", json!({"path":folder.0.join("refused.png")}));
            assert_eq!(refused["isError"], true);
            assert!(refused.to_string().contains("capability.unsupported"));
            assert!(refused.to_string().contains("explicit consent to bake"));
        }
        let png = folder.0.join(format!("{index}-mcp.png"));
        client.ok(
            "export_image",
            json!({"path":png,"allow_losses":["filters-baked","hidden-rgb"]}),
        );
        assert_eq!(
            image::decode(&fs::read(png).unwrap()).unwrap().rgba,
            expected
        );
        assert_eq!(client.ok("inspect_document", json!({})), snapshot);
        client.ok("history", json!({}));
        assert_eq!(
            client.ok("inspect_document", json!({}))["state_id"],
            original.state_id.to_string()
        );
        client.ok("history", json!({"redo":true}));
        assert_eq!(
            client.ok("inspect_document", json!({}))["state_id"],
            result["state_id"]
        );
        if index == 0 {
            record("batch-v4", &batch);
            record("envelope-v2", &report);
            record("result-v2", &result);
            record("inspect-v3-unmasked", &info);
            record("mcp-inspect-v3-unmasked", &inspect);
        }
        assert_eq!(fs::read(input).unwrap(), bytes);
    }
}
#[test]
fn final_stored_kinds_and_masks_select_inspection_without_changing_result_profiles() {
    let folder = Folder::new();
    let (doc, _) = source(90, [255, 0, 0, 255]);
    let input = folder.0.join("source.pcnd");
    fs::write(&input, pcnd::write_compact(&doc).unwrap()).unwrap();
    let layer = doc.roots[0].to_string();
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("open_project", json!({"path":input}));
    let desc = json!({"kind":"black-white-rgb-v1","strength":100});
    let added = client.ok(
        "apply_operations",
        json!({"operations":[add(&layer,&desc)]}),
    );
    let id = added["filter_aliases"]["tone"]["filter_id"].clone();
    let before = client.ok("inspect_document", json!({}));
    let noop=client.ok("apply_operations",json!({"operations":[{"op":"filter.update","layer_ref":{"id":layer},"filter_ref":{"id":id},"descriptor":desc}]}));
    assert_eq!(noop["schema"], "paintcraft.result/v1");
    assert_eq!(noop["committed"], false);
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let disabled=client.ok("apply_operations",json!({"operations":[{"op":"filter.set_enabled","layer_ref":{"id":layer},"filter_ref":{"id":id},"enabled":false}]}));
    assert_eq!(disabled["schema"], "paintcraft.result/v1");
    let info = client.ok("inspect_document", json!({}));
    assert_eq!(info["schema"], "paintcraft.mcp-inspect/v3");
    assert_eq!(info["layers"][0]["filters"][0]["enabled"], false);
    let disabled_path = folder.0.join("disabled.pcnd");
    client.ok("save_project", json!({"path":disabled_path}));
    let disabled_info = success(run(&["inspect", disabled_path.to_str().unwrap(), "--json"]));
    assert_eq!(disabled_info["schema"], "paintcraft.inspect/v3");
    assert_eq!(
        disabled_info["document"]["layers"][0]["filters"][0]["enabled"],
        false
    );
    record("inspect-v3-disabled", &disabled_info);
    record("mcp-inspect-v3-disabled", &info);
    client.ok("apply_operations",json!({"operations":[{"op":"filter.mask.set","layer_ref":{"id":layer},"filter_ref":{"id":id},"mask":{"domain":{"x":0,"y":0,"width":2,"height":1},"bands":[{"y0":0,"y1":1,"runs":[{"x0":0,"x1":2,"coverage":128}]}]}}]}));
    let info = client.ok("inspect_document", json!({}));
    assert_eq!(info["schema"], "paintcraft.mcp-inspect/v3");
    record("mcp-inspect-v3-masked", &info);
    let saved = folder.0.join("masked.pcnd");
    client.ok("save_project", json!({"path":saved}));
    let info = success(run(&["inspect", saved.to_str().unwrap(), "--json"]));
    assert_eq!(info["schema"], "paintcraft.inspect/v3");
    record("inspect-v3-masked", &info);
    let updated=client.ok("apply_operations",json!({"operations":[{"op":"filter.update","layer_ref":{"id":layer},"filter_ref":{"id":id},"descriptor":{"kind":"invert-rgb-v1"}}]}));
    assert_eq!(updated["schema"], "paintcraft.result/v3");
    let info = client.ok("inspect_document", json!({}));
    assert_eq!(info["schema"], "paintcraft.mcp-inspect/v2");
    record("legacy-mcp-inspect-v2", &info);
    record("legacy-result-v3", &updated);
    let removed=client.ok("apply_operations",json!({"operations":[{"op":"filter.mask.remove","layer_ref":{"id":layer},"filter_ref":{"id":id}}]}));
    assert_eq!(removed["schema"], "paintcraft.result/v1");
    assert!(
        client
            .ok("inspect_document", json!({}))
            .get("schema")
            .is_none()
    );
    record("legacy-result-v1", &removed);
}
#[test]
fn additional_schema_exports_are_exact_and_legacy_batch_profiles_reject_new_kinds() {
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    assert_eq!(
        client.ok("get_batch_schema", json!({})),
        serde_json::from_str::<Value>(include_str!("../schemas/batch-v4.schema.json")).unwrap()
    );
    for (name, expected) in [
        ("batch-v4", include_str!("../schemas/batch-v4.schema.json")),
        (
            "inspect-v3",
            include_str!("../schemas/inspect-v3.schema.json"),
        ),
        (
            "mcp-inspect-v3",
            include_str!("../schemas/mcp-inspect-v3.schema.json"),
        ),
    ] {
        assert_eq!(
            success(run(&["schema", name])),
            serde_json::from_str::<Value>(expected).unwrap()
        );
    }
    let (doc, _) = source(91, [1, 2, 3, 255]);
    let layer = doc.roots[0].to_string();
    for kind in oracle()["literal_controls"].as_array().unwrap() {
        for version in [2, 3] {
            let mut batch = packet(&doc, json!([add(&layer, &descriptor(kind))]));
            batch["schema"] = json!(format!("paintcraft.batch/v{version}"));
            assert_eq!(
                paintcraft_commands::Batch::from_json(&serde_json::to_vec(&batch).unwrap())
                    .unwrap_err()
                    .code,
                paintcraft_core::types::ErrorCode::InputSchema
            );
        }
    }
}

#[test]
fn actual_masked_new_kinds_match_independent_fractional_specimens_on_implicit_sources() {
    let folder = Folder::new();
    let oracle = oracle();
    let samples = oracle["context"]["mask_samples"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["rgba"][3] == 255 && row["coverage"] == 128)
        .collect::<Vec<_>>();
    assert_eq!(samples.len(), 4);
    for (index, row) in samples.into_iter().enumerate() {
        let desc = descriptor(row);
        let (original, _) = source(200 + index, rgba(&row["rgba"]));
        let input = folder.0.join(format!("mask-{index}-input.pcnd"));
        let output = folder.0.join(format!("mask-{index}-cli.pcnd"));
        let source_bytes = pcnd::write_compact(&original).unwrap();
        fs::write(&input, &source_bytes).unwrap();
        let layer = original.roots[0].to_string();
        let ops = json!([add(&layer,&desc),{"op":"filter.mask.set","layer_ref":{"id":layer},"filter_ref":{"ref":"tone"},"mask":{"domain":{"x":0,"y":0,"width":2,"height":1},"bands":[{"y0":0,"y1":1,"runs":[{"x0":0,"x1":1,"coverage":128}]}]}}]);
        let batch = packet(&original, ops.clone());
        let operations = folder.0.join(format!("mask-{index}-operations.json"));
        fs::write(&operations, serde_json::to_vec(&batch).unwrap()).unwrap();
        let report = success(run(&[
            "apply",
            input.to_str().unwrap(),
            "--operations",
            operations.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]));
        assert_eq!(report["schema"], "paintcraft.file-command/v3");
        assert_eq!(report["engine"]["schema"], "paintcraft.result/v3");
        let doc = read(&fs::read(&output).unwrap());
        assert_eq!(
            pcnd::probe(&fs::read(&output).unwrap())
                .unwrap()
                .container_version,
            4
        );
        let mut expected = rgba(&row["expected"]).to_vec();
        expected.extend_from_slice(&rgba(&row["rgba"]));
        check_project(&doc, &original, &desc, &expected);
        let info = success(run(&["inspect", output.to_str().unwrap(), "--json"]));
        assert_eq!(info["schema"], "paintcraft.inspect/v3");
        let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        client.ok("open_project", json!({"path":input}));
        let result = client.ok("apply_operations", json!({"operations":ops}));
        assert_eq!(result, report["engine"]);
        let inspect = client.ok("inspect_document", json!({}));
        assert_eq!(inspect["schema"], "paintcraft.mcp-inspect/v3");
        let png = folder.0.join(format!("mask-{index}.png"));
        client.ok(
            "export_image",
            json!({"path":png,"allow_losses":["filters-baked"]}),
        );
        assert_eq!(
            image::decode(&fs::read(png).unwrap()).unwrap().rgba,
            expected
        );
        assert_eq!(fs::read(&input).unwrap(), source_bytes);
        record(&format!("mask-{index}-batch-v4"), &batch);
        record(&format!("mask-{index}-result-v3"), &result);
        record(&format!("mask-{index}-envelope-v3"), &report);
        record(&format!("mask-{index}-inspect-v3"), &info);
        record(&format!("mask-{index}-mcp-inspect-v3"), &inspect);
    }
}

#[test]
fn actual_cross_channel_stage_order_rounds_before_the_next_filter() {
    let folder = Folder::new();
    let specimen = oracle();
    let order = &specimen["context"]["order"];
    for (index, descriptors) in [
        [
            json!({"kind":"black-white-rgb-v1","strength":100}),
            json!({"kind":"threshold-luma-v1","level":128}),
        ],
        [
            json!({"kind":"threshold-luma-v1","level":128}),
            json!({"kind":"black-white-rgb-v1","strength":100}),
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let mut pixel = rgba(&order["rgba"]);
        pixel[3] = 255;
        let (doc, _) = source(300 + index, pixel);
        let input = folder.0.join(format!("order-{index}-input.pcnd"));
        let output = folder.0.join(format!("order-{index}.pcnd"));
        let original = pcnd::write_compact(&doc).unwrap();
        fs::write(&input, &original).unwrap();
        let layer = doc.roots[0].to_string();
        let ops=Value::Array(descriptors.into_iter().enumerate().map(|(at,desc)|json!({"op":"filter.add","layer_ref":{"id":layer},"ref":format!("stage-{at}"),"descriptor":desc})).collect());
        let batch = packet(&doc, ops.clone());
        let operation = folder.0.join(format!("order-{index}.json"));
        fs::write(&operation, serde_json::to_vec(&batch).unwrap()).unwrap();
        let report = success(run(&[
            "apply",
            input.to_str().unwrap(),
            "--operations",
            operation.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]));
        let mut expected = rgba(
            &order[if index == 0 {
                "black_white100_then_threshold128"
            } else {
                "direct_threshold128"
            }],
        );
        expected[3] = 255;
        let expected = expected.repeat(2);
        assert_eq!(
            paintcraft_render::render_flat(&read(&fs::read(&output).unwrap())).unwrap(),
            expected
        );
        let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        client.ok("open_project", json!({"path":input}));
        let result = client.ok("apply_operations", json!({"operations":ops}));
        assert_eq!(result, report["engine"]);
        let png = folder.0.join(format!("order-{index}.png"));
        client.ok(
            "export_image",
            json!({"path":png,"allow_losses":["filters-baked"]}),
        );
        assert_eq!(
            image::decode(&fs::read(png).unwrap()).unwrap().rgba,
            expected
        );
        assert_eq!(fs::read(input).unwrap(), original);
    }
}
