//! Schema/early-failure controls independent of float registry qualification.
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
fn settings() -> Value {
    json!({"curve":"clip","exposure_milli_ev":0,"negative":"reject","alpha_output":"preserve"})
}
#[test]
fn cli_requires_full_policy_and_exact_digest_before_publication() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-float-validation-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let output = root.join("existing.pcnd");
    let original = b"preserve existing destination";
    fs::write(&output, original).unwrap();
    let base = [
        "import-float",
        "not-opened.exr",
        "--float-curve",
        "clip",
        "--float-exposure-milli-ev",
        "0",
        "--float-negative",
        "reject",
        "--float-alpha",
        "preserve",
        "--expect-input-sha256",
    ];
    let mut args = base.to_vec();
    args.extend([
        "bad",
        "--expect-assessment-sha256",
        "bad",
        "--output",
        output.to_str().unwrap(),
        "--overwrite",
        "--json",
    ]);
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap();
    assert!(!result.status.success());
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["engine"]["code"], "input.schema");
    assert_eq!(fs::read(&output).unwrap(), original);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(["inspect-float", "not-opened.exr"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn mcp_validation_preserves_saved_bindings_nonempty_history_and_dirty_state() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-mcp-float-validation-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let project = root.join("original.pcnd");
    let mut c = Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    c.ok("create_canvas", json!({"width":3,"height":2}));
    c.ok("save_project", json!({"path":project}));
    let original = fs::read(&project).unwrap();
    let before = c.ok("inspect_document", json!({}));
    let request = json!({"path":"not-opened.exr","settings":settings(),"expect_input_sha256":"bad","expect_assessment_sha256":"bad"});
    assert_eq!(c.call("import_float", request.clone())["isError"], true);
    assert_eq!(c.ok("inspect_document", json!({})), before);
    c.ok("save_project", json!({"path":project,"overwrite":true}));
    assert_eq!(fs::read(&project).unwrap(), original);
    fs::write(&project, b"external modification").unwrap();
    assert_eq!(
        c.call("save_project", json!({"path":project,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(fs::read(&project).unwrap(), b"external modification");
    fs::write(&project, &original).unwrap();
    c.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":1,"y":0}],"color":"#FF0000FF","size":1,"brush":"round"}));
    let dirty = c.ok("inspect_document", json!({}));
    assert_eq!(dirty["dirty"], true);
    assert_eq!(c.call("import_float", request)["isError"], true);
    assert_eq!(c.ok("inspect_document", json!({})), dirty);
    assert_eq!(fs::read(&project).unwrap(), original);
    let missing = json!({"path":"not-opened.exr","settings":settings(),"expect_input_sha256":"00".repeat(32)});
    assert_eq!(c.call("import_float", missing)["isError"], true);
    assert_eq!(c.ok("inspect_document", json!({})), dirty);
    let tools = c.request("tools/list", json!({}));
    let schema = &tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "import_float")
        .unwrap()["inputSchema"];
    assert_eq!(schema["additionalProperties"], false);
    let required = schema["required"].as_array().unwrap();
    for name in [
        "path",
        "settings",
        "expect_input_sha256",
        "expect_assessment_sha256",
    ] {
        assert!(required.contains(&json!(name)));
    }
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
