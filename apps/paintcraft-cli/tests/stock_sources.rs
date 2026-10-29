//! Actual CLI/stdio MCP source-preservation workflows, not codec qualification.
use paintcraft_formats::{
    image::{self, ImageFormat},
    pcnd,
};
use serde_json::{Value, json};
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
            .join("../../docs/validation/stock-source/stdio")
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
fn cli(source: &Path, output: &Path, losses: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
    command
        .arg("import")
        .arg(source)
        .arg("-o")
        .arg(output)
        .args(["--overwrite", "--frontend", "studio", "--json"]);
    if !losses.is_empty() {
        command.args(["--allow-loss", &losses.join(",")]);
    }
    command.output().unwrap()
}
fn dirty(c: &mut mcp_client::Client) -> Value {
    c.ok("create_canvas", json!({"width":3,"height":2}));
    c.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":2,"y":1}],"color":"#123456FF","size":1,"brush":"round"}));
    c.ok("inspect_document", json!({}))
}
#[test]
fn real_stock_cli_and_stdio_mcp_preserve_sources_and_retain_alias_guards_after_pcnd_save() {
    for (format, ext, losses) in [
        (ImageFormat::Png, "png", &[][..]),
        (ImageFormat::Bmp, "bmp", &[][..]),
        (ImageFormat::Jpeg, "jpg", &[][..]),
        (ImageFormat::Gif, "gif", &["palette-indices-expanded"][..]),
        (ImageFormat::Tiff, "tiff", &["tiff-metadata-ignored"][..]),
    ] {
        let folder = Folder::new();
        let source = folder.0.join(format!("source.{ext}"));
        let bytes = image::encode(format, 2, 1, &[13, 71, 201, 255, 7, 8, 9, 255], 90).unwrap();
        fs::write(&source, &bytes).unwrap();
        let alias = folder.0.join("direct.pcnd");
        fs::hard_link(&source, &alias).unwrap();
        assert!(!cli(&source, &alias, losses).status.success());
        assert_eq!(fs::read(&source).unwrap(), bytes);
        let output = folder.0.join("cli.pcnd");
        let result = cli(&source, &output, losses);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            pcnd::read(&fs::read(output).unwrap()).unwrap().canvas,
            (2, 1)
        );
        let mut c = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        let before = dirty(&mut c);
        assert_eq!(
            c.call("import_image", json!({"path":source,"allow_losses":losses}))["isError"],
            true
        );
        assert_eq!(c.ok("inspect_document", json!({})), before);
        assert_eq!(
            c.call(
                "import_image",
                json!({"path":source,"discard_changes":true,"allow_losses":["unknown-stock-loss"]})
            )["isError"],
            true
        );
        assert_eq!(c.ok("inspect_document", json!({})), before);
        if !losses.is_empty() {
            assert_eq!(
                c.call(
                    "import_image",
                    json!({"path":source,"discard_changes":true})
                )["isError"],
                true
            );
            assert_eq!(c.ok("inspect_document", json!({})), before);
        }
        c.ok("history", json!({}));
        assert_ne!(
            c.ok("inspect_document", json!({}))["state_id"],
            before["state_id"]
        );
        c.ok("history", json!({"redo":true}));
        assert_eq!(c.ok("inspect_document", json!({})), before);
        c.ok(
            "import_image",
            json!({"path":source,"discard_changes":true,"allow_losses":losses}),
        );
        let project = folder.0.join("saved.pcnd");
        c.ok("save_project", json!({"path":project}));
        let imported = c.ok("inspect_document", json!({}));
        let moved = folder.0.join("moved");
        fs::rename(&source, &moved).unwrap();
        fs::write(&source, b"new pathname occupant").unwrap();
        let save_alias = folder.0.join("save-alias.pcnd");
        fs::hard_link(&moved, &save_alias).unwrap();
        assert_eq!(
            c.call("save_project", json!({"path":save_alias,"overwrite":true}))["isError"],
            true
        );
        let export_alias = folder.0.join("export-alias.png");
        fs::hard_link(&moved, &export_alias).unwrap();
        assert_eq!(c.call("export_image",json!({"path":export_alias,"overwrite":true,"allow_losses":["flatten","hidden-rgb"]}))["isError"],true);
        assert_eq!(c.ok("inspect_document", json!({})), imported);
        assert_eq!(fs::read(&moved).unwrap(), bytes);
        let other = folder.0.join("safe-copy.pcnd");
        c.ok("save_project", json!({"path":other}));
        assert_eq!(
            pcnd::read(&fs::read(other).unwrap()).unwrap().canvas,
            (2, 1)
        );
    }
}
#[test]
#[cfg(unix)]
fn legacy_fifo_input_keeps_its_immutable_snapshot_policy() {
    let folder = Folder::new();
    let fifo = folder.0.join("stream.png");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let bytes = image::encode(
        ImageFormat::Png,
        2,
        1,
        &[13, 71, 201, 255, 7, 8, 9, 255],
        90,
    )
    .unwrap();
    let writer_path = fifo.clone();
    let writer = std::thread::spawn(move || fs::write(writer_path, bytes).unwrap());
    let out = folder.0.join("stream.pcnd");
    let result = cli(&fifo, &out, &[]);
    writer.join().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(pcnd::read(&fs::read(out).unwrap()).unwrap().canvas, (2, 1));
}
