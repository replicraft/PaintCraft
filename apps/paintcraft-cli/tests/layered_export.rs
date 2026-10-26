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
    for format in ["psd", "psb"] {
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
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
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
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
    fs::remove_dir_all(folder).unwrap();
}
