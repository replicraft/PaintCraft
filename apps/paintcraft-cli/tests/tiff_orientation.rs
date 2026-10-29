//! Actual compiled CLI and stdio MCP orientation conversions, independent pixels.
use paintcraft_document::Document;
use paintcraft_formats::{image, pcnd};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
const POLICY: &str = "tiff-uint16-high-byte-oriented-rgba8-v2";
const PAGES: &str = "tiff-other-pages-not-imported";
const DEPTH: &str = "depth-16-reduced-to-8";
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/tiff-orientation-consumers/runs")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/tiff-orientation-fixtures")
}
fn hash(bytes: &[u8]) -> String {
    paintcraft_document::hex(&Sha256::digest(bytes))
}
struct Case {
    name: &'static str,
    page: u16,
    selected: bool,
    bytes: Vec<u8>,
    rgba: Vec<u8>,
    size: (u32, u32),
    orientation: u32,
    count: u16,
    losses: Vec<&'static str>,
}
impl Case {
    fn new(name: &'static str, page: u16, selected: bool) -> Self {
        let data: Value =
            serde_json::from_slice(&fs::read(fixtures().join("oracle.json")).unwrap()).unwrap();
        let row = data["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["file"] == name)
            .unwrap();
        let bytes = fs::read(fixtures().join(name)).unwrap();
        assert_eq!(hash(&bytes), row["sha256"]);
        let pages = row["pages"].as_array().unwrap();
        let p = &pages[usize::from(page)];
        let rgba: Vec<_> = p["rgba"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(hash(&rgba), p["rgba_sha256"]);
        let mut losses = Vec::new();
        if name.starts_with("gray-") {
            losses.push(DEPTH);
        }
        if pages.len() > 1 {
            losses.push(PAGES);
        }
        Self {
            name,
            page,
            selected,
            bytes,
            rgba,
            size: (
                p["width"].as_u64().unwrap() as u32,
                p["height"].as_u64().unwrap() as u32,
            ),
            orientation: p["orientation"].as_u64().unwrap() as u32,
            count: pages.len() as u16,
            losses,
        }
    }
    fn tag(&self) -> String {
        format!(
            "{}-{}-{}",
            if self.selected {
                "selected"
            } else {
                "ordinary"
            },
            self.name,
            self.page
        )
    }
    fn assert_doc(&self, doc: &Document) {
        assert_eq!(doc.canvas, self.size, "{}", self.tag());
        assert_eq!(doc.roots.len(), 1);
        let mut rgba = Vec::new();
        for y in 0..self.size.1 {
            for x in 0..self.size.0 {
                rgba.extend(
                    doc.layers[&doc.roots[0]]
                        .raster
                        .get(i64::from(x), i64::from(y))
                        .0,
                );
            }
        }
        assert_eq!(rgba, self.rgba, "independent raw hidden RGB and alpha");
    }
    fn check_report(&self, report: &Value) -> String {
        assert_eq!(report["policy"], POLICY);
        assert_eq!(report["source_sha256"], hash(&self.bytes));
        assert_eq!(report["source_bytes"], self.bytes.len());
        assert_eq!(report["page_index"], self.page);
        assert_eq!(report["page_count"], self.count);
        assert_eq!(report["width"], self.size.0);
        assert_eq!(report["height"], self.size.1);
        assert_eq!(report["source_width"], 3);
        assert_eq!(report["source_height"], 2);
        assert_eq!(report["source_orientation"], self.orientation);
        assert_eq!(report["normalized_orientation"], 1);
        assert_eq!(report["descriptor"]["orientation"], self.orientation);
        assert_eq!(
            report["descriptor"]["little_endian"],
            self.bytes.starts_with(b"II")
        );
        assert_eq!(report["output_sha256"], hash(&self.rgba));
        assert_eq!(
            report["source_depth"],
            if self.name.starts_with("gray-") {
                16
            } else {
                8
            }
        );
        assert_eq!(report["output_depth"], 8);
        equal_losses(&report["losses"], &self.losses);
        assert_eq!(report["document_identity"], report["assessment_sha256"]);
        report["document_identity"].as_str().unwrap().to_string()
    }
}
fn equal_losses(value: &Value, losses: &[&str]) {
    let mut actual: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    actual.sort_unstable();
    let mut expected = losses.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}
fn read(path: &Path) -> Document {
    pcnd::read(&fs::read(path).unwrap()).unwrap()
}
fn ids(a: &Document, b: &Document) {
    assert_eq!(a.document_id, b.document_id);
    assert_eq!(a.state_id, b.state_id);
    assert_eq!(a.roots, b.roots);
}
fn cli(case: &Case, input: &Path, output: &Path, losses: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
    command
        .arg("import")
        .arg(input)
        .arg("-o")
        .arg(output)
        .args(["--overwrite", "--frontend", "studio", "--json"]);
    if case.selected {
        command.args(["--page", &case.page.to_string()]);
    }
    if !losses.is_empty() {
        command.args(["--allow-loss", &losses.join(",")]);
    }
    command.output().unwrap()
}
fn ok(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn client() -> mcp_client::Client {
    mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")))
}
fn args(case: &Case, input: &Path, losses: &[&str]) -> Value {
    let mut args = json!({"path":input,"allow_losses":losses});
    if case.selected {
        args["page"] = json!(case.page);
    }
    args
}
fn cases() -> Vec<Case> {
    let mut cases: Vec<_> = (0..8)
        .map(|page| Case::new("rgba8-le-eight-orientations.tif", page, true))
        .collect();
    for name in [
        "rgba8-le-orientation-5.tif",
        "rgba8-be-orientation-8.tif",
        "gray-alpha16-be-orientation-2.tif",
        "gray-alpha16-be-orientation-7.tif",
    ] {
        cases.push(Case::new(name, 0, true));
    }
    for name in ["rgba8-le-orientation-5.tif", "rgba8-be-orientation-8.tif"] {
        cases.push(Case::new(name, 0, false));
    }
    cases
}
#[test]
fn all_eight_pages_and_both_source_policies_match_independent_oracles_and_native_copies() {
    let folder = Folder::new();
    let mut source_page_ids = std::collections::HashSet::new();
    for case in cases() {
        let input = folder.path(case.name);
        fs::write(&input, &case.bytes).unwrap();
        let mut previous = None;
        for repeat in 0..2 {
            let output = folder.path(&format!("{}-{repeat}.pcnd", case.tag()));
            let reply = ok(cli(&case, &input, &output, &case.losses));
            let doc = read(&output);
            case.assert_doc(&doc);
            let identity = if case.selected {
                case.check_report(&reply["engine"]["source_assessment"])
            } else {
                let decoded = image::Decoded {
                    format: image::ImageFormat::Tiff,
                    width: case.size.0,
                    height: case.size.1,
                    rgba: case.rgba.clone(),
                    losses: vec![],
                };
                image::document_conversion_key(&case.bytes, &decoded, &|| Ok(()))
                    .unwrap()
                    .unwrap()
                    .hex()
            };
            let expected =
                paintcraft_document::import_image(case.size.0, case.size.1, &case.rgba, &identity)
                    .unwrap();
            ids(&doc, &expected);
            let legacy = paintcraft_document::import_image(
                case.size.0,
                case.size.1,
                &case.rgba,
                &hash(&case.bytes),
            )
            .unwrap();
            assert_ne!(doc.document_id, legacy.document_id);
            assert_ne!(doc.state_id, legacy.state_id);
            if let Some(previous) = &previous {
                ids(previous, &doc);
            }
            previous = Some(doc.clone());
            if repeat == 0 && case.name == "rgba8-le-eight-orientations.tif" {
                assert!(source_page_ids.insert(doc.state_id));
            }
            let mut mcp = client();
            let result = mcp.ok("import_image", args(&case, &input, &case.losses));
            if case.selected {
                assert_eq!(
                    case.check_report(&result["conversion"]["source_assessment"]),
                    identity
                );
            }
            let saved = folder.path(&format!("mcp-{}-{repeat}.pcnd", case.tag()));
            mcp.ok("save_project", json!({"path":saved}));
            let peer = read(&saved);
            case.assert_doc(&peer);
            ids(&doc, &peer);
            if repeat == 0
                && let Some(root) = std::env::var_os("PAINTCRAFT_TIFF_ORIENTATION_EVIDENCE")
            {
                let root = PathBuf::from(root);
                let native = root.join("native").join(format!("{}.pcnd", case.tag()));
                let has_native = case.name != "rgba8-le-eight-orientations.tif"
                    || [0, 3, 5, 7].contains(&case.page);
                if has_native {
                    let peer = read(&native);
                    case.assert_doc(&peer);
                    ids(&doc, &peer);
                }
                for (transport, path) in [("cli", &output), ("mcp", &saved)] {
                    let target = root.join(transport);
                    fs::create_dir_all(&target).unwrap();
                    fs::copy(path, target.join(format!("{}.pcnd", case.tag()))).unwrap();
                }
                println!(
                    "{} exact raw pixels, policy IDs and saved CLI/MCP{} copies agree",
                    case.tag(),
                    if has_native { "/native" } else { "" }
                );
            }
        }
        assert_eq!(fs::read(&input).unwrap(), case.bytes);
    }
    assert_eq!(source_page_ids.len(), 8);
}
#[test]
fn orientation_losses_refusals_and_saved_source_aliases_preserve_dirty_history() {
    let folder = Folder::new();
    let case = Case::new("gray-alpha16-be-orientation-7.tif", 0, true);
    let input = folder.path("source.tif");
    fs::write(&input, &case.bytes).unwrap();
    let dest = folder.path("existing.pcnd");
    fs::write(&dest, b"prior destination").unwrap();
    assert!(!cli(&case, &input, &dest, &[]).status.success());
    assert_eq!(fs::read(&dest).unwrap(), b"prior destination");
    let mut mcp = client();
    mcp.ok("create_canvas", json!({"width":3,"height":2}));
    mcp.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":2,"y":1}],"color":"#123456FF","size":1,"brush":"round"}));
    let before = mcp.ok("inspect_document", json!({}));
    for allowed in [
        vec![],
        vec!["tiff-metadata-ignored"],
        vec!["unknown-orientation-loss"],
    ] {
        let mut a = args(&case, &input, &allowed);
        a["discard_changes"] = json!(true);
        assert_eq!(mcp.call("import_image", a)["isError"], true);
        assert_eq!(mcp.ok("inspect_document", json!({})), before);
    }
    mcp.ok("history", json!({}));
    assert_ne!(
        mcp.ok("inspect_document", json!({}))["state_id"],
        before["state_id"]
    );
    mcp.ok("history", json!({"redo":true}));
    assert_eq!(mcp.ok("inspect_document", json!({})), before);
    let multi = Case::new("rgba8-le-eight-orientations.tif", 0, true);
    let source = folder.path(multi.name);
    fs::write(&source, &multi.bytes).unwrap();
    let mut no_page = args(&multi, &source, &[PAGES]);
    no_page.as_object_mut().unwrap().remove("page");
    no_page["discard_changes"] = json!(true);
    assert_eq!(mcp.call("import_image", no_page)["isError"], true);
    assert_eq!(mcp.ok("inspect_document", json!({})), before);
    let mut adopt = args(&case, &input, &[DEPTH]);
    adopt["discard_changes"] = json!(true);
    mcp.ok("import_image", adopt);
    let copy = folder.path("saved.pcnd");
    mcp.ok("save_project", json!({"path":copy}));
    case.assert_doc(&read(&copy));
    let state = mcp.ok("inspect_document", json!({}));
    let moved = folder.path("retained.tif");
    fs::rename(&input, &moved).unwrap();
    let alias = folder.path("alias.pcnd");
    fs::hard_link(&moved, &alias).unwrap();
    assert_eq!(
        mcp.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(mcp.ok("inspect_document", json!({})), state);
    assert_eq!(fs::read(&moved).unwrap(), case.bytes);
}
