//! Actual CLI/stdio import, preservation, consent and source-alias guards.
use paintcraft_formats::{image, pcnd};
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
#[test]
fn actual_png_import_and_pcnd_save_retain_original_metadata_without_foreign_binding() {
    let folder = Folder(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/png-source-retention/cli")
            .join(uuid::Uuid::new_v4().to_string()),
    );
    fs::create_dir_all(&folder.0).unwrap();
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("create_canvas", json!({"width":2,"height":1}));
    for (i, name) in [
        "rgba8.png",
        "gray16-adam7.png",
        "indexed4-adam7.png",
        "rgba8-phys-only.png",
        "rgba8-omissions-only.png",
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/services/paintcraft-formats/tests/png-retention-fixtures")
            .join(name);
        let bytes = fs::read(&fixture).unwrap();
        let source = folder.0.join(name);
        fs::write(&source, &bytes).unwrap();
        let cli = folder.0.join(format!("cli{i}.pcnd"));
        let remote = folder.0.join(format!("mcp{i}.pcnd"));
        let denied = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .arg("import")
            .arg(&source)
            .arg("-o")
            .arg(&cli)
            .output()
            .unwrap();
        assert!(!denied.status.success());
        assert!(!cli.exists());
        let allowed = image::KNOWN_IMPORT_LOSSES.join(",");
        let imported = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .arg("import")
            .arg(&source)
            .arg("-o")
            .arg(&cli)
            .args(["--allow-loss", &allowed])
            .output()
            .unwrap();
        assert!(
            imported.status.success(),
            "{name}: {} {}",
            String::from_utf8_lossy(&imported.stdout),
            String::from_utf8_lossy(&imported.stderr)
        );
        let before = client.ok("inspect_document", json!({}));
        let denied = client.call(
            "import_image",
            json!({"path":source,"discard_changes":true}),
        );
        assert_eq!(denied["isError"], true);
        assert_eq!(client.ok("inspect_document", json!({})), before);
        let imported = client.ok(
            "import_image",
            json!({"path":source,"allow_losses":image::KNOWN_IMPORT_LOSSES,"discard_changes":true}),
        );
        assert_eq!(imported["dirty"], true, "{name}: {imported}");
        let alias = client.call("save_project", json!({"path":source,"overwrite":true}));
        assert_eq!(alias["isError"], true);
        client.ok("save_project", json!({"path":remote}));
        let a = pcnd::read(&fs::read(&cli).unwrap()).unwrap();
        let b = pcnd::read(&fs::read(&remote).unwrap()).unwrap();
        assert!(a.same_semantics(&b), "{name}");
        if name != "rgba8-omissions-only.png" {
            assert!(a.source_metadata.is_some());
        }
        assert_eq!(fs::read(&source).unwrap(), bytes);
        client.ok(
            "open_project",
            json!({"path":remote,"discard_changes":true}),
        );
        assert_eq!(client.ok("inspect_document", json!({}))["dirty"], false);
    }
}
