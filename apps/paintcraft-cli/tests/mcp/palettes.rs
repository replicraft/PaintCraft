use super::*;
#[test]
fn palette_tools_are_independent_assets_and_preserve_the_painting_session() {
    let root = std::env::temp_dir().join(format!("paintcraft-mcp-palettes-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("source.ase");
    let path = root.join("copy.ase");
    let bytes = include_bytes!(
        "../../../../crates/services/paintcraft-formats/tests/ase-fixtures/mixed.ase"
    );
    std::fs::write(&input, bytes).unwrap();
    let mut client = Client::new();
    let tools = client.request("tools/list", json!({}));
    for name in ["inspect_palette", "export_palette"] {
        assert!(
            tools["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["name"] == name)
        );
    }
    let inspected = client.ok("inspect_palette", json!({"path":input}));
    assert_eq!(inspected["resource_type"], "palette");
    assert_eq!(inspected["swatches"], 6);
    assert_eq!(inspected["entries"][1]["group"], "印刷 / Print");
    assert_eq!(
        inspected["entries"][3]["swatch"]["channels_f32_be"][0],
        "80000000"
    );
    assert_eq!(client.call("inspect_document", json!({}))["isError"], true);
    let first = client.ok("export_palette", json!({"input":input,"path":path}));
    assert_eq!(first["file"]["operation"], "export-asset");
    assert!(first["file"]["acknowledged_state_id"].is_null());
    assert_eq!(first["engine"], inspected);
    client.ok("create_canvas", json!({"width":8,"height":8}));
    client.ok(
        "paint_stroke",
        json!({"points":[{"x":1,"y":2},{"x":6,"y":2}],"color":"#FF0000FF","size":1}),
    );
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.ok("inspect_palette", json!({"path":input})),
        inspected
    );
    for args in [
        json!({"input":input,"path":input,"overwrite":true}),
        json!({"input":input,"path":path}),
        json!({"input":input,"path":path,"overwrite":true,"expect_input_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}),
    ] {
        assert_eq!(client.call("export_palette", args)["isError"], true);
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(client.ok("inspect_document", json!({})), before);
    }
    client.ok("export_palette",json!({"input":input,"path":path,"overwrite":true,"expect_input_sha256":inspected["source_sha256"]}));
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let malformed = root.join("malformed.ase");
    std::fs::write(&malformed, b"ASEF").unwrap();
    assert_eq!(
        client.call(
            "export_palette",
            json!({"input":malformed,"path":path,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(client.ok("inspect_document", json!({})), before);
    #[cfg(unix)]
    {
        let project = root.join("painting.pcnd");
        client.ok("save_project", json!({"path":project}));
        let project_bytes = std::fs::read(&project).unwrap();
        let bound = client.ok("inspect_document", json!({}));
        let alias = root.join("painting-alias.ase");
        std::fs::hard_link(&project, &alias).unwrap();
        assert_eq!(
            client.call(
                "export_palette",
                json!({"input":input,"path":alias,"overwrite":true})
            )["isError"],
            true
        );
        assert_eq!(std::fs::read(&project).unwrap(), project_bytes);
        assert_eq!(client.ok("inspect_document", json!({})), bound);
    }
    drop(client);
    std::fs::remove_dir_all(root).unwrap();
}
