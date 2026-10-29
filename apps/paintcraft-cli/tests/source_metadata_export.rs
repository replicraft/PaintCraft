//! Real command and stdio policy delivery; loss names alone never authorize omission.
use paintcraft_formats::{export, image, pcnd};
use serde_json::json;
use std::{fs, path::Path, process::Command};
#[path = "support/mcp_client.rs"]
mod mcp_client;
struct Folder(std::path::PathBuf, bool);
impl Drop for Folder {
    fn drop(&mut self) {
        if !self.1 {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn fixture() -> &'static [u8] {
    include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    )
}
#[test]
fn actual_cli_and_mcp_require_intent_and_keep_other_losses_project_and_destinations() {
    let selected = std::env::var_os("PAINTCRAFT_METADATA_EXPORT_EVIDENCE");
    let retained = selected.is_some();
    let folder = Folder(
        selected.map(std::path::PathBuf::from).unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "paintcraft-export-metadata-{}",
                uuid::Uuid::new_v4()
            ))
        }),
        retained,
    );
    fs::create_dir_all(&folder.0).unwrap();
    let source = folder.0.join("source.pcnd");
    let mut document = paintcraft_document::import_image(
        2,
        1,
        &[17, 92, 231, 255, 210, 40, 80, 255],
        "source-metadata-export-authored-pixels",
    )
    .unwrap();
    document.source_metadata = pcnd::read(fixture()).unwrap().source_metadata;
    assert!(document.source_metadata.is_some());
    let source_bytes = pcnd::write_compact(&document).unwrap();
    fs::write(&source, &source_bytes).unwrap();
    let expected = [17, 92, 231, 255, 210, 40, 80, 255];
    let mut mcp = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    mcp.ok("open_project", json!({"path":source}));
    let state = mcp.ok("inspect_document", json!({}));
    for (name, policy, allowed, success) in [
        ("default", None, export::KNOWN_LOSSES.to_vec(), false),
        (
            "refuse",
            Some("refuse"),
            export::KNOWN_LOSSES.to_vec(),
            false,
        ),
        (
            "reason-not-intent",
            Some("source-metadata-omitted"),
            export::KNOWN_LOSSES.to_vec(),
            false,
        ),
        ("explicit", Some("omit"), vec![], true),
    ] {
        let cli = folder.0.join(format!("cli-{name}.png"));
        let remote = folder.0.join(format!("mcp-{name}.png"));
        for path in [&cli, &remote] {
            fs::write(path, b"existing destination").unwrap();
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
        command
            .arg("render")
            .arg(&source)
            .arg("-o")
            .arg(&cli)
            .args(["--json", "--overwrite"]);
        if let Some(policy) = policy {
            command.args(["--source-metadata", policy]);
        }
        if !allowed.is_empty() {
            command.arg("--allow-loss").arg(allowed.join(","));
        }
        let result = command.output().unwrap();
        assert_eq!(
            result.status.success(),
            success,
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let mut args = json!({"path":remote,"overwrite":true,"allow_losses":allowed});
        if let Some(policy) = policy {
            args["source_metadata"] = policy.into();
        }
        let result = mcp.call("export_image", args);
        assert_eq!(result["isError"] == true, !success, "{result}");
        if success {
            assert_eq!(fs::read(&cli).unwrap(), fs::read(&remote).unwrap());
            assert_eq!(
                image::decode(&fs::read(&cli).unwrap()).unwrap().rgba,
                expected
            );
            assert!(
                result
                    .to_string()
                    .contains(export::metadata_policy::OMITTED_REASON)
            );
        } else {
            for path in [&cli, &remote] {
                assert_eq!(fs::read(path).unwrap(), b"existing destination");
            }
        }
        assert_eq!(mcp.ok("inspect_document", json!({})), state);
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
    }
    let jpeg = folder.0.join("jpeg-other-losses.jpg");
    fs::write(&jpeg, b"existing").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("render")
        .arg(&source)
        .arg("-o")
        .arg(&jpeg)
        .args(["--overwrite", "--source-metadata", "omit"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read(&jpeg).unwrap(), b"existing");
    assert_eq!(
        mcp.call(
            "export_image",
            json!({"path":jpeg,"source_metadata":"omit","overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(fs::read(&jpeg).unwrap(), b"existing");
    assert_eq!(mcp.ok("inspect_document", json!({})), state);
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}
