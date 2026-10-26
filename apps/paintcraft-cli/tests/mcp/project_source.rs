use super::{Client, json};
use std::fs;

#[test]
fn actual_mcp_opened_project_identity_survives_rename_and_failed_replacement() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-mcp-source-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&folder).unwrap();
    let path = folder.join("original.pcnd");
    let mut client = Client::at(std::path::Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("create_canvas", json!({"width":4,"height":3}));
    client.ok("save_project", json!({"path":path}));
    client.ok("open_project", json!({"path":path}));
    let moved = folder.join("moved.pcnd");
    fs::rename(&path, &moved).unwrap();
    let bytes = fs::read(&moved).unwrap();
    let alias = folder.join("alias.png");
    fs::hard_link(&moved, &alias).unwrap();
    let unrelated = folder.join("unrelated.png");
    let expected = [255, 255, 255, 255].repeat(12);
    client.ok("export_image", json!({"path":unrelated}));
    assert_eq!(
        paintcraft_formats::image::decode(&fs::read(&unrelated).unwrap())
            .unwrap()
            .rgba,
        expected
    );
    let before = client.ok("inspect_document", json!({}));
    let denied = client.call("export_image", json!({"path":alias,"overwrite":true}));
    assert_eq!(denied["isError"], true);
    assert!(denied.to_string().contains("captured source project"));
    client.ok("export_image", json!({"path":unrelated,"overwrite":true}));
    assert_eq!(
        paintcraft_formats::image::decode(&fs::read(&unrelated).unwrap())
            .unwrap()
            .rgba,
        expected
    );
    for suffix in ["ico", "cur", "icns"] {
        let icon_alias = folder.join(format!("source-alias.{suffix}"));
        fs::hard_link(&moved, &icon_alias).unwrap();
        let mut entry = json!({"width":16,"height":16,"encoding":"png"});
        if suffix == "cur" {
            entry["hotspot"] = json!([0, 0]);
        }
        let (tool, mut request) = if suffix == "icns" {
            (
                "export_icns",
                json!({"types":["ic11"],"allow_losses":["icon-resized"]}),
            )
        } else {
            (
                "export_icons",
                json!({"entries":[entry],"allow_losses":["icon-resized"]}),
            )
        };
        request["path"] = json!(icon_alias);
        request["overwrite"] = json!(true);
        let denied = client.call(tool, request.clone());
        assert_eq!(denied["isError"], true, "{suffix}: {denied}");
        assert!(denied.to_string().contains("captured source project"));
        assert_eq!(fs::read(&icon_alias).unwrap(), bytes);
        request["path"] = json!(folder.join(format!("unrelated.{suffix}")));
        client.ok(tool, request);
    }
    // A failed open/save must not replace or clear the retained source identity.
    assert_eq!(
        client.call("open_project", json!({"path":folder.join("missing.pcnd")}))["isError"],
        true
    );
    assert_eq!(
        client.call(
            "save_project",
            json!({"path":folder.join("missing/copy.pcnd")})
        )["isError"],
        true
    );
    assert_eq!(
        client.call("export_image", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&alias).unwrap(), bytes);
    // A successful native save refreshes the active binding without changing
    // intentional native Save behavior; its renamed source is protected too.
    let saved = folder.join("fresh.pcnd");
    client.ok("save_project", json!({"path":saved}));
    let saved_move = folder.join("fresh-moved.pcnd");
    fs::rename(&saved, &saved_move).unwrap();
    let saved_alias = folder.join("fresh-alias.png");
    fs::hard_link(&saved_move, &saved_alias).unwrap();
    assert_eq!(
        client.call("export_image", json!({"path":saved_alias,"overwrite":true}))["isError"],
        true
    );
    client.ok(
        "create_canvas",
        json!({"width":2,"height":2,"discard_changes":true}),
    );
    client.ok("export_image", json!({"path":alias,"overwrite":true}));
    assert!(fs::read(&alias).unwrap().starts_with(b"\x89PNG"));
    fs::remove_dir_all(folder).unwrap();
}
