use super::*;
use paintcraft_formats::{icons, pcnd};
use std::fs;

#[test]
fn icon_entry_import_is_atomic_and_source_is_guarded() {
    let folder = std::env::temp_dir().join(format!("paintcraft-mcp-icons-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/icon-fixtures");
    let mut c = Client::new();
    c.ok("create_canvas", json!({"width":4,"height":3}));
    for suffix in ["ico", "cur"] {
        let bytes = fs::read(fixtures.join(format!("mixed.{suffix}"))).unwrap();
        let source = folder.join(format!("mixed.{suffix}"));
        fs::write(&source, &bytes).unwrap();
        let before = c.ok("inspect_document", json!({}));
        for request in [
            json!({"path":source,"discard_changes":true,"allow_losses":icons::KNOWN_IMPORT_LOSSES}),
            json!({"path":source,"discard_changes":true,"entry":1}),
            json!({"path":source,"discard_changes":true,"entry":8,"allow_losses":icons::KNOWN_IMPORT_LOSSES}),
            json!({"path":source,"discard_changes":true,"entry":1,"page":0,"allow_losses":icons::KNOWN_IMPORT_LOSSES}),
        ] {
            assert_eq!(c.call("import_image", request)["isError"], true);
            assert_eq!(c.ok("inspect_document", json!({})), before);
            assert_eq!(fs::read(&source).unwrap(), bytes);
        }
        let loaded = c.ok("import_image", json!({"path":source,"discard_changes":true,"entry":1,"allow_losses":icons::KNOWN_IMPORT_LOSSES}));
        assert_eq!(loaded["width"], 19);
        assert_eq!(loaded["height"], 19);
        assert_eq!(loaded["dirty"], true);
        let alias = folder.join(format!("{suffix}-alias.pcnd"));
        fs::hard_link(&source, &alias).unwrap();
        assert_eq!(
            c.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(c.ok("inspect_document", json!({})), loaded);
        let output = folder.join(format!("{suffix}.pcnd"));
        c.ok("save_project", json!({"path":output}));
        let doc = pcnd::read(&fs::read(output).unwrap()).unwrap();
        let expected = icons::decode_entry(&bytes, 1, 65536).unwrap();
        let raster = &doc.layers[&doc.roots[0]].raster;
        for (index, rgba) in expected.rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                raster.get((index % 19) as i64, (index / 19) as i64).0,
                *rgba
            );
        }
        assert_eq!(fs::read(source).unwrap(), bytes);
    }
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}
