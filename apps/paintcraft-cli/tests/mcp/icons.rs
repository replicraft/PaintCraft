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
        let icon_alias = folder.join(format!("{suffix}-alias.ico"));
        fs::hard_link(&source, &icon_alias).unwrap();
        assert_eq!(c.call("export_icons", json!({"path":icon_alias,"overwrite":true,"entries":[{"width":16,"height":16,"encoding":"png"}],"allow_losses":["icon-resized"]}))["isError"], true);
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

#[test]
fn explicit_icon_export_requires_consent_and_preserves_project_and_destinations() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-mcp-icon-export-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let mut c = Client::new();
    c.ok(
        "create_canvas",
        json!({"width":2,"height":1,"background":"#FF0000FF"}),
    );
    let source = folder.join("source.pcnd");
    c.ok("save_project", json!({"path":source}));
    let source_bytes = fs::read(&source).unwrap();
    let before = c.ok("inspect_document", json!({}));
    for suffix in ["ico", "cur"] {
        let path = folder.join(format!("output.{suffix}"));
        fs::write(&path, b"existing destination").unwrap();
        let entries = if suffix == "ico" {
            json!([{"width":16,"height":16,"encoding":"png"},{"width":32,"height":32,"encoding":"bitmap"}])
        } else {
            json!([{"width":16,"height":16,"encoding":"png","hotspot":[3,7]},{"width":32,"height":32,"encoding":"bitmap","hotspot":[9,11]}])
        };
        for request in [
            json!({"path":path,"entries":entries,"overwrite":true}),
            json!({"path":path,"entries":[],"overwrite":true,"allow_losses":["icon-resized"]}),
            json!({"path":path,"entries":[{"width":257,"height":1,"encoding":"png"}],"overwrite":true,"allow_losses":["icon-resized"]}),
            json!({"path":path,"entries":entries,"overwrite":true,"allow_losses":["lossy-jpeg"]}),
        ] {
            assert_eq!(c.call("export_icons", request)["isError"], true);
            assert_eq!(c.ok("inspect_document", json!({})), before);
            assert_eq!(fs::read(&path).unwrap(), b"existing destination");
        }
        c.ok("export_icons", json!({"path":path,"entries":entries,"overwrite":true,"resampling":"nearest","allow_losses":["icon-resized"]}));
        assert_eq!(c.ok("inspect_document", json!({})), before);
        let bytes = fs::read(&path).unwrap();
        for (index, size) in [(0, 16), (1, 32)] {
            let decoded = icons::decode_entry(&bytes, index, 65536).unwrap();
            assert_eq!((decoded.entry.width, decoded.entry.height), (size, size));
            let offset = (size / 4 * size * 4) as usize;
            assert_eq!(&decoded.rgba[offset..offset + 4], &[255, 0, 0, 255]);
        }
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        fs::remove_file(&path).unwrap();
        fs::hard_link(&source, &path).unwrap();
        assert_eq!(c.call("export_icons", json!({"path":path,"entries":entries,"overwrite":true,"allow_losses":["icon-resized"]}))["isError"], true);
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
    }
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}
