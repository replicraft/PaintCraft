use super::*;
use paintcraft_formats::pcnd;
use std::{fs, path::Path};
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
#[test]
fn explicit_svg_options_and_consent_preserve_failure_state_and_source_aliases() {
    let dir = std::env::temp_dir().join(format!("paintcraft-svg-mcp-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("scene.svg");
    let output = dir.join("scene.pcnd");
    let bytes =
        fs::read(root().join("crates/services/paintcraft-formats/tests/svg-fixtures/aligned.svg"))
            .unwrap();
    fs::write(&source, &bytes).unwrap();
    let mut c = Client::new();
    c.ok("create_canvas", json!({"width":4,"height":3}));
    let before = c.ok("inspect_document", json!({}));
    for request in [
        json!({"path":source,"discard_changes":true,"allow_losses":["svg-rasterized"]}),
        json!({"path":source,"svg":{"width":64,"height":64},"allow_losses":["svg-rasterized"]}),
        json!({"path":source,"discard_changes":true,"svg":{"width":64,"height":64}}),
        json!({"path":source,"discard_changes":true,"svg":{"width":64,"height":64,"dpi":0},"allow_losses":["svg-rasterized"]}),
        json!({"path":source,"discard_changes":true,"svg":{"width":64,"height":64},"page":0,"allow_losses":["svg-rasterized"]}),
        json!({"path":source,"discard_changes":true,"svg":{"width":64,"height":64},"allow_losses":["made-up"]}),
    ] {
        assert_eq!(c.call("import_image", request)["isError"], true);
        assert_eq!(c.ok("inspect_document", json!({})), before);
    }
    let imported=c.ok("import_image",json!({"path":source,"discard_changes":true,"svg":{"width":64,"height":64},"allow_losses":["svg-rasterized"]}));
    assert_eq!(
        (imported["width"].as_u64(), imported["height"].as_u64()),
        (Some(64), Some(64))
    );
    assert_eq!(imported["dirty"], true);
    super::foreign::protect_source(&mut c, &source, &bytes, &[]);
    c.ok("save_project", json!({"path":output}));
    let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.get(0, 0).0, [0; 4]);
    assert_eq!(layer.raster.get(0, 16).0, [255, 0, 0, 255]);
    assert_eq!(layer.raster.get(63, 47).0, [0, 0, 255, 128]);
    assert_eq!(fs::read(&source).unwrap(), bytes);
    drop(c);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn supplied_cjk_font_is_used_and_remains_protected_after_project_save() {
    let dir = std::env::temp_dir().join(format!("paintcraft-svg-font-mcp-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("text.svg");
    let font = dir.join("japanese.otf");
    let output = dir.join("text.pcnd");
    fs::copy(
        root().join("assets/fonts/noto-sans-jp/NotoSansJP-Regular.otf"),
        &font,
    )
    .unwrap();
    let font_bytes = fs::read(&font).unwrap();
    fs::write(&source,"<svg xmlns='http://www.w3.org/2000/svg' width='96' height='32'><text x='0' y='24' font-family='Noto Sans JP' font-size='22'>日本語</text></svg>").unwrap();
    let mut c = Client::new();
    c.ok("create_canvas", json!({"width":4,"height":3}));
    let before = c.ok("inspect_document", json!({}));
    let mut request = json!({"path":source,"discard_changes":true,"svg":{"width":96,"height":32},"allow_losses":["svg-rasterized","svg-font-substituted"]});
    assert_eq!(c.call("import_image", request.clone())["isError"], true);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    request["svg"]["font_files"] = json!([font]);
    c.ok("import_image", request);
    super::foreign::protect_source(&mut c, &font, &font_bytes, &[]);
    c.ok("save_project", json!({"path":output}));
    super::foreign::protect_source(&mut c, &font, &font_bytes, &[]);
    let alias = dir.join("font-alias.ico");
    fs::hard_link(&font, &alias).unwrap();
    assert_eq!(c.call("export_icons",json!({"path":alias,"overwrite":true,"entries":[{"width":32,"height":32,"encoding":"png"}],"allow_losses":["icon-resized"]}))["isError"],true);
    assert_eq!(fs::read(&font).unwrap(), font_bytes);
    let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    assert!((0..32).any(|y| (0..96).any(|x| layer.raster.get(x, y).0[3] > 0)));
    drop(c);
    fs::remove_dir_all(dir).unwrap();
}
