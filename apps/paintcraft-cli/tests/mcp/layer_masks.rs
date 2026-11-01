//! Actual stdio tools share the live selection, Session and admitted persistence.
use super::*;
use std::fs;
#[test]
fn four_typed_layer_mask_tools_capture_real_selection_and_preserve_undo_save_reopen() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-mcp-own-mask-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&folder).unwrap();
    let project = folder.join("masked.pcnd");
    let mut c = Client::new();
    let tools = c.request("tools/list", json!({}));
    for name in [
        "layer_mask_from_selection",
        "layer_mask_set_enabled",
        "layer_mask_invert",
        "layer_mask_remove",
        "get_layer_appearance_schema",
    ] {
        assert!(
            tools["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["name"] == name)
        );
    }
    let manifest = c.ok("get_layer_appearance_schema", json!({}));
    assert_eq!(
        manifest["schema"],
        "paintcraft.layer-appearance-schema-manifest/v1"
    );
    assert_eq!(
        manifest["profile_contract"]["filter_ids"]
            .as_array()
            .unwrap()
            .len(),
        13
    );
    c.ok("create_canvas", json!({"width":2,"height":1}));
    let initial = c.ok("inspect_document", json!({}));
    let layer = initial["layers"][0]["id"].clone();
    assert_eq!(
        c.call(
            "layer_mask_from_selection",
            json!({"layer_id":layer,"enabled":true})
        )["isError"],
        true
    );
    assert_eq!(c.ok("inspect_document", json!({})), initial);
    c.ok("apply_operations",json!({"operations":[{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}}]}));
    let captured = c.ok(
        "layer_mask_from_selection",
        json!({"layer_id":layer,"enabled":true}),
    );
    assert_eq!(captured["schema"], "paintcraft.result/v5");
    assert_eq!(captured["required_filters"], json!([]));
    let first = c.ok("inspect_document", json!({}));
    assert_eq!(first["schema"], "paintcraft.mcp-inspect/v5");
    assert_eq!(first["layers"][0]["mask"]["enabled"], true);
    // Preview uses work_document and the ordinary actual Rust PNG encoder.
    let preview = c.call("preview", json!({}));
    assert_ne!(preview["isError"], true, "{preview}");
    let image = preview["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["type"] == "image")
        .unwrap();
    assert_eq!(image["mimeType"], "image/png");
    use base64::Engine as _;
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(image["data"].as_str().unwrap())
        .unwrap();
    let decoder = png::Decoder::new(std::io::Cursor::new(&png_bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(
        (info.width, info.height, info.color_type),
        (2, 1, png::ColorType::Rgba)
    );
    assert_eq!(
        &pixels[..info.buffer_size()],
        [255, 255, 255, 255, 0, 0, 0, 0]
    );
    assert_eq!(c.ok("inspect_document", json!({})), first);
    // Destructive native flatten is a separate transaction, never foreign bake.
    let flattened = c.ok("flatten_image", json!({}));
    assert_eq!(flattened["committed"], true);
    assert_eq!(flattened["schema"], "paintcraft.result/v1");
    assert!(
        c.ok("inspect_document", json!({}))["layers"][0]
            .get("mask")
            .is_none()
    );
    let flat_path = folder.join("flattened.pcnd");
    c.ok("save_project", json!({"path":flat_path}));
    let flat_bytes = fs::read(&flat_path).unwrap();
    assert_eq!(flat_bytes[4], 1);
    let flat_doc = paintcraft_formats::pcnd::read(&flat_bytes).unwrap();
    let flat_layer = &flat_doc.layers[&flat_doc.roots[0]];
    assert!(flat_layer.mask.is_none());
    assert!(flat_layer.filters.is_empty());
    assert_eq!(flat_layer.raster.get(0, 0).0, [255, 255, 255, 255]);
    assert_eq!(flat_layer.raster.get(1, 0).0, [0, 0, 0, 0]);
    c.ok("history", json!({}));
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"][0]["mask"],
        first["layers"][0]["mask"]
    );
    let before_foreign = c.ok("inspect_document", json!({}));
    let foreign_path = folder.join("refused.png");
    let refused = c.call(
        "export_image",
        json!({"path":foreign_path,"allow_losses":["filters-baked"]}),
    );
    assert_eq!(refused["isError"], true);
    let refusal: Value =
        serde_json::from_str(refused["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(refusal["code"], "capability.unsupported");
    assert_eq!(
        refusal["message"],
        "Layer masks require a qualified appearance or persistence route"
    );
    assert!(!foreign_path.exists());
    assert_eq!(c.ok("inspect_document", json!({})), before_foreign);
    let digest = first["layers"][0]["mask"]["digest"].clone();
    c.ok(
        "layer_mask_set_enabled",
        json!({"layer_id":layer,"enabled":false}),
    );
    let disabled = c.ok("inspect_document", json!({}));
    assert_eq!(disabled["layers"][0]["mask"]["digest"], digest);
    assert_eq!(disabled["layers"][0]["mask"]["enabled"], false);
    let noop = c.ok(
        "layer_mask_set_enabled",
        json!({"layer_id":layer,"enabled":false}),
    );
    assert_eq!(noop["committed"], false);
    c.ok("layer_mask_invert", json!({"layer_id":layer}));
    let inverted = c.ok("inspect_document", json!({}));
    assert_ne!(inverted["layers"][0]["mask"]["digest"], digest);
    c.ok("history", json!({}));
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"][0]["mask"],
        disabled["layers"][0]["mask"]
    );
    c.ok("history", json!({"redo":true}));
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"][0]["mask"],
        inverted["layers"][0]["mask"]
    );
    c.ok("save_project", json!({"path":project}));
    let source = fs::read(&project).unwrap();
    assert_eq!(source[4], 7);
    c.ok(
        "open_project",
        json!({"path":project,"discard_changes":true}),
    );
    let reopened = c.ok("inspect_document", json!({}));
    assert_eq!(reopened["layers"][0]["mask"], inverted["layers"][0]["mask"]);
    // PCND restores persistent coverage, never the former live Session selection.
    assert_eq!(
        c.call(
            "layer_mask_from_selection",
            json!({"layer_id":layer,"enabled":true})
        )["isError"],
        true
    );
    assert_eq!(c.ok("inspect_document", json!({})), reopened);
    assert_eq!(fs::read(&project).unwrap(), source);
    let removed = c.ok("layer_mask_remove", json!({"layer_id":layer}));
    assert_eq!(removed["schema"], "paintcraft.result/v1");
    assert!(removed.get("required_filters").is_none());
    let finaldoc = c.ok("inspect_document", json!({}));
    assert!(finaldoc["layers"][0].get("mask").is_none());
    c.ok("history", json!({}));
    assert!(
        c.ok("inspect_document", json!({}))["layers"][0]
            .get("mask")
            .is_some()
    );
    c.ok("history", json!({"redo":true}));
    let plain = folder.join("plain.pcnd");
    c.ok("save_project", json!({"path":plain}));
    assert_eq!(fs::read(&plain).unwrap()[4], 1);
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn unknown_numeric_layer_mask_operation_is_atomic_and_does_not_replace_selection() {
    let mut c = Client::new();
    c.ok("create_canvas", json!({"width":2,"height":1}));
    c.ok("apply_operations",json!({"operations":[{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}}]}));
    let before = c.ok("inspect_document", json!({}));
    let layer = before["layers"][0]["id"].clone();
    let failure = c.call(
        "apply_operations",
        json!({"operations":[
            {"op":"layer.set_properties","layer_ref":{"id":layer},"name":"must-not-publish"},
            {"op":"layer.mask.set","layer_ref":{"id":layer},"mask":{"domain":[0,0,2,1],"bands":[]}}
        ]}),
    );
    assert_eq!(failure["isError"], true);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(
        c.ok(
            "layer_mask_from_selection",
            json!({"layer_id":layer,"enabled":true})
        )["schema"],
        "paintcraft.result/v5"
    );
}
