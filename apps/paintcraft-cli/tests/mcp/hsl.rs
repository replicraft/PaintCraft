//! Actual stdio Hue/Saturation intent and rendered copy boundaries.
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
        let p = std::env::temp_dir().join(format!("paintcraft-mcp-hsl-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn descriptor(parameters: [i16; 3]) -> Value {
    json!({"kind":"hue-saturation-rgb-v1","hue_degrees":parameters[0],"saturation_hundredths":parameters[1],"lightness_hundredths":parameters[2]})
}
fn record(name: &str, value: &Value) {
    if let Some(root) = std::env::var_os("PAINTCRAFT_HSL_RECEIPTS") {
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
fn hsl_stdio_signed_disabled_default_noop_reopen_and_remove_downgrade() {
    let folder = Folder::new();
    for (amount, parameters) in [(-180, [-180, 0, 0]), (0, [0, 0, 0]), (180, [180, 0, 0])] {
        let input = folder.0.join(format!("source-{amount}.pcnd"));
        let output = folder.0.join(format!("hsl-{amount}.pcnd"));
        let removed = folder.0.join(format!("removed-{amount}.pcnd"));
        let doc = new_document(2, 1, Some(Rgba([64, 16, 128, 0])), 0, POLICY).unwrap();
        let layer = doc.roots[0].to_string();
        let source = pcnd::write_compact(&doc).unwrap();
        fs::write(&input, &source).unwrap();
        let mut c = Client::new();
        c.ok("open_project", json!({"path":input}));
        let added=c.ok("apply_operations",json!({"operations":[{"op":"filter.add","layer_ref":{"id":layer},"ref":"hsl","enabled":false,"descriptor":descriptor(parameters)}]}));
        assert_eq!(added["schema"], "paintcraft.result/catalog-v1");
        assert_eq!(added["required_filters"], json!(["hue-saturation-rgb-v1"]));
        let id = added["filter_aliases"]["hsl"]["filter_id"].clone();
        record(&format!("mcp-{amount}-result"), &added);
        let inspected = c.ok("inspect_document", json!({}));
        assert_eq!(inspected["schema"], "paintcraft.mcp-inspect/catalog-v1");
        assert_eq!(
            inspected["layers"][0]["filters"],
            json!([{"id":id,"enabled":false,"descriptor":descriptor(parameters)}])
        );
        record(&format!("mcp-{amount}-inspect"), &inspected);
        let noop=c.ok("apply_operations",json!({"operations":[{"op":"filter.update","layer_ref":{"id":layer},"filter_ref":{"id":id},"descriptor":descriptor(parameters)}]}));
        assert_eq!(noop["committed"], false);
        assert_eq!(noop["schema"], "paintcraft.result/catalog-v1");
        assert_eq!(c.ok("inspect_document", json!({})), inspected);
        record(&format!("mcp-{amount}-noop-result"), &noop);
        c.ok("save_project", json!({"path":output}));
        let saved = fs::read(&output).unwrap();
        assert_eq!(saved[4], 6);
        let reopened_doc = read_catalog(&saved);
        let filter = &reopened_doc.layers[&doc.roots[0]].filters.as_slice()[0];
        let (semantic, used) = filter.descriptor().semantic_parameter_bytes();
        assert_eq!(used, 6);
        let expected: Vec<u8> = parameters.into_iter().flat_map(i16::to_be_bytes).collect();
        assert_eq!(&semantic[..used], expected);
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
fn hsl_stdio_active_png_literal_bake_consent_preserves_source_and_state() {
    for (case, parameters, expected_rgb) in [
        ("all", [137, 73, -19], [47, 34, 0]),
        ("negative-half-turn", [-180, 0, 0], [80, 128, 16]),
        ("positive-half-turn", [180, 0, 0], [80, 128, 16]),
    ] {
        let folder = Folder::new();
        let input = folder.0.join("source.pcnd");
        let output = folder.0.join("hsl.pcnd");
        let png = folder.0.join("copy.png");
        let doc = new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY).unwrap();
        let layer = doc.roots[0].to_string();
        let source = pcnd::write_compact(&doc).unwrap();
        fs::write(&input, &source).unwrap();
        let mut c = Client::new();
        c.ok("open_project", json!({"path":input}));
        let added=c.ok("apply_operations",json!({"operations":[{"op":"filter.add","layer_ref":{"id":layer},"ref":"hsl","descriptor":descriptor(parameters)}]}));
        record(&format!("mcp-active-{case}-result"), &added);
        c.ok("save_project", json!({"path":output}));
        let saved = fs::read(&output).unwrap();
        assert_eq!(saved[4], 6);
        let decoded_source = read_catalog(&saved);
        let stored = decoded_source.layers[&doc.roots[0]].filters.as_slice()[0].descriptor();
        let (bytes, length) = stored.semantic_parameter_bytes();
        assert_eq!(length, 6);
        let expected: Vec<u8> = parameters.into_iter().flat_map(i16::to_be_bytes).collect();
        assert_eq!(&bytes[..length], expected);

        let before = c.ok("inspect_document", json!({}));
        record(&format!("mcp-active-{case}-inspect"), &before);
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
        assert_eq!(
            decoded.rgba,
            [expected_rgb[0], expected_rgb[1], expected_rgb[2], 255].repeat(2)
        );
        assert_eq!(c.ok("inspect_document", json!({})), before);
        assert_eq!(fs::read(&output).unwrap(), saved);
        assert_eq!(fs::read(&input).unwrap(), source);
        assert_eq!(
            read_catalog(&saved).layers[&doc.roots[0]].raster,
            doc.layers[&doc.roots[0]].raster
        );
    }
}
