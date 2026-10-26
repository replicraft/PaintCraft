use super::*;
#[test]
fn msp_consent_receipt_and_frozen_source_guard_survive_save_rename_and_failed_replacement() {
    let root = std::env::temp_dir().join(format!("paintcraft-mcp-msp-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("legacy.msp");
    let bytes = include_bytes!(
        "../../../../crates/services/paintcraft-formats/tests/msp-fixtures/raw-9.msp"
    );
    std::fs::write(&source, bytes).unwrap();
    let mut client = Client::new();
    client.ok("create_canvas", json!({"width":3,"height":3}));
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":source,"discard_changes":true})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let imported = client.ok(
        "import_image",
        json!({"path":source,"allow_losses":["msp-metadata-ignored"],"discard_changes":true}),
    );
    assert_eq!(imported["width"], 9);
    assert_eq!(imported["undo_depth"], 0);
    assert_eq!(imported["dirty"], true);
    assert_eq!(
        imported["conversion"]["losses_acknowledged"],
        json!(["msp-metadata-ignored"])
    );
    assert_eq!(imported["conversion"]["preserving_save"], false);
    let moved = root.join("moved.msp");
    std::fs::rename(&source, &moved).unwrap();
    let alias = root.join("source.pcnd");
    std::fs::hard_link(&moved, &alias).unwrap();
    let export_alias = root.join("alias.msp");
    std::fs::hard_link(&moved, &export_alias).unwrap();
    let malformed = root.join("bad.msp");
    std::fs::write(&malformed, &bytes[..32]).unwrap();
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":malformed,"allow_losses":["msp-metadata-ignored"],"discard_changes":true})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    for (tool, path) in [("save_project", &alias), ("export_image", &export_alias)] {
        assert_eq!(
            client.call(tool, json!({"path":path,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(std::fs::read(&moved).unwrap(), bytes);
    }
    let saved = root.join("converted.pcnd");
    let receipt = client.ok("save_project", json!({"path":saved}));
    assert_eq!(receipt["source_lineage_adopted"], false);
    let clean = client.ok("inspect_document", json!({}));
    assert_eq!(clean["dirty"], false);
    assert_eq!(
        client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), clean);
    let copy = root.join("fresh.msp");
    let export = client.ok("export_image", json!({"path":copy}));
    assert_eq!(export["losses_acknowledged"], json!([]));
    assert_eq!(
        paintcraft_formats::image::decode(&std::fs::read(copy).unwrap())
            .unwrap()
            .rgba,
        paintcraft_formats::image::decode(bytes).unwrap().rgba
    );
    drop(client);
    std::fs::remove_dir_all(root).unwrap();
}
