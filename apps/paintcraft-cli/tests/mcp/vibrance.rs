//! Actual stdio Vibrance intent and rendered copy boundaries.
use super::catalog::read_catalog;
use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::pcnd;
use std::{
    fs,
    path::{Path, PathBuf},
};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("paintcraft-mcp-vibrance-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn descriptor(amount: i16) -> Value {
    json!({"kind":"vibrance-rgb-v1","amount_hundredths":amount})
}
fn record(name: &str, value: &Value) {
    if let Some(root) = std::env::var_os("PAINTCRAFT_VIBRANCE_RECEIPTS") {
        let root = Path::new(&root);
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn vibrance_stdio_signed_disabled_default_noop_reopen_and_remove_downgrade() {
    let folder = Folder::new();
    for amount in [-100, 0, 100] {
        let input = folder.0.join(format!("source-{amount}.pcnd"));
        let output = folder.0.join(format!("vibrance-{amount}.pcnd"));
        let removed = folder.0.join(format!("removed-{amount}.pcnd"));
        let doc = new_document(2, 1, Some(Rgba([64, 16, 128, 0])), 0, POLICY).unwrap();
        let layer = doc.roots[0].to_string();
        let source = pcnd::write_compact(&doc).unwrap();
        fs::write(&input, &source).unwrap();
        let mut c = Client::new();
        c.ok("open_project", json!({"path":input}));
        let added=c.ok("apply_operations",json!({"operations":[{"op":"filter.add","layer_ref":{"id":layer},"ref":"vib","enabled":false,"descriptor":descriptor(amount)}]}));
        assert_eq!(added["schema"], "paintcraft.result/catalog-v1");
        assert_eq!(added["required_filters"], json!(["vibrance-rgb-v1"]));
        let id = added["filter_aliases"]["vib"]["filter_id"].clone();
        record(&format!("mcp-{amount}-result"), &added);
        let inspected = c.ok("inspect_document", json!({}));
        assert_eq!(inspected["schema"], "paintcraft.mcp-inspect/catalog-v1");
        assert_eq!(
            inspected["layers"][0]["filters"],
            json!([{"id":id,"enabled":false,"descriptor":descriptor(amount)}])
        );
        record(&format!("mcp-{amount}-inspect"), &inspected);
        let noop=c.ok("apply_operations",json!({"operations":[{"op":"filter.update","layer_ref":{"id":layer},"filter_ref":{"id":id},"descriptor":descriptor(amount)}]}));
        assert_eq!(noop["committed"], false);
        assert_eq!(noop["schema"], "paintcraft.result/catalog-v1");
        assert_eq!(c.ok("inspect_document", json!({})), inspected);
        record(&format!("mcp-{amount}-noop-result"), &noop);
        c.ok("save_project", json!({"path":output}));
        let saved = fs::read(&output).unwrap();
        assert_eq!(saved[4], 6);
        assert_eq!(
            read_catalog(&saved).layers[&doc.roots[0]].raster,
            doc.layers[&doc.roots[0]].raster
        );
        c.ok("open_project", json!({"path":output}));
        let reopened = c.ok("inspect_document", json!({}));
        assert_eq!(reopened["state_id"], inspected["state_id"]);
        assert_eq!(reopened["layers"], inspected["layers"]);
        let removed_receipt=c.ok("apply_operations",json!({"operations":[{"op":"filter.remove","layer_ref":{"id":layer},"filter_ref":{"id":id}}]}));
        assert_eq!(removed_receipt["schema"], "paintcraft.result/v1");
        assert!(removed_receipt.get("required_filters").is_none());
        let plain = c.ok("inspect_document", json!({}));
        assert!(plain.get("schema").is_none());
        assert!(plain["layers"][0].get("filters").is_none());
        c.ok("save_project", json!({"path":removed}));
        assert_eq!(fs::read(&removed).unwrap()[4], 1);
        assert_eq!(fs::read(&input).unwrap(), source);
        assert_eq!(fs::read(&output).unwrap(), saved);
    }
}
#[test]
fn vibrance_stdio_active_png_literal_bake_consent_preserves_source_and_history() {
    let folder = Folder::new();
    let input = folder.0.join("source.pcnd");
    let output = folder.0.join("vibrance.pcnd");
    let png = folder.0.join("copy.png");
    let doc = new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY).unwrap();
    let layer = doc.roots[0].to_string();
    let source = pcnd::write_compact(&doc).unwrap();
    fs::write(&input, &source).unwrap();
    let mut c = Client::new();
    c.ok("open_project", json!({"path":input}));
    let added=c.ok("apply_operations",json!({"operations":[{"op":"filter.add","layer_ref":{"id":layer},"ref":"vib","descriptor":descriptor(100)}]}));
    record("mcp-active-result", &added);
    c.ok("save_project", json!({"path":output}));
    let saved = fs::read(&output).unwrap();
    assert_eq!(saved[4], 6);
    let before = c.ok("inspect_document", json!({}));
    record("mcp-active-inspect", &before);
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
    assert_eq!(decoded.rgba, [62, 4, 140, 255, 62, 4, 140, 255]);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&output).unwrap(), saved);
    assert_eq!(fs::read(&input).unwrap(), source);
    assert_eq!(
        read_catalog(&saved).layers[&doc.roots[0]].raster,
        doc.layers[&doc.roots[0]].raster
    );
}
