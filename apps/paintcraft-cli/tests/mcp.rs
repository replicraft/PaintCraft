//! Real stdio protocol and pixel/file contracts, without launching a window.
use serde_json::{Value, json};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
impl Client {
    fn new() -> Self {
        Self::at(std::path::Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")))
    }
}

#[test]
fn painting_history_atomicity_preview_and_guarded_files() {
    let folder = std::env::temp_dir().join(format!("paintcraft-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let project = folder.join("painting.pcnd");
    let png = folder.join("painting.png");
    let mut c = Client::at(std::path::Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    let tools = c.request("tools/list", json!({}));
    let tools = tools["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 30);
    assert!(tools.iter().any(|tool| tool["name"] == "inspect_source"));
    assert_eq!(c.call("inspect_document", json!({}))["isError"], true);
    c.ok("create_canvas", json!({"width":32,"height":24}));
    c.ok("paint_stroke",json!({"points":[{"x":2,"y":4},{"x":20,"y":4}],"color":"#FF0000FF","size":1,"brush":"round"}));
    c.ok(
        "draw_shape",
        json!({"kind":"rect","from":{"x":10,"y":10},"to":{"x":20,"y":20},"fill":"#0000FFFF"}),
    );
    let before = c.ok("inspect_document", json!({}));
    let layer = before["layers"][0]["id"].clone();
    let failed = c.call(
        "apply_operations",
        json!({"operations":[
            {"op":"layer.set_properties","layer_ref":{"id":layer},"name":"Changed"},
            {"op":"no.such.operation"}
        ]}),
    );
    assert_eq!(failed["isError"], true);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(
        c.call("create_canvas", json!({"width":1,"height":1}))["isError"],
        true
    );
    let preview = c.call("preview", json!({}));
    assert_eq!(preview["content"][0]["type"], "image");
    use base64::Engine as _;
    let preview = base64::engine::general_purpose::STANDARD
        .decode(preview["content"][0]["data"].as_str().unwrap())
        .unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(preview));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (32, 24));
    assert_eq!(
        &pixels[(4 * 32 + 10) * 4..(4 * 32 + 10) * 4 + 4],
        &[255, 0, 0, 255]
    );
    assert_eq!(
        &pixels[(15 * 32 + 15) * 4..(15 * 32 + 15) * 4 + 4],
        &[0, 0, 255, 255]
    );
    c.ok("history", json!({}));
    assert_eq!(c.ok("inspect_document", json!({}))["undo_depth"], 1);
    c.ok("history", json!({"redo":true}));
    c.ok("save_project", json!({"path":project}));
    assert_eq!(c.ok("inspect_document", json!({}))["dirty"], false);
    c.ok("export_image", json!({"path":png}));
    assert_eq!(c.call("export_image", json!({"path":png}))["isError"], true);
    c.ok("open_project", json!({"path":project}));
    let loaded = c.ok("inspect_document", json!({}));
    assert_eq!(loaded["state_id"], before["state_id"]);
    c.ok("save_project", json!({"path":project}));
    c.ok(
        "apply_operations",
        json!({"operations":[{
            "op":"layer.add","ref":"overlay","name":"Overlay","width":32,"height":24,
            "origin":{"x":0,"y":0},"default_pixel":"#00000000"
        }]}),
    );
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    c.ok("save_project", json!({"path":project}));
    let flattened = folder.join("flattened.png");
    assert_eq!(
        c.call("export_image", json!({"path":flattened}))["isError"],
        true
    );
    assert!(!flattened.exists());
    c.ok(
        "export_image",
        json!({"path":flattened,"allow_losses":["flatten"]}),
    );
    c.ok("open_project", json!({"path":project}));
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let layered = c.ok("inspect_document", json!({}));
    let flattened = c.ok("flatten_image", json!({}));
    assert_eq!(flattened["committed"], true);
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(c.ok("flatten_image", json!({}))["committed"], false);
    c.ok("history", json!({}));
    assert_eq!(
        c.ok("inspect_document", json!({}))["state_id"],
        layered["state_id"]
    );
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"],
        layered["layers"]
    );
    std::fs::write(&project, b"external change").unwrap();
    assert_eq!(
        c.call("save_project", json!({"path":project}))["isError"],
        true
    );
    assert_eq!(std::fs::read(&project).unwrap(), b"external change");
    drop(c);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn image_import_requires_loss_consent_preserves_failures_and_keeps_source_unbound() {
    let folder = std::env::temp_dir().join(format!("paintcraft-mcp-import-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join("source.png");
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[0x12, 0x12, 0x34, 0x34, 0x56, 0x56, 0xff, 0xff])
            .unwrap();
    }
    std::fs::write(&source, &encoded).unwrap();
    let mut c = Client::at(std::path::Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    let original = c.ok("create_canvas", json!({"width":4,"height":3}));
    assert_eq!(
        c.call("import_image", json!({"path":source}))["isError"],
        true
    );
    assert_eq!(c.ok("inspect_document", json!({})), original);
    assert_eq!(
        c.call(
            "import_image",
            json!({"path":source,"discard_changes":true})
        )["isError"],
        true
    );
    assert_eq!(c.ok("inspect_document", json!({})), original);
    assert_eq!(
        c.call(
            "import_image",
            json!({"path":source,"discard_changes":true,"allow_losses":["made-up"]})
        )["isError"],
        true
    );
    assert_eq!(c.ok("inspect_document", json!({})), original);
    let imported = c.ok(
        "import_image",
        json!({"path":source,"discard_changes":true,"allow_losses":["depth-16-reduced-to-8"]}),
    );
    assert_eq!(imported["width"], 1);
    assert_eq!(imported["height"], 1);
    assert_eq!(imported["dirty"], true);
    // Import never grants an implicit overwrite binding to the source picture.
    assert_eq!(
        c.call("save_project", json!({"path":source}))["isError"],
        true
    );
    assert_eq!(std::fs::read(&source).unwrap(), encoded);
    assert_eq!(c.ok("inspect_document", json!({})), imported);
    let project = folder.join("imported.pcnd");
    c.ok("save_project", json!({"path":project}));
    c.ok("open_project", json!({"path":project}));
    let preview = c.call("preview", json!({}));
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(preview["content"][0]["data"].as_str().unwrap())
        .unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut pixels).unwrap();
    assert_eq!(pixels, &[0x12, 0x34, 0x56, 0xff]);
    // Wrong operation and malformed input preserve the existing imported project.
    let before = c.ok("inspect_document", json!({}));
    assert_eq!(
        c.call("open_project", json!({"path":source}))["isError"],
        true
    );
    assert_eq!(
        c.call("import_image", json!({"path":project}))["isError"],
        true
    );
    let malformed = folder.join("broken.png");
    std::fs::write(&malformed, b"\x89PNG\r\n\x1a\ninvalid").unwrap();
    assert_eq!(
        c.call("import_image", json!({"path":malformed}))["isError"],
        true
    );
    assert_eq!(c.ok("inspect_document", json!({})), before);
    drop(c);
    std::fs::remove_dir_all(folder).unwrap();
}

#[path = "mcp/exports.rs"]
mod exports;
#[path = "mcp/foreign.rs"]
mod foreign;
#[path = "mcp/icons.rs"]
mod icons;
#[path = "mcp/palettes.rs"]
mod palettes;
#[path = "mcp/svg.rs"]
mod svg;

#[path = "mcp/shapes.rs"]
mod shapes;

#[path = "mcp/brushes.rs"]
mod brushes;

#[path = "mcp/msp.rs"]
mod msp;

#[path = "mcp/icns.rs"]
mod icns;

#[path = "mcp/project_source.rs"]
mod project_source;

#[path = "mcp/catalog.rs"]
mod catalog;

#[path = "mcp/exposure.rs"]
mod exposure;

#[path = "mcp/curves.rs"]
mod curves;

#[path = "mcp/vibrance.rs"]
mod vibrance;
