use super::*;

#[test]
fn quantized_pcx_requires_palette_loss_consent_through_the_actual_mcp_protocol() {
    let dir = std::env::temp_dir().join(format!("paintcraft-mcp-quantized-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rgba: Vec<u8> = (0..400u32)
        .flat_map(|i| [i as u8, (i >> 8) as u8, i.wrapping_mul(53) as u8, 255])
        .collect();
    let doc = paintcraft_document::import_image(40, 10, &rgba, "mcp-pcx-palette").unwrap();
    let source = dir.join("source.pcnd");
    let original = paintcraft_formats::pcnd::write(&doc).unwrap();
    std::fs::write(&source, &original).unwrap();
    let path = dir.join("quantized.pcx");
    let mut c = Client::new();
    c.ok("open_project", json!({"path":source}));
    let before = c.ok("inspect_document", json!({}));
    for mode in ["indexed-exact", "indexed-256"] {
        assert_eq!(
            c.call("export_image", json!({"path":path,"pcx_encoding":mode}))["isError"],
            true
        );
        assert!(!path.exists());
        assert_eq!(c.ok("inspect_document", json!({})), before);
    }
    c.ok(
        "export_image",
        json!({"path":path,"pcx_encoding":"indexed-256","allow_losses":["palette-quantized"]}),
    );
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(bytes[65], 1);
    let decoded = paintcraft_formats::image::decode(&bytes).unwrap();
    assert_eq!((decoded.width, decoded.height), (40, 10));
    assert_ne!(decoded.rgba, rgba);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(std::fs::read(source).unwrap(), original);
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}

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
    for encoding in ["indexed-exact", "indexed-256"] {
        c.ok("export_image",json!({"path":path,"overwrite":true,"pcx_encoding":encoding,"allow_losses":["alpha-discarded"]}));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes[65], 1);
        assert_eq!(
            paintcraft_formats::image::decode(&bytes).unwrap().rgba,
            [136, 153, 174, 255].repeat(6)
        );
        assert_eq!(c.ok("inspect_document", json!({})), before);
    }
    let invalid = dir.join("wrong.png");
    assert_eq!(
        c.call(
            "export_image",
            json!({"path":invalid,"pcx_encoding":"rgb24"})
        )["isError"],
        true
    );
    assert!(!invalid.exists());
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}
