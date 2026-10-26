use super::*;
use paintcraft_formats::csh;
#[test]
fn shape_tools_exchange_independent_resources_without_mutating_a_painting_or_its_binding() {
    let root = std::env::temp_dir().join(format!("paintcraft-mcp-shapes-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("source.csh");
    let path = root.join("copy.csh");
    let bytes = include_bytes!(
        "../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
    );
    std::fs::write(&input, bytes).unwrap();
    let expected = csh::decode(bytes, &|| Ok(())).unwrap();
    let mut client = Client::new();
    let tools = client.request("tools/list", json!({}));
    for name in ["inspect_shapes", "export_shapes"] {
        assert!(
            tools["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["name"] == name)
        );
    }
    let inspection = client.ok("inspect_shapes", json!({"path":input}));
    assert_eq!(inspection["resource_type"], "shape-library");
    assert_eq!(inspection["shapes"].as_array().unwrap().len(), 5);
    assert_eq!(inspection["shapes"][0]["name"], "形 / Shape 🚀");
    assert_eq!(
        inspection["shapes"][0]["records"][4]["points_i32"],
        json!([16777216, -1, 1, 8388608, 0, -16777216])
    );
    assert_eq!(client.call("inspect_document", json!({}))["isError"], true);
    let written = client.ok("export_shapes", json!({"input":input,"path":path}));
    assert_eq!(written["engine"], inspection);
    assert_eq!(written["file"]["operation"], "export-asset");
    assert!(written["file"]["acknowledged_state_id"].is_null());
    assert_eq!(written["file"]["source_lineage_adopted"], false);
    assert_eq!(
        csh::decode(&std::fs::read(&path).unwrap(), &|| Ok(())).unwrap(),
        expected
    );
    client.ok("create_canvas", json!({"width":8,"height":8}));
    client.ok(
        "paint_stroke",
        json!({"points":[{"x":1,"y":2},{"x":6,"y":2}],"color":"#FF0000FF","size":1}),
    );
    let before = client.ok("inspect_document", json!({}));
    let copy = std::fs::read(&path).unwrap();
    for args in [
        json!({"input":input,"path":input,"overwrite":true}),
        json!({"input":input,"path":path}),
        json!({"input":input,"path":path,"overwrite":true,"expect_input_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}),
    ] {
        assert_eq!(client.call("export_shapes", args)["isError"], true);
        assert_eq!(client.ok("inspect_document", json!({})), before);
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
        assert_eq!(std::fs::read(&path).unwrap(), copy);
    }
    assert_eq!(
        client.ok(
            "inspect_shapes",
            json!({"path":input,"expect_input_sha256":inspection["source_sha256"]})
        ),
        inspection
    );
    client.ok("export_shapes",json!({"input":input,"path":path,"overwrite":true,"expect_input_sha256":inspection["source_sha256"]}));
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let alias = root.join("source-alias.csh");
    std::fs::hard_link(&input, &alias).unwrap();
    assert_eq!(
        client.call(
            "export_shapes",
            json!({"input":input,"path":alias,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(std::fs::read(&alias).unwrap(), bytes);
    let malformed = root.join("broken.csh");
    std::fs::write(&malformed, b"cush").unwrap();
    assert_eq!(
        client.call(
            "export_shapes",
            json!({"input":malformed,"path":path,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(std::fs::read(&path).unwrap(), copy);
    assert_eq!(
        client.call("import_image", json!({"path":input,"discard_changes":true}))["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let project = root.join("painting.pcnd");
    client.ok("save_project", json!({"path":project}));
    let bound = client.ok("inspect_document", json!({}));
    let project_bytes = std::fs::read(&project).unwrap();
    let alias = root.join("project-alias.csh");
    std::fs::hard_link(&project, &alias).unwrap();
    assert_eq!(
        client.call(
            "export_shapes",
            json!({"input":input,"path":alias,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(std::fs::read(&project).unwrap(), project_bytes);
    assert_eq!(client.ok("inspect_document", json!({})), bound);
    client.ok(
        "export_shapes",
        json!({"input":input,"path":path,"overwrite":true}),
    );
    assert_eq!(client.ok("inspect_document", json!({})), bound);
    client.ok("save_project", json!({"path":project}));
    assert_eq!(client.ok("inspect_document", json!({})), bound);
    let picture = root.join("picture.png");
    let png = paintcraft_formats::image::encode(
        paintcraft_formats::image::ImageFormat::Png,
        1,
        1,
        &[17, 51, 93, 255],
        90,
    )
    .unwrap();
    std::fs::write(&picture, &png).unwrap();
    client.ok("import_image", json!({"path":picture}));
    let imported = client.ok("inspect_document", json!({}));
    let alias = root.join("picture-alias.csh");
    std::fs::hard_link(&picture, &alias).unwrap();
    assert_eq!(
        client.call(
            "export_shapes",
            json!({"input":input,"path":alias,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(std::fs::read(&picture).unwrap(), png);
    assert_eq!(client.ok("inspect_document", json!({})), imported);
    drop(client);
    std::fs::remove_dir_all(root).unwrap();
}
