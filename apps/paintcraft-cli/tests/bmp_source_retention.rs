//! Actual CLI and stdio BMP retention, consent, privacy and source guards.
use paintcraft_formats::{export, image, pcnd};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
struct Folder(PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn pixels(doc: &paintcraft_document::Document) -> Vec<u8> {
    let raster = &doc.layers[&doc.roots[0]].raster;
    let mut out = Vec::new();
    for y in 0..doc.canvas.1 {
        for x in 0..doc.canvas.0 {
            out.extend(raster.get(i64::from(x), i64::from(y)).0);
        }
    }
    out
}
#[test]
fn actual_bmp_cli_and_stdio_retain_exact_source_facts_and_preserve_originals() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/bmp-retention-fixtures");
    let oracle: serde_json::Value =
        serde_json::from_slice(&fs::read(fixtures.join("oracle.json")).unwrap()).unwrap();
    let profile = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/profiles/sRGB2014.icc"),
    )
    .unwrap();
    let folder = Folder(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/bmp-source-retention/cli")
            .join(uuid::Uuid::new_v4().to_string()),
    );
    fs::create_dir_all(&folder.0).unwrap();
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("create_canvas", json!({"width":2,"height":1}));
    let rows = oracle["fixtures"].as_array().unwrap();
    assert_eq!(rows.len(), 13);
    for (index, row) in rows.iter().enumerate() {
        let name = row["file"].as_str().unwrap();
        let bytes = fs::read(fixtures.join(name)).unwrap();
        let expected = fs::read(fixtures.join(format!("{name}.rgba"))).unwrap();
        let source = folder.0.join(name);
        fs::write(&source, &bytes).unwrap();
        let denied_path = folder.0.join(format!("without-consent-{index}.pcnd"));
        let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .arg("import")
            .arg(&source)
            .arg("-o")
            .arg(&denied_path)
            .output()
            .unwrap();
        let warned = name != "rgb24-v40-no-metadata.bmp";
        assert_eq!(
            result.status.success(),
            !warned,
            "{name}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert_eq!(denied_path.exists(), !warned);
        let cli = folder.0.join(format!("cli-{index}.pcnd"));
        let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .arg("import")
            .arg(&source)
            .arg("-o")
            .arg(&cli)
            .args(["--allow-loss", &image::KNOWN_IMPORT_LOSSES.join(",")])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let before = client.ok("inspect_document", json!({}));
        if warned {
            assert_eq!(
                client.call(
                    "import_image",
                    json!({"path":source,"discard_changes":true})
                )["isError"],
                true
            );
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
        let imported = client.ok(
            "import_image",
            json!({"path":source,"allow_losses":image::KNOWN_IMPORT_LOSSES,"discard_changes":true}),
        );
        assert_eq!(imported["dirty"], true);
        let remote = folder.0.join(format!("mcp-{index}.pcnd"));
        client.ok("save_project", json!({"path":remote}));
        let cli_bytes = fs::read(&cli).unwrap();
        assert_eq!(
            cli_bytes,
            fs::read(&remote).unwrap(),
            "deterministic cross-transport PCND: {name}"
        );
        let document = pcnd::read(&cli_bytes).unwrap();
        assert_eq!(pixels(&document), expected, "{name}: includes hidden RGB");
        let density = [
            row["density"][0].as_i64().unwrap(),
            row["density"][1].as_i64().unwrap(),
        ];
        let has_profile = row["profile"].as_bool().unwrap();
        assert_eq!(
            document.source_metadata.is_some(),
            has_profile || density != [0, 0]
        );
        if let Some(metadata) = &document.source_metadata {
            use paintcraft_document::source_metadata::{ResolutionUnit, SourceFormat};
            assert_eq!(metadata.descriptor().format, SourceFormat::Bmp);
            assert_eq!(
                metadata.profile(),
                has_profile.then_some(profile.as_slice())
            );
            if density == [0, 0] {
                assert!(metadata.descriptor().resolution.is_none());
            } else {
                let resolution = metadata.descriptor().resolution.unwrap();
                assert_eq!(resolution.unit, ResolutionUnit::Metre);
                assert_eq!(resolution.horizontal.numerator, density[0]);
                assert_eq!(resolution.vertical.numerator, density[1]);
                assert_eq!(resolution.horizontal.denominator, 1);
                assert_eq!(resolution.vertical.denominator, 1);
            }
            let output = folder.0.join(format!("copy-{index}.png"));
            fs::write(&output, b"existing destination").unwrap();
            let state = client.ok("inspect_document", json!({}));
            assert_eq!(
                client.call(
                    "export_image",
                    json!({"path":output,"overwrite":true,"allow_losses":export::KNOWN_LOSSES})
                )["isError"],
                true
            );
            assert_eq!(fs::read(&output).unwrap(), b"existing destination");
            client.ok("export_image", json!({"path":output,"overwrite":true,"source_metadata":"omit","allow_losses":export::KNOWN_LOSSES}));
            let decoded = image::decode(&fs::read(&output).unwrap()).unwrap();
            assert_eq!(
                decoded.rgba,
                paintcraft_render::render_flat(&document).unwrap()
            );
            assert_eq!(client.ok("inspect_document", json!({})), state);
            assert_eq!(fs::read(&remote).unwrap(), cli_bytes);
        }
        // A native extension cannot bypass the original file's hard-link guard,
        // including after a successful PCND save changed the current binding.
        let alias = folder.0.join(format!("source-alias-{index}.pcnd"));
        fs::hard_link(&source, &alias).unwrap();
        assert_eq!(
            client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(fs::read(&alias).unwrap(), bytes);
        client.ok(
            "open_project",
            json!({"path":remote,"discard_changes":true}),
        );
        assert_eq!(client.ok("inspect_document", json!({}))["dirty"], false);
    }
}
