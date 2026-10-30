//! Real CLI and stdio expose editable filter intentions without baking sources.
use paintcraft_core::types::Rgba;
use paintcraft_formats::pcnd;
use serde_json::json;
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
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/filter-operations")
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
fn filter(layer: String) -> serde_json::Value {
    json!({"op":"filter.add","layer_ref":{"id":layer},"ref":"tone","descriptor":{"kind":"brightness-contrast-v1","brightness":10,"contrast":0}})
}
fn record_receipt(name: &str, value: &serde_json::Value, version: u8) {
    assert_eq!(value["schema"], format!("paintcraft.result/v{version}"));
    if version == 1 {
        assert!(value.get("filter_aliases").is_none());
    } else {
        assert!(!value["filter_aliases"].as_object().unwrap().is_empty());
    }
    if let Some(path) = std::env::var_os("PAINTCRAFT_FILTER_RECEIPTS") {
        let root = PathBuf::from(path);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
fn record_envelope(name: &str, value: &serde_json::Value, version: u8) {
    assert_eq!(
        value["schema"],
        format!("paintcraft.file-command/v{version}")
    );
    if let Some(path) = std::env::var_os("PAINTCRAFT_FILTER_RECEIPTS") {
        fs::write(
            PathBuf::from(path).join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
fn apply_again(
    input: &Path,
    operations: &Path,
    output: &Path,
    state: uuid::Uuid,
    op: serde_json::Value,
) -> std::process::Output {
    fs::write(operations,serde_json::to_vec(&json!({"schema":"paintcraft.batch/v2","atomic":true,"policy":paintcraft_document::POLICY,"base":{"state_id":state.to_string(),"publication_epoch":"0"},"operations":[op]})).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("apply")
        .arg(input)
        .arg("--operations")
        .arg(operations)
        .arg("-o")
        .arg(output)
        .arg("--json")
        .output()
        .unwrap()
}
#[test]
fn actual_cli_apply_inspect_and_stdio_keep_live_filters_sources_and_optional_aliases() {
    let dir = Folder::new();
    let input = dir.0.join("input.pcnd");
    let output = dir.0.join("copy.pcnd");
    let original = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([10, 40, 70, 255])),
        7,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let bytes = pcnd::write_compact(&original).unwrap();
    fs::write(&input, &bytes).unwrap();
    let operations = dir.0.join("ops.json");
    fs::write(&operations,serde_json::to_vec(&json!({"schema":"paintcraft.batch/v2","atomic":true,"policy":paintcraft_document::POLICY,"base":{"state_id":original.state_id.to_string(),"publication_epoch":"0"},"operations":[filter(original.roots[0].to_string())]})).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("apply")
        .arg(&input)
        .arg("--operations")
        .arg(&operations)
        .arg("-o")
        .arg(&output)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    record_receipt("cli-add", &report["engine"], 2);
    record_envelope("envelope-add", &report, 2);
    let filter_id = report["engine"]["filter_aliases"]["tone"]["filter_id"]
        .as_str()
        .unwrap();
    let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        doc.layers[&doc.roots[0]].filters.as_slice()[0]
            .id()
            .to_string(),
        filter_id
    );
    assert_eq!(
        doc.layers[&doc.roots[0]].raster,
        original.layers[&original.roots[0]].raster
    );
    assert_eq!(
        paintcraft_render::render_flat(&doc).unwrap(),
        [36, 66, 96, 255, 36, 66, 96, 255]
    );
    let inspected = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("inspect")
        .arg(&output)
        .arg("--json")
        .output()
        .unwrap();
    assert!(inspected.status.success());
    let info: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert!(info.to_string().contains("brightness-contrast-v1"));
    for (name, source, state, operation, success) in [
        (
            "ordinary",
            input.as_path(),
            original.state_id,
            json!({"op":"selection.clear"}),
            true,
        ),
        (
            "removed",
            output.as_path(),
            doc.state_id,
            json!({"op":"filter.remove","layer_ref":{"id":doc.roots[0].to_string()},"filter_ref":{"id":filter_id}}),
            true,
        ),
        (
            "error",
            input.as_path(),
            original.state_id,
            json!({"op":"filter.add","layer_ref":{"id":original.roots[0].to_string()},"ref":"bad","descriptor":{"kind":"brightness-contrast-v1","brightness":0,"contrast":100}}),
            false,
        ),
    ] {
        let result = apply_again(
            source,
            &operations,
            &dir.0.join(format!("{name}.pcnd")),
            state,
            operation,
        );
        assert_eq!(
            result.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        record_envelope(&format!("envelope-{name}"), &value, 1);
        if success {
            record_receipt(&format!("cli-{name}"), &value["engine"], 1);
        }
    }
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("open_project", json!({"path":input}));
    let before = client.ok("inspect_document", json!({}));
    assert!(before["layers"][0].get("filters").is_none());
    let ordinary = client.ok(
        "apply_operations",
        json!({"operations":[{"op":"selection.clear"}]}),
    );
    record_receipt("ordinary", &ordinary, 1);
    let added = client.ok(
        "apply_operations",
        json!({"operations":[filter(original.roots[0].to_string())]}),
    );
    record_receipt("mcp-add", &added, 2);
    assert_eq!(added["filter_aliases"]["tone"]["filter_id"], filter_id);
    let info = client.ok("inspect_document", json!({}));
    assert_eq!(
        info["layers"][0]["filters"][0]["descriptor"],
        json!({"kind":"brightness-contrast-v1","brightness":10,"contrast":0})
    );
    assert_eq!(info["layers"][0]["filters"][0]["enabled"], true);
    let noop = client.ok("apply_operations",json!({"operations":[{"op":"filter.update","layer_ref":{"id":original.roots[0].to_string()},"filter_ref":{"id":filter_id},"descriptor":{"kind":"brightness-contrast-v1","brightness":10,"contrast":0}}]}));
    record_receipt("noop", &noop, 1);
    assert_eq!(noop["committed"], false);
    let toggled=client.ok("apply_operations",json!({"operations":[{"op":"filter.set_enabled","layer_ref":{"id":original.roots[0].to_string()},"filter_ref":{"id":filter_id},"enabled":false}]}));
    record_receipt("toggled", &toggled, 1);
    assert_eq!(
        client.ok("inspect_document", json!({}))["layers"][0]["filters"][0]["enabled"],
        false
    );
    client.ok("history", json!({}));
    assert_eq!(
        client.ok("inspect_document", json!({}))["layers"][0]["filters"][0]["enabled"],
        true
    );
    let saved = dir.0.join("mcp.pcnd");
    client.ok("save_project", json!({"path":saved}));
    assert!(
        pcnd::read(&fs::read(saved).unwrap())
            .unwrap()
            .same_semantics(&doc)
    );
    let removed=client.ok("apply_operations",json!({"operations":[{"op":"filter.remove","layer_ref":{"id":original.roots[0].to_string()},"filter_ref":{"id":filter_id}}]}));
    record_receipt("removed", &removed, 1);
    assert_eq!(removed["committed"], true);
    assert!(
        client.ok("inspect_document", json!({}))["layers"][0]
            .get("filters")
            .is_none()
    );
    assert_eq!(fs::read(input).unwrap(), bytes);
}
