//! Real command owners reject metadata growth before history or file publication.
use paintcraft_formats::{export, pcnd};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "../../../crates/services/paintcraft-formats/tests/support/metadata_limit.rs"]
mod metadata_limit;
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/persistent-metadata-cli")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn add_layer() -> serde_json::Value {
    json!({"op":"layer.add","ref":"blocked","name":"x","width":2,"height":2,
        "origin":{"x":0,"y":0},"default_pixel":"#00000000"})
}
#[test]
fn actual_cli_apply_rejects_growth_before_touching_the_destination() {
    let folder = Folder::new();
    let doc = metadata_limit::exact_doc(2, false);
    let input = folder.0.join("input.pcnd");
    let original = pcnd::write_compact(&doc).unwrap();
    fs::write(&input, &original).unwrap();
    let operations = folder.0.join("operations.json");
    fs::write(
        &operations,
        serde_json::to_vec(&json!({"schema":"paintcraft.batch/v2","atomic":true,
        "policy":paintcraft_document::POLICY,"base":{"state_id":doc.state_id.to_string(),
        "publication_epoch":"0"},"operations":[add_layer()]}))
        .unwrap(),
    )
    .unwrap();
    let output = folder.0.join("output.pcnd");
    fs::write(&output, b"existing destination").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("apply")
        .arg(&input)
        .arg("--operations")
        .arg(operations)
        .arg("-o")
        .arg(&output)
        .args(["--overwrite", "--json"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    let report = String::from_utf8(result.stdout).unwrap();
    assert!(
        report.contains("resource.limit"),
        "{report} {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(&output).unwrap(), b"existing destination");
    assert_eq!(fs::read(&input).unwrap(), original);
}
#[test]
fn actual_stdio_preserves_state_on_growth_and_allows_fitting_edits_history_and_copies() {
    let folder = Folder::new();
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    for with_layer in [false, true] {
        let doc = metadata_limit::exact_doc(2, with_layer);
        let input = folder.0.join(format!("input-{with_layer}.pcnd"));
        let original = pcnd::write_compact(&doc).unwrap();
        fs::write(&input, &original).unwrap();
        client.ok("open_project", json!({"path":input,"discard_changes":true}));
        let before = client.ok("inspect_document", json!({}));
        let rejected = client.call("apply_operations", json!({"operations":[add_layer()]}));
        assert_eq!(rejected["isError"], true, "{rejected}");
        assert!(
            rejected.to_string().contains("resource.limit"),
            "{rejected}"
        );
        assert_eq!(client.ok("inspect_document", json!({})), before);
        if with_layer {
            let rejected = client.call(
                "apply_operations",
                json!({"operations":[{
                    "op":"filter.add","layer_ref":{"id":doc.roots[0].to_string()},
                    "ref":"blocked-filter","descriptor":{"kind":"invert-rgb-v1"}
                }]}),
            );
            assert_eq!(rejected["isError"], true, "{rejected}");
            assert!(
                rejected.to_string().contains("resource.limit"),
                "{rejected}"
            );
            assert_eq!(
                client.ok("inspect_document", json!({})),
                before,
                "no filter aliases, history, metadata or publication may leak"
            );
            let rejected = client.call("paint_stroke", json!({"points":[{"x":0,"y":0},{"x":1,"y":1}],"color":"#000000FF","size":1,"brush":"round"}));
            assert_eq!(rejected["isError"], true, "{rejected}");
            assert!(
                rejected.to_string().contains("resource.limit"),
                "{rejected}"
            );
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
        let saved = folder.0.join(format!("unchanged-{with_layer}.pcnd"));
        client.ok("save_project", json!({"path":saved}));
        assert_eq!(fs::read(&saved).unwrap(), original);
        if with_layer {
            client.ok(
                "apply_operations",
                json!({"operations":[{"op":"layer.set_properties",
                "layer_ref":{"id":doc.roots[0].to_string()},"name":"y"}]}),
            );
            let after = client.ok("inspect_document", json!({}));
            assert_eq!(after["undo_depth"], 1);
            assert_eq!(after["dirty"], true);
            let renamed = folder.0.join("renamed.pcnd");
            client.ok("save_project", json!({"path":renamed}));
            let reopened = pcnd::read(&fs::read(renamed).unwrap()).unwrap();
            assert_eq!(reopened.layers[&reopened.roots[0]].name, "y");
            assert_eq!(reopened.source_metadata, doc.source_metadata);
            client.ok("history", json!({"redo":false}));
            let restored = folder.0.join("restored.pcnd");
            client.ok("save_project", json!({"path":restored}));
            assert_eq!(fs::read(restored).unwrap(), original);
        }
        let state = client.ok("inspect_document", json!({}));
        let copy = folder.0.join(format!("private-{with_layer}.png"));
        fs::write(&copy, b"existing copy").unwrap();
        assert_eq!(
            client.call(
                "export_image",
                json!({"path":copy,"overwrite":true,
            "allow_losses":export::KNOWN_LOSSES})
            )["isError"],
            true
        );
        assert_eq!(fs::read(&copy).unwrap(), b"existing copy");
        client.ok(
            "export_image",
            json!({"path":copy,"overwrite":true,
            "source_metadata":"omit","allow_losses":export::KNOWN_LOSSES}),
        );
        assert_eq!(client.ok("inspect_document", json!({})), state);
        assert_eq!(fs::read(&input).unwrap(), original);
    }
}
