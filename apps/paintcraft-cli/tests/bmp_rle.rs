//! Actual CLI/stdio MCP BMP RLE consumers, pinned pixels and cross-frontend IDs.
use paintcraft_document::Document;
use paintcraft_formats::pcnd;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
            .join("../../docs/validation/bmp-rle-consumers/runs")
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
        .join("../../crates/services/paintcraft-formats/tests/bmp-rle-fixtures")
}
fn names() -> Vec<String> {
    [
        "rle4-combined.bmp",
        "rle8-combined.bmp",
        "rle4-size-zero.bmp",
        "rle8-size-zero.bmp",
        "rle4-icc-before.bmp",
        "rle8-icc-after-size-zero.bmp",
    ]
    .map(str::to_owned)
    .to_vec()
}
struct Case {
    name: String,
    bytes: Vec<u8>,
    pixels: Vec<u8>,
    size: (u32, u32),
    expected: Document,
}
impl Case {
    fn new(name: &str) -> Self {
        let data: Value =
            serde_json::from_slice(&fs::read(fixtures().join("oracle.json")).unwrap()).unwrap();
        let row = data["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["file"] == name)
            .unwrap();
        let bytes = fs::read(fixtures().join(name)).unwrap();
        let pixels = fs::read(fixtures().join(format!("{name}.rgba"))).unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row["sha256"].as_str().unwrap()
        );
        assert_eq!(
            pixels,
            row["ffmpeg_rgba"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect::<Vec<_>>()
        );
        let size = (
            row["width"].as_u64().unwrap() as u32,
            row["height"].as_u64().unwrap() as u32,
        );
        let expected = paintcraft_document::import_image(
            size.0,
            size.1,
            &pixels,
            &paintcraft_document::hex(&Sha256::digest(&bytes)),
        )
        .unwrap();
        Self {
            name: name.into(),
            bytes,
            pixels,
            size,
            expected,
        }
    }
    fn losses(&self) -> Vec<&'static str> {
        let mut losses = vec!["palette-indices-expanded"];
        if self.name.contains("-icc-") {
            losses.extend(["icc-profile-ignored", "bmp-color-metadata-ignored"]);
        }
        if self.name.contains("size-zero") {
            losses.push("bmp-metadata-ignored");
        }
        losses
    }
    fn assert_doc(&self, doc: &Document) {
        assert_eq!(doc.canvas, self.size, "{} imported exactly once", self.name);
        assert_ids(doc, &self.expected);
        let layer = &doc.layers[&doc.roots[0]];
        let mut rgba = Vec::new();
        for y in 0..self.size.1 {
            for x in 0..self.size.0 {
                rgba.extend(layer.raster.get(i64::from(x), i64::from(y)).0);
            }
        }
        assert_eq!(rgba, self.pixels, "{} independent raw oracle", self.name);
    }
}
fn assert_ids(a: &Document, b: &Document) {
    assert_eq!(a.document_id, b.document_id);
    assert_eq!(a.state_id, b.state_id);
    assert_eq!(a.roots, b.roots);
}
fn read(path: &Path) -> Document {
    pcnd::read(&fs::read(path).unwrap()).unwrap()
}
fn cli(source: &Path, output: &Path, losses: &[&str], profile: &str) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
    cmd.arg("import").arg(source).arg("-o").arg(output).args([
        "--overwrite",
        "--frontend",
        profile,
        "--json",
    ]);
    if !losses.is_empty() {
        cmd.args(["--allow-loss", &losses.join(",")]);
    }
    cmd.output().unwrap()
}
#[test]
fn actual_cli_and_stdio_mcp_bmp_rle_copies_match_oracles_and_other_frontend_document_ids() {
    let folder = Folder::new();
    for name in names() {
        let case = Case::new(&name);
        let losses = case.losses();
        let source = folder.0.join(&name);
        fs::write(&source, &case.bytes).unwrap();
        let refused = folder.0.join("no-consent.pcnd");
        let result = cli(&source, &refused, &[], "studio");
        assert!(
            !result.status.success(),
            "{name} must require actual losses"
        );
        assert!(!refused.exists());
        if losses.len() > 1 {
            for omitted in &losses {
                let partial: Vec<_> = losses
                    .iter()
                    .copied()
                    .filter(|loss| loss != omitted)
                    .collect();
                let result = cli(&source, &refused, &partial, "studio");
                assert!(
                    !result.status.success(),
                    "{name} must acknowledge {omitted}"
                );
                assert!(!refused.exists());
            }
        }
        let mut previous = None;
        for repeat in 0..2 {
            let output = folder.0.join(format!("cli-{name}-{repeat}.pcnd"));
            let result = cli(&source, &output, &losses, "studio");
            assert!(
                result.status.success(),
                "{} {}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            let a = read(&output);
            case.assert_doc(&a);
            let mut client =
                mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
            client.ok("import_image", json!({"path":source,"allow_losses":losses}));
            let saved = folder.0.join(format!("mcp-{name}-{repeat}.pcnd"));
            client.ok("save_project", json!({"path":saved}));
            let b = read(&saved);
            case.assert_doc(&b);
            assert_ids(&a, &b);
            if let Some(previous) = &previous {
                assert_ids(previous, &a);
            }
            previous = Some(a.clone());
            if repeat == 0
                && let Some(root) = std::env::var_os("PAINTCRAFT_BMP_RLE_CONSUMER_EVIDENCE")
            {
                let root = PathBuf::from(root);
                for transport in ["native", "web"] {
                    let peer = root.join(transport).join(format!("{name}.pcnd"));
                    let doc = read(&peer);
                    case.assert_doc(&doc);
                    assert_ids(&a, &doc);
                    println!(
                        "BMP RLE {name}: actual CLI/MCP/{transport} raw pixels and all IDs match"
                    );
                }
                for (transport, path) in [("cli", &output), ("mcp", &saved)] {
                    let target = root.join(transport);
                    fs::create_dir_all(&target).unwrap();
                    fs::copy(path, target.join(format!("{name}.pcnd"))).unwrap();
                }
            }
        }
        assert_eq!(fs::read(&source).unwrap(), case.bytes);
    }
    for profile in ["classic-v7", "classic-v10", "classic-v11"] {
        let case = Case::new("rle8-icc-after-size-zero.bmp");
        let source = folder.0.join("profile.bmp");
        fs::write(&source, &case.bytes).unwrap();
        let output = folder.0.join(format!("{profile}.pcnd"));
        let result = cli(&source, &output, &case.losses(), profile);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        case.assert_doc(&read(&output));
    }
}
#[test]
fn actual_bmp_rle_stdio_refusals_preserve_painting_history_and_original_source_aliases() {
    let folder = Folder::new();
    let case = Case::new("rle8-icc-after-size-zero.bmp");
    let losses = case.losses();
    let source = folder.0.join("source.bmp");
    fs::write(&source, &case.bytes).unwrap();
    let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("create_canvas", json!({"width":3,"height":2}));
    client.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":2,"y":1}],"color":"#123456FF","size":1,"brush":"round"}));
    let before = client.ok("inspect_document", json!({}));
    for args in [
        json!({"path":source,"allow_losses":losses}),
        json!({"path":source,"discard_changes":true}),
        json!({"path":source,"discard_changes":true,"allow_losses":["unknown-bmp_rle-loss"]}),
        json!({"path":source,"discard_changes":true,"allow_losses":["palette-indices-expanded","icc-profile-ignored","bmp-color-metadata-ignored"]}),
        json!({"path":source,"discard_changes":true,"allow_losses":["palette-indices-expanded","icc-profile-ignored","bmp-metadata-ignored"]}),
    ] {
        assert_eq!(client.call("import_image", args)["isError"], true);
        assert_eq!(client.ok("inspect_document", json!({})), before);
    }
    client.ok("history", json!({}));
    assert_ne!(
        client.ok("inspect_document", json!({}))["state_id"],
        before["state_id"]
    );
    client.ok("history", json!({"redo":true}));
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let alias = folder.0.join("alias.pcnd");
    fs::hard_link(&source, &alias).unwrap();
    let result = cli(&source, &alias, &losses, "studio");
    assert!(!result.status.success());
    assert_eq!(fs::read(&source).unwrap(), case.bytes);
    client.ok(
        "import_image",
        json!({"path":source,"discard_changes":true,"allow_losses":losses}),
    );
    let saved = folder.0.join("saved.pcnd");
    client.ok("save_project", json!({"path":saved}));
    case.assert_doc(&read(&saved));
    let adopted = client.ok("inspect_document", json!({}));
    let moved = folder.0.join("moved.bmp");
    fs::rename(&source, &moved).unwrap();
    fs::write(&source, b"new pathname occupant").unwrap();
    assert_eq!(
        client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    let export = folder.0.join("alias.png");
    fs::hard_link(&moved, &export).unwrap();
    assert_eq!(
        client.call(
            "export_image",
            json!({"path":export,"overwrite":true,"allow_losses":["flatten","hidden-rgb"]})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), adopted);
    assert_eq!(fs::read(&moved).unwrap(), case.bytes);
}
