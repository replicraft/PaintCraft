use super::*;
#[test]
fn imported_static_brush_selection_paints_undoes_and_guards_renamed_source_aliases() {
    let root = std::env::temp_dir().join(format!("paintcraft-mcp-abr-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("source.abr");
    let bytes = include_bytes!(
        "../../../../crates/services/paintcraft-formats/tests/abr-fixtures/native-presets.abr"
    );
    std::fs::write(&source, bytes).unwrap();
    let mut client = Client::new();
    let inspect = client.ok("inspect_brushes", json!({"path":source}));
    assert_eq!(inspect["presets"][0]["qualified"], true);
    assert_eq!(client.call("inspect_document", json!({}))["isError"], true);
    client.ok(
        "create_canvas",
        json!({"width":32,"height":32,"background":"#00000000"}),
    );
    let before = client.ok("inspect_document", json!({}));
    client.ok(
        "adopt_brush",
        json!({"path":source,"preset":0,"policy":"native-grid-v1"}),
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(
        client.call(
            "adopt_brush",
            json!({"path":source,"preset":2,"policy":"native-grid-v1"})
        )["isError"],
        true
    );
    assert_eq!(
        client.call(
            "adopt_brush",
            json!({"path":source,"preset":1,"policy":"native-grid-v1"})
        )["isError"],
        true
    );
    client.ok(
        "paint_sampled_stroke",
        json!({"points":[[3,3]],"color":"#123456FF"}),
    );
    let painted = client.ok("inspect_document", json!({}));
    assert_eq!(painted["undo_depth"], 1);
    client.ok("history", json!({}));
    assert_eq!(client.ok("inspect_document", json!({}))["undo_depth"], 0);
    client.ok("history", json!({"redo":true}));
    assert_eq!(
        client.ok("inspect_document", json!({}))["state_id"],
        painted["state_id"]
    );
    let noop = client.ok(
        "paint_sampled_stroke",
        json!({"points":[[3,3]],"color":"#12345600"}),
    );
    assert_eq!(noop["committed"], false);
    let moved = root.join("moved.abr");
    std::fs::rename(&source, &moved).unwrap();
    let alias = root.join("source.pcnd");
    std::fs::hard_link(&moved, &alias).unwrap();
    assert_eq!(
        client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(std::fs::read(&moved).unwrap(), bytes);
    let saved = root.join("painted.pcnd");
    client.ok("save_project", json!({"path":saved}));
    let doc = paintcraft_formats::pcnd::read(&std::fs::read(saved).unwrap()).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.get(3, 2).0, [0x12, 0x34, 0x56, 1]);
    assert_eq!(layer.raster.get(4, 3).a(), 255);
    let before_selection = client.ok("inspect_document", json!({}));
    let converted=client.ok("adopt_brush",json!({"path":moved,"preset":1,"policy":"native-grid-v1","allow_spacing_quantization":true}));
    assert_eq!(converted["spacing_q8"], 77);
    assert_eq!(client.ok("inspect_document", json!({})), before_selection);
    // More than the distinct-source allowance must remain usable when the
    // same captured file/version is reselected for separate real strokes.
    for index in 0..17 {
        client.ok(
            "adopt_brush",
            json!({"path":moved,"preset":0,"policy":"native-grid-v1"}),
        );
        assert_eq!(
            client.ok(
                "paint_sampled_stroke",
                json!({"points":[[index+3,8]],"color":"#123456FF"})
            )["committed"],
            true
        );
    }
    let replacement = root.join("replacement.abr");
    std::fs::write(&replacement, bytes).unwrap();
    client.ok(
        "adopt_brush",
        json!({"path":replacement,"preset":0,"policy":"native-grid-v1"}),
    );
    assert_eq!(
        client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(std::fs::read(&moved).unwrap(), bytes);
    drop(client);
    std::fs::remove_dir_all(root).unwrap();
}
