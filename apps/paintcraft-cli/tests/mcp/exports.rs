use super::*;

#[test]
fn pcx_export_requires_alpha_consent_and_preserves_session_state() {
    let dir = std::env::temp_dir().join(format!("paintcraft-mcp-pcx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("picture.pcx");
    let mut c = Client::new();
    c.ok(
        "create_canvas",
        json!({"width":3,"height":2,"background":"#11335D80"}),
    );
    let before = c.ok("inspect_document", json!({}));
    assert_eq!(
        c.call("export_image", json!({"path":path}))["isError"],
        true
    );
    assert!(!path.exists());
    assert_eq!(c.ok("inspect_document", json!({})), before);
    c.ok(
        "export_image",
        json!({"path":path,"allow_losses":["alpha-discarded"]}),
    );
    let bytes = std::fs::read(&path).unwrap();
    let image = paintcraft_formats::image::decode(&bytes).unwrap();
    assert_eq!((image.width, image.height), (3, 2));
    assert_eq!(image.rgba, [136, 153, 174, 255].repeat(6));
    assert_eq!(c.ok("inspect_document", json!({})), before);
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}
