//! Production entrypoints honor external-application qualification, not consent bypasses.
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
use serde_json::json;
use std::{fs, process::Command};
#[test]
fn cli_requires_writer_qualification_before_acquiring_input_or_touching_output() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-cli-layered-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&folder).unwrap();
    for format in ["psd", "psb", "pdn", "paint"] {
        let output = folder.join(format!("copy.{format}"));
        fs::write(&output, b"original").unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .args(["export-layered", "missing.pcnd", "--format", format, "-o"])
            .arg(&output)
            .args([
                "--overwrite",
                "--allow-loss",
                "psd-document-title-not-exported",
                "--json",
            ])
            .output()
            .unwrap();
        assert!(!result.status.success());
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("capability.unsupported"), "{text}");
        assert_eq!(fs::read(&output).unwrap(), b"original");
    }
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 4);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn mcp_denial_preserves_document_history_dirty_state_and_existing_destination() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-mcp-layered-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&folder).unwrap();
    let path = folder.join("copy.psd");
    fs::write(&path, b"original").unwrap();
    let mut client = Client::at(std::path::Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    let tools = client.request("tools/list", json!({}));
    assert!(
        tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "export_layered")
    );
    client.ok("create_canvas", json!({"width":4,"height":3}));
    let before = client.ok("inspect_document", json!({}));
    let denied=client.call("export_layered",json!({"path":path,"format":"psd","overwrite":true,"allow_losses":["psd-document-title-not-exported"]}));
    assert_eq!(denied["isError"], true);
    assert!(denied.to_string().contains("capability.unsupported"));
    let pdn_path = folder.join("copy.pdn");
    fs::write(&pdn_path, b"original pdn").unwrap();
    let denied=client.call("export_layered",json!({"path":pdn_path,"format":"pdn","pdn_background":"ordinary","pdn_geometry":"materialize","overwrite":true,"allow_losses":["pdn-document-title-not-exported","pdn-layer-geometry-materialized","pdn-outside-canvas-samples-not-retained"]}));
    assert_eq!(denied["isError"], true);
    assert!(denied.to_string().contains("capability.unsupported"));
    assert_eq!(fs::read(&pdn_path).unwrap(), b"original pdn");
    let paint_path = folder.join("copy.paint");
    fs::write(&paint_path, b"original paint").unwrap();
    let denied=client.call("export_layered",json!({"path":paint_path,"format":"paint","paint_canvas_fill":[0,0,0,0],"paint_names":"omit","overwrite":true,"allow_losses":["paint-layer-names-not-retained"]}));
    assert_eq!(denied["isError"], true);
    assert!(denied.to_string().contains("capability.unsupported"));
    assert_eq!(fs::read(&paint_path).unwrap(), b"original paint");
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 3);
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn explicit_paint_cli_flags_parse_but_do_not_override_qualification() {
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args([
            "export-layered",
            "missing.pcnd",
            "--format",
            "paint",
            "--paint-canvas-fill",
            "0,0,0,0",
            "--paint-names",
            "omit",
            "-o",
            "missing-copy.paint",
            "--allow-loss",
            "paint-layer-names-not-retained",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(
        text.contains("capability.unsupported"),
        "{text} {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!std::path::Path::new("missing-copy.paint").exists());
}
