use super::*;
use paintcraft_formats::{icns, pcnd};
use std::fs;
const SOURCE: &[u8] = include_bytes!(
    "../../../../crates/services/paintcraft-formats/tests/icns-fixtures/modern.icns"
);
#[test]
fn icns_selected_import_is_atomic_reports_density_and_retains_renamed_source_guard() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-mcp-icns-import-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.icns");
    fs::write(&source, SOURCE).unwrap();
    let mut c = Client::new();
    c.ok("create_canvas", json!({"width":3,"height":2}));
    let before = c.ok("inspect_document", json!({}));
    let discovery = c.ok("inspect_icns", json!({"path":source}));
    assert_eq!(discovery["assessment_only"], true);
    assert_eq!(discovery["entries"][6]["tag"], "ic11");
    assert_eq!(discovery["entries"][6]["logical_width"], 16);
    assert_eq!(discovery["entries"][6]["scale"], 2);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    for request in [
        json!({"path":source,"discard_changes":true,"allow_losses":icns::KNOWN_IMPORT_LOSSES}),
        json!({"path":source,"discard_changes":true,"entry":6}),
        json!({"path":source,"discard_changes":true,"entry":99,"allow_losses":icns::KNOWN_IMPORT_LOSSES}),
    ] {
        assert_eq!(c.call("import_image", request)["isError"], true);
        assert_eq!(c.ok("inspect_document", json!({})), before);
    }
    let result=c.ok("import_image",json!({"path":source,"discard_changes":true,"entry":6,"allow_losses":icns::KNOWN_IMPORT_LOSSES}));
    assert_eq!(
        (result["width"].as_u64(), result["height"].as_u64()),
        (Some(32), Some(32))
    );
    assert_eq!(result["conversion"]["icns_entry"]["tag"], "ic11");
    assert_eq!(result["conversion"]["icns_entry"]["logical_width"], 16);
    assert_eq!(result["conversion"]["icns_entry"]["scale"], 2);
    assert_eq!(result["conversion"]["preserving_save"], false);
    let current = c.ok("inspect_document", json!({}));
    let renamed = root.join("renamed.icns");
    fs::rename(&source, &renamed).unwrap();
    let alias = root.join("alias.pcnd");
    fs::hard_link(&renamed, &alias).unwrap();
    assert_eq!(
        c.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(
        c.call(
            "export_icns",
            json!({"path":renamed,"overwrite":true,"types":["ic11"],"allow_losses":["hidden-rgb"]})
        )["isError"],
        true
    );
    assert_eq!(c.ok("inspect_document", json!({})), current);
    assert_eq!(fs::read(&renamed).unwrap(), SOURCE);
    let project = root.join("saved.pcnd");
    c.ok("save_project", json!({"path":project}));
    let doc = pcnd::read(&fs::read(&project).unwrap()).unwrap();
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(1, 0).0,
        [234, 91, 134, 64]
    );
    assert_eq!(
        c.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn icns_export_tool_schema_typed_preview_pixels_and_failures_preserve_state() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-mcp-icns-export-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let mut c = Client::new();
    let tools = c.request("tools/list", json!({}));
    let export = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "export_icns")
        .unwrap();
    fn enums(value: &Value, values: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                if let Some(Value::Array(items)) = object.get("enum") {
                    values.extend(items.iter().filter_map(Value::as_str).map(str::to_owned));
                }
                for child in object.values() {
                    enums(child, values);
                }
            }
            Value::Array(items) => {
                for child in items {
                    enums(child, values)
                }
            }
            _ => {}
        }
    }
    let mut values = Vec::new();
    enums(&export["inputSchema"], &mut values);
    for tag in [
        "ic07", "ic08", "ic09", "ic10", "ic11", "ic12", "ic13", "ic14",
    ] {
        assert!(values.iter().any(|value| value == tag));
    }
    assert!(
        !values
            .iter()
            .any(|value| value == "icp4" || value == "is32")
    );
    c.ok(
        "create_canvas",
        json!({"width":2,"height":1,"background":"#FF0000FF"}),
    );
    let project = root.join("source.pcnd");
    c.ok("save_project", json!({"path":project}));
    let original = fs::read(&project).unwrap();
    let before = c.ok("inspect_document", json!({}));
    let output = root.join("set.icns");
    fs::write(&output, b"original destination").unwrap();
    for request in [
        json!({"path":output,"overwrite":true,"types":["ic11"]}),
        json!({"path":output,"overwrite":true,"types":["icp4"],"allow_losses":["icon-resized"]}),
        json!({"path":output,"overwrite":true,"types":["ic11","ic11"],"allow_losses":["icon-resized"]}),
        json!({"path":output,"overwrite":true,"types":[],"allow_losses":["icon-resized"]}),
    ] {
        assert_eq!(c.call("export_icns", request)["isError"], true);
        assert_eq!(c.ok("inspect_document", json!({})), before);
        assert_eq!(fs::read(&output).unwrap(), b"original destination");
    }
    c.ok("export_icns",json!({"path":output,"overwrite":true,"types":["ic11","ic12"],"resampling":"nearest","allow_losses":["icon-resized"]}));
    let bytes = fs::read(&output).unwrap();
    for (index, side, logical) in [(0, 32, 16), (1, 64, 32)] {
        let decoded = icns::decode_entry(&bytes, index, icns::PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(
            (decoded.entry.logical_width, decoded.entry.scale),
            (logical, 2)
        );
        for y in 0..side {
            for x in 0..side {
                assert_eq!(
                    &decoded.rgba[((y * side + x) * 4) as usize..][..4],
                    if y >= side / 4 && y < side * 3 / 4 {
                        &[255, 0, 0, 255]
                    } else {
                        &[0; 4]
                    }
                );
            }
        }
    }
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&project).unwrap(), original);
    fs::remove_file(&output).unwrap();
    fs::hard_link(&project, &output).unwrap();
    assert_eq!(
        c.call(
            "export_icns",
            json!({"path":output,"overwrite":true,"types":["ic11"],"allow_losses":["icon-resized"]})
        )["isError"],
        true
    );
    assert_eq!(fs::read(&project).unwrap(), original);
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
