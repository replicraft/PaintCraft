use super::*;
use paintcraft_formats::{pcnd, project_import};
use std::{fs, path::Path};

fn dcx() -> Vec<u8> {
    let mut bytes = vec![0; 4100];
    bytes[..4].copy_from_slice(&987_654_321u32.to_le_bytes());
    for (index, rgb) in [[10, 20, 30], [200, 210, 220]].into_iter().enumerate() {
        let offset = bytes.len() as u32;
        bytes[4 + index * 4..8 + index * 4].copy_from_slice(&offset.to_le_bytes());
        let mut pcx = vec![0; 128];
        pcx[..4].copy_from_slice(&[10, 5, 1, 8]);
        pcx[65] = 3;
        pcx[66..68].copy_from_slice(&2u16.to_le_bytes());
        for value in rgb {
            if value >= 192 {
                pcx.extend([0xc1, value]);
            } else {
                pcx.push(value);
            }
            pcx.push(0);
        }
        bytes.extend(pcx);
    }
    bytes
}
fn protect_source(c: &mut Client, source: &Path, bytes: &[u8], losses: &[&str]) {
    let state = c.ok("inspect_document", json!({}));
    for (ext, tool) in [("pcnd", "save_project"), ("png", "export_image")] {
        let alias = source.with_extension(format!("alias.{ext}"));
        fs::hard_link(source, &alias).unwrap();
        let mut args = json!({"path":alias,"overwrite":true});
        if tool == "export_image" {
            args["allow_losses"] = json!(losses);
        }
        let rejected = c.call(tool, args);
        assert_eq!(rejected["isError"], true, "{rejected}");
        let error: Value =
            serde_json::from_str(rejected["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(error["code"], "input.schema");
        assert!(
            error["message"]
                .as_str()
                .unwrap()
                .contains("must not replace"),
            "{error}"
        );
        assert_eq!(error["committed"], false);
        assert_eq!(fs::read(&alias).unwrap(), bytes);
        assert_eq!(fs::read(source).unwrap(), bytes);
        assert_eq!(c.ok("inspect_document", json!({})), state);
        fs::remove_file(alias).unwrap();
    }
    #[cfg(unix)]
    {
        let alias = source.with_extension("symlink.pcnd");
        std::os::unix::fs::symlink(source, &alias).unwrap();
        assert_eq!(
            c.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(fs::read(source).unwrap(), bytes);
        assert_eq!(c.ok("inspect_document", json!({})), state);
        fs::remove_file(alias).unwrap();
    }
}
#[test]
fn foreign_layers_require_exact_consent_and_cannot_overwrite_source_aliases() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-mcp-foreign-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/adapters");
    let mut c = Client::new();
    c.ok("create_canvas", json!({"width":4,"height":3}));
    for (name, fixture, losses, layers) in [
        (
            "source.paint",
            root.join("paint-lib/tests/fixtures/mspaintlib-sample.paint"),
            project_import::PAINT_LOSSES.to_vec(),
            5,
        ),
        (
            "source.pdn",
            root.join("pdn-lib/tests/fixtures/paintdotjs-2-layer.pdn"),
            vec![project_import::PDN_LOSS],
            2,
        ),
        (
            "source.ora",
            root.join("../services/paintcraft-formats/tests/ora-fixtures/deflated.ora"),
            vec!["ora-resolution-ignored"],
            3,
        ),
        (
            "source.psd",
            root.join("../services/paintcraft-formats/tests/psd-fixtures/layers-3.psd"),
            vec!["psd-metadata-ignored"],
            2,
        ),
        (
            "source.psb",
            root.join("../services/paintcraft-formats/tests/psd-fixtures/layers-1.psb"),
            vec!["psd-metadata-ignored"],
            2,
        ),
    ] {
        let source = folder.join(name);
        let bytes = fs::read(fixture).unwrap();
        fs::write(&source, &bytes).unwrap();
        let before = c.ok("inspect_document", json!({}));
        for request in [
            json!({"path":source}),
            json!({"path":source,"discard_changes":true}),
            json!({"path":source,"discard_changes":true,"allow_losses":["made-up"]}),
        ] {
            assert_eq!(c.call("import_image", request)["isError"], true);
            assert_eq!(c.ok("inspect_document", json!({})), before);
        }
        let imported = c.ok(
            "import_image",
            json!({"path":source,"discard_changes":true,"allow_losses":losses}),
        );
        assert_eq!(imported["dirty"], true);
        assert_eq!(imported["layers"].as_array().unwrap().len(), layers);
        if name.ends_with("pdn") {
            assert_eq!(imported["layers"][0]["name"], "Base 日本語");
            assert_eq!(imported["layers"][0]["opacity"], 201);
            assert_eq!(imported["layers"][1]["name"], "Overlay é");
            assert_eq!(imported["layers"][1]["visible"], false);
        }
        protect_source(&mut c, &source, &bytes, &["flatten"]);
        let output = folder.join(format!("{name}.pcnd"));
        c.ok("save_project", json!({"path":output}));
        let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(doc.roots.len(), layers);
        c.ok("open_project", json!({"path":output}));
        assert_eq!(
            c.ok("inspect_document", json!({}))["state_id"],
            imported["state_id"]
        );
        assert_eq!(fs::read(source).unwrap(), bytes);
        let broken = folder.join(name.replace("source", "broken"));
        fs::write(&broken, &bytes[..bytes.len() / 2]).unwrap();
        let saved = c.ok("inspect_document", json!({}));
        assert_eq!(
            c.call("import_image", json!({"path":broken,"allow_losses":losses}))["isError"],
            true
        );
        assert_eq!(c.ok("inspect_document", json!({})), saved);
    }
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn dcx_requires_explicit_page_exact_loss_consent_and_preserves_original_on_failure() {
    let folder = std::env::temp_dir().join(format!("paintcraft-mcp-dcx-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let source = folder.join("pages.dcx");
    let bytes = dcx();
    fs::write(&source, &bytes).unwrap();
    let mut c = Client::new();
    let before = c.ok("create_canvas", json!({"width":4,"height":3}));
    let losses = ["pcx-metadata-ignored", "dcx-other-pages-not-imported"];
    for request in [
        json!({"path":source,"discard_changes":true,"allow_losses":losses}),
        json!({"path":source,"discard_changes":true,"page":1}),
        json!({"path":source,"discard_changes":true,"page":1,"allow_losses":["pcx-metadata-ignored"]}),
        json!({"path":source,"discard_changes":true,"page":9,"allow_losses":losses}),
    ] {
        assert_eq!(c.call("import_image", request)["isError"], true);
        assert_eq!(c.ok("inspect_document", json!({})), before);
    }
    let loaded = c.ok(
        "import_image",
        json!({"path":source,"discard_changes":true,"page":1,"allow_losses":losses}),
    );
    assert_eq!(loaded["width"], 1);
    assert_eq!(loaded["height"], 1);
    protect_source(&mut c, &source, &bytes, &[]);
    let output = folder.join("selected.pcnd");
    c.ok("save_project", json!({"path":output}));
    let doc = pcnd::read(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(0, 0).0,
        [200, 210, 220, 255]
    );
    assert_eq!(fs::read(source).unwrap(), bytes);
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}
