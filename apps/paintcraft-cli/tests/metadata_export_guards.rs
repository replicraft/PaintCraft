//! Real file/session export refusal with independently authored PCND2 attachments.
use paintcraft_formats::pcnd;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/specialty-metadata/runs")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures")
}
fn metadata_error(output: &std::process::Output) {
    assert!(!output.status.success());
    let text = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("explicit source metadata retention or privacy omission policy"),
        "{text}"
    );
    assert!(
        !text.to_lowercase().contains("worker missing"),
        "must refuse before needing native helper"
    );
}
fn commands() -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    vec![
        ("render", "png", vec!["--format", "png"]),
        (
            "render",
            "dds",
            vec![
                "--format",
                "dds",
                "--dds-encoding",
                "rgba8",
                "--dds-mips",
                "none",
            ],
        ),
        (
            "render",
            "msp",
            vec!["--format", "msp", "--msp-version", "v2"],
        ),
        ("export-jxr", "jxr", vec![]),
        ("export-dcx", "dcx", vec!["--pcx-mode", "rgb24"]),
        ("export-icons", "ico", vec!["--entry", "16:png"]),
        ("export-icons", "cur", vec!["--entry", "16:png:0:0"]),
        ("export-icns", "icns", vec!["--type", "ic11"]),
    ]
}
#[test]
fn actual_cli_exports_refuse_without_changing_existing_destinations_or_native_source() {
    let folder = Folder::new();
    for fixture in [
        "rgb-profile.pcnd",
        "resolution-only.pcnd",
        "orientation-only.pcnd",
    ] {
        let input = folder.0.join(fixture);
        let original = fs::read(fixtures().join(fixture)).unwrap();
        fs::write(&input, &original).unwrap();
        let document = pcnd::read(&original).unwrap();
        assert!(document.source_metadata.is_some());
        for (command, extension, args) in commands() {
            let output = folder.0.join(format!("{fixture}-{command}.{extension}"));
            fs::write(&output, b"existing user file").unwrap();
            let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
                .arg(command)
                .arg(&input)
                .arg("-o")
                .arg(&output)
                .args(["--overwrite", "--frontend", "studio", "--json"])
                .args(args)
                .output()
                .unwrap();
            metadata_error(&result);
            assert_eq!(fs::read(&output).unwrap(), b"existing user file");
            assert_eq!(fs::read(&input).unwrap(), original);
            assert_eq!(
                pcnd::read(&fs::read(&input).unwrap())
                    .unwrap()
                    .source_metadata,
                document.source_metadata
            );
        }
        assert!(fs::read_dir(&folder.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("paintcraft-stage")
        }));
    }
}
#[test]
fn actual_stdio_specialty_refusals_keep_dirty_history_and_allow_metadata_preserving_pcnd_save() {
    let folder = Folder::new();
    for fixture in [
        "rgb-profile.pcnd",
        "resolution-only.pcnd",
        "orientation-only.pcnd",
    ] {
        let input = folder.0.join(fixture);
        let bytes = fs::read(fixtures().join(fixture)).unwrap();
        fs::write(&input, &bytes).unwrap();
        let expected = pcnd::read(&bytes).unwrap().source_metadata.unwrap();
        let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        client.ok("open_project", json!({"path":input}));
        client.ok("apply_operations",json!({"operations":[{"op":"layer.add","ref":"paint","name":"Paint","width":2,"height":3,"origin":{"x":0,"y":0},"default_pixel":"#FFFFFFFF"}]}));
        client.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":1,"y":2}],"color":"#123456FF","size":1,"brush":"round"}));
        let before = client.ok("inspect_document", json!({}));
        assert_eq!(before["dirty"], true);
        for (tool, extension, settings) in [
            ("export_image", "png", json!({})),
            (
                "export_image",
                "dds",
                json!({"dds":{"encoding":"rgba8","mips":"none"}}),
            ),
            ("export_image", "msp", json!({"msp_version":"v2"})),
            ("export_jxr", "jxr", json!({})),
            ("export_dcx", "dcx", json!({"pcx_mode":"rgb24"})),
            (
                "export_icons",
                "ico",
                json!({"entries":[{"width":16,"height":16,"encoding":"png"}]}),
            ),
            (
                "export_icons",
                "cur",
                json!({"entries":[{"width":16,"height":16,"encoding":"png","hotspot":[0,0]}]}),
            ),
            ("export_icns", "icns", json!({"types":["ic11"]})),
        ] {
            let output = folder.0.join(format!("rpc-{fixture}.{extension}"));
            fs::write(&output, b"existing user destination").unwrap();
            let mut args = settings;
            args["path"] = json!(output);
            args["overwrite"] = json!(true);
            let result = client.call(tool, args);
            assert_eq!(result["isError"], true, "{tool}: {result}");
            let text = result["content"][0]["text"].as_str().unwrap();
            assert!(
                text.contains("explicit source metadata retention or privacy omission policy"),
                "{tool}: {text}"
            );
            assert_eq!(fs::read(output).unwrap(), b"existing user destination");
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
        client.ok("history", json!({}));
        assert_ne!(
            client.ok("inspect_document", json!({}))["state_id"],
            before["state_id"]
        );
        client.ok("history", json!({"redo":true}));
        assert_eq!(client.ok("inspect_document", json!({})), before);
        let saved = folder.0.join(format!("preserved-{fixture}"));
        client.ok("save_project", json!({"path":saved}));
        let copy = pcnd::read(&fs::read(&saved).unwrap()).unwrap();
        assert_eq!(copy.source_metadata.as_ref(), Some(&expected));
        assert_eq!(copy.state_id.to_string(), before["state_id"]);
        assert_eq!(fs::read(&input).unwrap(), bytes);
    }
}
