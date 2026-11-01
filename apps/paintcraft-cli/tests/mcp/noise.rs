//! Real stdio Noise intent, preview/export and persistent-history boundaries.
//! Candidate bytes are literals from the independent numerical qualification.
use super::*;
use base64::Engine as _;
use std::{fs, path::PathBuf};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("paintcraft-mcp-noise-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn descriptor(distribution: &str) -> Value {
    json!({"kind":"add-noise-rgb-v1","amount_hundredths":100,
        "seed":65535,"monochrome":false,"distribution":distribution})
}
fn preview(client: &mut Client) -> Vec<u8> {
    let result = client.call("preview", json!({}));
    assert_ne!(result.get("isError"), Some(&json!(true)), "{result}");
    let png = base64::engine::general_purpose::STANDARD
        .decode(result["content"][0]["data"].as_str().unwrap())
        .unwrap();
    let decoded = paintcraft_formats::image::decode(&png).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 1));
    decoded.rgba
}
#[test]
fn noise_stdio_preview_export_noop_history_and_pcnd_keep_original_pixels() {
    let folder = Folder::new();
    for (distribution, expected) in [
        ("uniform", [255, 211, 0, 255, 51, 0, 97, 255]),
        ("gaussian", [47, 139, 0, 255, 0, 4, 46, 255]),
    ] {
        let project = folder.0.join(format!("{distribution}.pcnd"));
        let plain = folder.0.join(format!("{distribution}-plain.pcnd"));
        let png = folder.0.join(format!("{distribution}.png"));
        let mut client = Client::new();
        client.ok(
            "create_canvas",
            json!({"width":2,"height":1,"background":"#401080FF"}),
        );
        let initial = client.ok("inspect_document", json!({}));
        let layer = initial["layers"][0]["id"].clone();
        let added = client.ok(
            "apply_operations",
            json!({"operations":[{
            "op":"filter.add","layer_ref":{"id":layer},"ref":"noise",
            "descriptor":descriptor(distribution)}]}),
        );
        assert_eq!(added["required_filters"], json!(["add-noise-rgb-v1"]));
        let id = added["filter_aliases"]["noise"]["filter_id"].clone();
        let before = client.ok("inspect_document", json!({}));
        assert_eq!(preview(&mut client), expected);
        let refused = client.call("export_image", json!({"path":png}));
        assert_eq!(refused["isError"], true);
        assert!(refused.to_string().contains("explicit consent to bake"));
        assert!(!png.exists());
        client.ok(
            "export_image",
            json!({"path":png,"allow_losses":["filters-baked"]}),
        );
        assert_eq!(
            paintcraft_formats::image::decode(&fs::read(&png).unwrap())
                .unwrap()
                .rgba,
            expected
        );
        assert_eq!(client.ok("inspect_document", json!({})), before);
        let noop = client.ok(
            "apply_operations",
            json!({"operations":[{
            "op":"filter.update","layer_ref":{"id":layer},"filter_ref":{"id":id},
            "descriptor":descriptor(distribution)}]}),
        );
        assert_eq!(noop["committed"], false);
        assert_eq!(client.ok("inspect_document", json!({})), before);
        client.ok("history", json!({}));
        assert_eq!(preview(&mut client), [64, 16, 128, 255].repeat(2));
        client.ok("history", json!({"redo":true}));
        assert_eq!(preview(&mut client), expected);
        client.ok("save_project", json!({"path":project}));
        let saved = fs::read(&project).unwrap();
        assert_eq!(saved[4], 6);
        let restored = super::catalog::read_catalog(&saved);
        assert_eq!(
            restored.layers[&restored.roots[0]].raster.get(0, 0).0,
            [64, 16, 128, 255]
        );
        client.ok(
            "apply_operations",
            json!({"operations":[{
            "op":"filter.remove","layer_ref":{"id":layer},"filter_ref":{"id":id}}]}),
        );
        client.ok("save_project", json!({"path":plain}));
        assert_eq!(fs::read(&plain).unwrap()[4], 1);
        assert_eq!(preview(&mut client), [64, 16, 128, 255].repeat(2));
        client.ok(
            "open_project",
            json!({"path":project,"discard_changes":true}),
        );
        assert_eq!(preview(&mut client), expected);
        assert_eq!(fs::read(&project).unwrap(), saved);
    }
}
