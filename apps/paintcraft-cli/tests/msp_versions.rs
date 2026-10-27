//! Actual version selection, exact independent golden and protected copy outputs.
use paintcraft_core::types::Rgba;
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_formats::pcnd;
use paintcraft_raster::{Admission, RasterLayer};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
const GOLDEN: &[u8] = include_bytes!(
    "../../../crates/services/paintcraft-formats/tests/msp2-writer-fixtures/runs-2048.msp"
);
fn root(name: &str) -> PathBuf {
    let base = std::env::var_os("PAINTCRAFT_MSP_CONSUMER_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let folder = base.join(format!("{name}-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&folder).unwrap();
    folder
}
fn pixels(odd: bool) -> (u32, u32, Vec<u8>) {
    let (width, height, packed): (u32, u32, Vec<u8>) = if odd {
        (9, 3, vec![0x80, 0x80, 0xff, 0x80, 0x55, 0])
    } else {
        let mut packed = vec![0; 256];
        packed.extend([255; 256]);
        packed.extend(0u8..=255);
        (2048, 3, packed)
    };
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let value = if packed[(y * width.div_ceil(8) + x / 8) as usize] & (0x80 >> (x % 8)) != 0
            {
                255
            } else {
                0
            };
            rgba.extend([value, value, value, 255]);
        }
    }
    (width, height, rgba)
}
fn document(odd: bool) -> Document {
    let (w, h, rgba) = pixels(odd);
    let mut doc = new_document(w, h, Some(Rgba([0, 0, 0, 255])), 0, POLICY).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster =
        RasterLayer::from_rgba(w, h, Rgba([0; 4]), &rgba, &mut Admission::new(1 << 20)).unwrap();
    doc
}
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn success(args: &[&str]) -> Value {
    let result = run(args);
    assert!(
        result.status.success(),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn cleanup(root: PathBuf) {
    if std::env::var_os("PAINTCRAFT_MSP_CONSUMER_OUTPUT").is_none() {
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn cli_versions_preserve_default_bytes_and_match_the_independent_v2_file() {
    let folder = root("cli");
    for odd in [false, true] {
        let name = if odd { "odd" } else { "runs" };
        let source = folder.join(format!("{name}.pcnd"));
        let original = pcnd::write(&document(odd)).unwrap();
        fs::write(&source, &original).unwrap();
        let mut outputs = Vec::new();
        for version in [None, Some("v1"), Some("v2")] {
            let output = folder.join(format!("{name}-{}.msp", version.unwrap_or("default")));
            let mut args = vec![
                "render",
                source.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--json",
            ];
            if let Some(version) = version {
                args.extend(["--msp-version", version]);
            }
            let reply = success(&args);
            assert_eq!(reply["outcome"], "success");
            let bytes = fs::read(&output).unwrap();
            assert_eq!(
                &bytes[..4],
                if version == Some("v2") {
                    b"LinS"
                } else {
                    b"DanM"
                }
            );
            if version.is_some() {
                assert!(
                    reply
                        .to_string()
                        .contains("exact-opaque-black-white-msb-first")
                );
            }
            if !odd && version == Some("v2") {
                assert_eq!(bytes, GOLDEN);
            }
            outputs.push(bytes);
        }
        assert_eq!(outputs[0], outputs[1]);
        assert_ne!(outputs[1], outputs[2]);
        for extra in [
            vec!["--msp-version", "v3"],
            vec!["--msp-version", "v2", "--frontend", "classic-v11"],
            vec!["--msp-version", "v2", "--render-workers", "2"],
            vec![
                "--msp-version",
                "v1",
                "--expect-input-sha256",
                &"0".repeat(64),
            ],
        ] {
            let output = folder.join("denied.msp");
            let mut args = vec![
                "render",
                source.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--json",
            ];
            args.extend(extra);
            assert!(!run(&args).status.success());
            assert!(!output.exists());
        }
        let alias = folder.join(format!("{name}-source-alias.msp"));
        fs::hard_link(&source, &alias).unwrap();
        assert!(
            !run(&[
                "render",
                source.to_str().unwrap(),
                "--output",
                alias.to_str().unwrap(),
                "--overwrite",
                "--msp-version",
                "v2",
                "--json"
            ])
            .status
            .success()
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        let wrong = folder.join("wrong.png");
        assert!(
            !run(&[
                "render",
                source.to_str().unwrap(),
                "--output",
                wrong.to_str().unwrap(),
                "--msp-version",
                "v1",
                "--json"
            ])
            .status
            .success()
        );
        assert!(!wrong.exists());
    }
    cleanup(folder);
}
#[test]
fn mcp_versions_keep_session_binding_after_rename_and_do_not_convert_color_or_alpha() {
    let folder = root("mcp");
    let mut client = Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    for odd in [false, true] {
        let name = if odd { "odd" } else { "runs" };
        let source = folder.join(format!("{name}.pcnd"));
        let original = pcnd::write(&document(odd)).unwrap();
        fs::write(&source, &original).unwrap();
        // Replacing the prior dirty fixture is deliberate setup between cases.
        client.ok("open_project", json!({"path":source,"discard_changes":odd}));
        let opened = client.ok("inspect_document", json!({}));
        assert_eq!(opened["dirty"], false);
        let layer = opened["layers"][0]["id"].clone();
        client.ok(
            "apply_operations",
            json!({"operations":[{
                "op":"layer.set_properties","layer_ref":{"id":layer},
                "name":format!("Unsaved {name}")
            }]}),
        );
        let moved = folder.join(format!("{name}-moved.pcnd"));
        fs::rename(&source, &moved).unwrap();
        let before = client.ok("inspect_document", json!({}));
        assert_eq!(before["dirty"], true);
        assert_eq!(
            before["undo_depth"],
            opened["undo_depth"].as_u64().unwrap() + 1
        );
        assert_eq!(before["redo_depth"], 0);
        assert_ne!(before["state_id"], opened["state_id"]);
        assert_eq!(before["document_id"], opened["document_id"]);
        // Inspection exposes history/dirty state, not a saved-path field. Prove
        // the actual opened binding remains protected before and after export.
        let alias = folder.join(format!("{name}-source-alias.msp"));
        fs::hard_link(&moved, &alias).unwrap();
        assert_eq!(
            client.call(
                "export_image",
                json!({"path":alias,"overwrite":true,"msp_version":"v2"})
            )["isError"],
            true
        );
        assert_eq!(fs::read(&alias).unwrap(), original);
        assert_eq!(client.ok("inspect_document", json!({})), before);
        let mut outputs = Vec::new();
        for version in [None, Some("v1"), Some("v2")] {
            let path = folder.join(format!("{name}-{}.msp", version.unwrap_or("default")));
            let mut request = json!({"path":path});
            if let Some(version) = version {
                request["msp_version"] = json!(version);
            }
            let result = client.ok("export_image", request);
            if let Some(version) = version {
                assert_eq!(result["export"]["version"], version);
                assert_eq!(result["export"]["preserving_save"], false);
            }
            let bytes = fs::read(&path).unwrap();
            if !odd && version == Some("v2") {
                assert_eq!(bytes, GOLDEN);
            }
            outputs.push(bytes);
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
        assert_eq!(outputs[0], outputs[1]);
        assert_ne!(outputs[1], outputs[2]);
        assert_eq!(
            client.call(
                "export_image",
                json!({"path":alias,"overwrite":true,"msp_version":"v2"})
            )["isError"],
            true
        );
        assert_eq!(fs::read(&alias).unwrap(), original);
        let fresh = folder.join(format!("{name}-after-alias.msp"));
        client.ok("export_image", json!({"path":fresh,"msp_version":"v2"}));
        assert_eq!(fs::read(&fresh).unwrap(), outputs[2]);
        for request in [
            json!({"path":folder.join("wrong.png"),"msp_version":"v1"}),
            json!({"path":folder.join("bad.msp"),"msp_version":"v3"}),
            json!({"path":folder.join("bad.msp"),"msp_version":"v2","dds":{}}),
        ] {
            assert_eq!(client.call("export_image", request)["isError"], true);
        }
        assert_eq!(client.ok("inspect_document", json!({})), before);
        assert_eq!(fs::read(&moved).unwrap(), original);
    }
    for color in ["#808080FF", "#FFFFFF7F"] {
        client.ok(
            "create_canvas",
            json!({"width":1,"height":1,"background":color,"discard_changes":true}),
        );
        let before = client.ok("inspect_document", json!({}));
        for version in ["v1", "v2"] {
            let output = folder.join("unsupported.msp");
            assert_eq!(
                client.call("export_image", json!({"path":output,"msp_version":version}))["isError"],
                true
            );
            assert!(!output.exists());
        }
        assert_eq!(client.ok("inspect_document", json!({})), before);
    }
    cleanup(folder);
}
