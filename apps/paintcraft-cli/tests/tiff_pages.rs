//! Real CLI/stdio MCP selected-page workflows against independent libtiff samples.
use paintcraft_document::Document;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
const META: &str = "tiff-metadata-ignored";
const PAGES: &str = "tiff-other-pages-not-imported";
const DEPTH: &str = "depth-16-reduced-to-8";
const ICC: &str = "icc-profile-ignored";
const POLICY: &str = "tiff-uint16-high-byte-rgba8-v1";
const PROFILE: &str = "tiff-selected-page-rgba8-v1";
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/tiff-cli")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn source(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::copy(fixtures().join(name), &path).unwrap();
        path
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/tiff-fixtures")
}
fn oracle(name: &str, page: usize) -> Value {
    let manifest: Value =
        serde_json::from_slice(&fs::read(fixtures().join("oracle.json")).unwrap()).unwrap();
    manifest["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["file"] == name)
        .unwrap()["pages"][page]
        .clone()
}
fn hash(bytes: &[u8]) -> String {
    paintcraft_document::hex(&Sha256::digest(bytes))
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
fn cli(input: &Path, output: &Path, page: Option<u16>, losses: &[&str], frontend: &str) -> Output {
    let mut args = vec![
        "import".into(),
        input.to_string_lossy().into_owned(),
        "-o".into(),
        output.to_string_lossy().into_owned(),
        "--overwrite".into(),
        "--frontend".into(),
        frontend.into(),
        "--json".into(),
    ];
    if let Some(page) = page {
        args.extend(["--page".into(), page.to_string()]);
    }
    if !losses.is_empty() {
        args.extend(["--allow-loss".into(), losses.join(",")]);
    }
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn expected(page: &Value) -> Vec<u8> {
    page["expected_rgba8"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| u8::try_from(value.as_u64().unwrap()).unwrap())
        .collect()
}
fn pixels(path: &Path, page: &Value) -> Document {
    let doc = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    let width = page["width"].as_u64().unwrap();
    let height = page["height"].as_u64().unwrap();
    assert_eq!(doc.canvas, (width as u32, height as u32));
    assert_eq!(doc.roots.len(), 1);
    for (index, pixel) in expected(page).as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            doc.layers[&doc.roots[0]]
                .raster
                .get((index as u64 % width) as i64, (index as u64 / width) as i64)
                .0,
            *pixel,
            "raw sample {index}; do not flatten away hidden RGB"
        );
    }
    doc
}
fn equal_losses(actual: &Value, expected: &[&str]) {
    let mut actual: Vec<_> = actual
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}
fn assessment(value: &Value, input: &Path, page: &Value, index: u16, count: u16, losses: &[&str]) {
    let bytes = fs::read(input).unwrap();
    assert_eq!(value["policy"], POLICY);
    assert_eq!(value["profile_variant"], PROFILE);
    assert_eq!(value["source_sha256"], hash(&bytes));
    assert_eq!(value["source_bytes"], bytes.len() as u64);
    assert_eq!(value["page_index"], index);
    assert_eq!(value["page_count"], count);
    assert_eq!(value["width"], page["width"]);
    assert_eq!(value["height"], page["height"]);
    assert_eq!(value["source_depth"], page["bits"]);
    assert_eq!(value["output_depth"], 8);
    assert_eq!(value["output_sha256"], page["expected_rgba8_sha256"]);
    assert_eq!(value["output_sha256"], hash(&expected(page)));
    equal_losses(&value["losses"], losses);
    let digest = value["assessment_sha256"].as_str().unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(value["document_identity"], digest);
    let descriptor = &value["descriptor"];
    assert!(descriptor["ifd_offset"].as_u64().unwrap() >= 8);
    assert_eq!(descriptor["little_endian"], bytes.starts_with(b"II"));
    assert_eq!(descriptor["bits_per_sample"]["values"][0], page["bits"]);
    assert_eq!(
        descriptor["sample_format"]["values"][0],
        page["sample_format"]
    );
    assert_eq!(descriptor["samples_per_pixel"], page["channels"]);
    assert_eq!(descriptor["photometric"], page["photometric"]);
    assert_eq!(descriptor["compression"], page["compression"]);
    assert_eq!(
        descriptor["storage"],
        if page["tiled"] == true { 2 } else { 1 }
    );
    assert_eq!(descriptor["orientation"], 1);
    assert_eq!(descriptor["planar_configuration"], 1);
    assert!(descriptor["metadata_bytes"].as_u64().unwrap() > 0);
    assert_eq!(descriptor["omitted_metadata"], true);
    assert_eq!(descriptor["resolution_unit"], 2);
    for (key, dpi) in [("resolution_x", 96), ("resolution_y", 72)] {
        let n = descriptor[key][0].as_u64().unwrap();
        let d = descriptor[key][1].as_u64().unwrap();
        assert_ne!(d, 0);
        assert_eq!(n, d * dpi);
    }
    assert_eq!(descriptor["extra_samples"]["values"][0], page["extra"]);
    if page["icc_hex"].as_str().unwrap().is_empty() {
        assert_eq!(descriptor["icc_bytes"], 0);
        assert!(descriptor["icc_sha256"].is_null());
    } else {
        let hex = page["icc_hex"].as_str().unwrap();
        let icc = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(descriptor["icc_bytes"], icc.len() as u64);
        assert_eq!(descriptor["icc_sha256"], hash(&icc));
    }
}
fn client() -> Client {
    Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")))
}
fn dirty(client: &mut Client) -> Value {
    client.ok("create_canvas", json!({"width":3,"height":2}));
    client.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":2,"y":1}],"color":"#123456FF","size":1,"brush":"round"}));
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(before["dirty"], true);
    before
}
#[test]
fn independent_integer_gray_strip_tile_and_profile_cases_reach_cli_and_stdio_mcp_without_hidden_rgb_loss()
 {
    let cases: [(&str, u16, u16, &[&str]); 19] = [
        ("rgba8-le-strips-none.tif", 0, 2, &[META, PAGES]),
        ("rgba8-le-strips-none.tif", 1, 2, &[META, PAGES]),
        ("rgba16-be-strips-lzw.tif", 0, 2, &[META, PAGES, DEPTH]),
        ("rgba8-le-tiles-deflate.tif", 0, 2, &[META, PAGES]),
        ("rgba8-le-tiles-deflate.tif", 1, 2, &[META, PAGES, DEPTH]),
        ("gray-endian-packbits.tif", 0, 2, &[META, PAGES]),
        ("gray-endian-packbits.tif", 1, 2, &[META, PAGES, DEPTH]),
        ("gray1-le-fax4.tif", 0, 1, &[META]),
        ("icc-srgb-rgba8.tif", 0, 1, &[META, ICC]),
        ("unsupported-first-rgba16.tif", 1, 2, &[META, PAGES, DEPTH]),
        ("associated-first-rgba8.tif", 1, 2, &[META, PAGES]),
        ("float-first-rgba8.tif", 1, 2, &[META, PAGES]),
        ("jpeg-gray-rgb8.tif", 0, 2, &[META, PAGES]),
        ("jpeg-gray-rgb8.tif", 1, 2, &[META, PAGES]),
        ("gray-alpha-8-16-both-photos.tif", 0, 4, &[META, PAGES]),
        ("gray-alpha-8-16-both-photos.tif", 1, 4, &[META, PAGES]),
        (
            "gray-alpha-8-16-both-photos.tif",
            2,
            4,
            &[META, PAGES, DEPTH],
        ),
        (
            "gray-alpha-8-16-both-photos.tif",
            3,
            4,
            &[META, PAGES, DEPTH],
        ),
        ("icc-srgb-larger-rgba8.tif", 0, 1, &[META, ICC]),
    ];
    for (name, index, count, losses) in cases {
        let folder = Folder::new();
        let input = folder.source(name);
        let original = fs::read(&input).unwrap();
        let output = folder.path("selected.pcnd");
        let page = oracle(name, index as usize);
        let receipt = ok(cli(&input, &output, Some(index), losses, "studio"));
        equal_losses(&receipt["file"]["losses_acknowledged"], losses);
        assert_eq!(receipt["engine"]["decoder_profile"], PROFILE);
        assessment(
            &receipt["engine"]["source_assessment"],
            &input,
            &page,
            index,
            count,
            losses,
        );
        let direct = pixels(&output, &page);
        let mut c = client();
        let adopted = c.ok(
            "import_image",
            json!({"path":input,"page":index,"allow_losses":losses}),
        );
        assert_eq!(adopted["dirty"], true);
        assert_eq!(adopted["conversion"]["preserving_save"], false);
        assert_eq!(adopted["conversion"]["profile_variant"], PROFILE);
        equal_losses(&adopted["conversion"]["losses_acknowledged"], losses);
        assessment(
            &adopted["conversion"]["source_assessment"],
            &input,
            &page,
            index,
            count,
            losses,
        );
        assert_eq!(
            adopted["conversion"]["source_assessment"],
            receipt["engine"]["source_assessment"]
        );
        let saved = folder.path("mcp.pcnd");
        c.ok("save_project", json!({"path":saved}));
        let via_mcp = pixels(&saved, &page);
        assert_eq!(direct.document_id, via_mcp.document_id);
        assert_eq!(direct.state_id, via_mcp.state_id);
        assert_eq!(direct.roots, via_mcp.roots);
        assert_eq!(fs::read(input).unwrap(), original);
    }
}
#[test]
fn explicit_page_and_every_loss_are_required_and_refusals_preserve_destinations_and_dirty_mcp_history()
 {
    let folder = Folder::new();
    let input = folder.source("unsupported-first-rgba16.tif");
    let original = fs::read(&input).unwrap();
    let dest = folder.path("existing.pcnd");
    fs::write(&dest, b"original destination").unwrap();
    for (page, losses) in [
        (None, vec![META, PAGES, DEPTH]),
        (Some(1), vec![]),
        (Some(1), vec![META, PAGES]),
        (Some(1), vec![META, DEPTH]),
        (Some(1), vec![PAGES, DEPTH]),
        (Some(1), vec![META, PAGES, DEPTH, "unknown-tiff-loss"]),
        (Some(0), vec![META, PAGES, DEPTH]),
        (Some(65535), vec![META, PAGES, DEPTH]),
    ] {
        assert!(!cli(&input, &dest, page, &losses, "studio").status.success());
        assert_eq!(fs::read(&dest).unwrap(), b"original destination");
        assert_eq!(fs::read(&input).unwrap(), original);
    }
    for frontend in ["classic-v7", "classic-v10", "classic-v11"] {
        assert!(
            !cli(&input, &dest, Some(1), &[META, PAGES, DEPTH], frontend)
                .status
                .success()
        );
        assert_eq!(fs::read(&dest).unwrap(), b"original destination");
    }
    let mut c = client();
    let before = dirty(&mut c);
    for args in [
        json!({"path":input,"page":1,"allow_losses":[META,PAGES,DEPTH]}),
        json!({"path":input,"discard_changes":true,"allow_losses":[META,PAGES,DEPTH]}),
        json!({"path":input,"page":1,"discard_changes":true,"allow_losses":[META,PAGES]}),
        json!({"path":input,"page":1,"discard_changes":true,"allow_losses":[META,DEPTH]}),
        json!({"path":input,"page":1,"discard_changes":true,"allow_losses":[PAGES,DEPTH]}),
        json!({"path":input,"page":1,"discard_changes":true,"allow_losses":[META,PAGES,DEPTH,"unknown-tiff-loss"]}),
        json!({"path":input,"page":0,"discard_changes":true,"allow_losses":[META,PAGES,DEPTH]}),
        json!({"path":input,"page":65535,"discard_changes":true,"allow_losses":[META,PAGES,DEPTH]}),
    ] {
        assert_eq!(c.call("import_image", args)["isError"], true);
        assert_eq!(c.ok("inspect_document", json!({})), before);
    }
    c.ok("history", json!({}));
    assert_ne!(
        c.ok("inspect_document", json!({}))["state_id"],
        before["state_id"]
    );
    c.ok("history", json!({"redo":true}));
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&input).unwrap(), original);
}
#[test]
fn copied_projects_keep_source_alias_guards_after_save_rename_and_export() {
    let folder = Folder::new();
    let input = folder.source("rgba8-le-strips-none.tif");
    let original = fs::read(&input).unwrap();
    let alias = folder.path("alias.pcnd");
    fs::hard_link(&input, &alias).unwrap();
    assert!(
        !cli(&input, &alias, Some(1), &[META, PAGES], "studio")
            .status
            .success()
    );
    assert_eq!(fs::read(&input).unwrap(), original);
    let mut c = client();
    c.ok(
        "import_image",
        json!({"path":input,"page":1,"allow_losses":[META,PAGES]}),
    );
    let project = folder.path("first.pcnd");
    c.ok("save_project", json!({"path":project}));
    let before = c.ok("inspect_document", json!({}));
    let moved = folder.path("renamed.tif");
    fs::rename(&input, &moved).unwrap();
    let alias2 = folder.path("renamed-alias.pcnd");
    fs::hard_link(&moved, &alias2).unwrap();
    assert_eq!(
        c.call("save_project", json!({"path":alias2,"overwrite":true}))["isError"],
        true
    );
    let png_alias = folder.path("renamed-alias.png");
    fs::hard_link(&moved, &png_alias).unwrap();
    let unrelated_png = folder.path("copy.png");
    c.ok(
        "export_image",
        json!({"path":unrelated_png,"allow_losses":["flatten","hidden-rgb"]}),
    );
    let refused = c.call(
        "export_image",
        json!({"path":png_alias,"overwrite":true,"allow_losses":["flatten","hidden-rgb"]}),
    );
    assert_eq!(refused["isError"], true);
    assert!(
        refused.to_string().contains("captured imported image"),
        "{refused}"
    );
    assert_eq!(c.ok("inspect_document", json!({})), before);
    let unrelated = folder.path("second.pcnd");
    c.ok("save_project", json!({"path":unrelated}));
    pixels(&unrelated, &oracle("rgba8-le-strips-none.tif", 1));
    assert_eq!(fs::read(&moved).unwrap(), original);
}
#[test]
fn different_pages_get_distinct_ids_but_the_same_choice_is_stable_in_both_consumers() {
    let folder = Folder::new();
    let input = folder.source("rgba8-le-strips-none.tif");
    let mut docs = Vec::new();
    let mut reports = Vec::new();
    for (n, index) in [0, 1, 1].into_iter().enumerate() {
        let output = folder.path(&format!("page-{n}.pcnd"));
        let report = ok(cli(&input, &output, Some(index), &[META, PAGES], "studio"));
        docs.push(pixels(
            &output,
            &oracle("rgba8-le-strips-none.tif", index as usize),
        ));
        reports.push(report["engine"]["source_assessment"].clone());
    }
    assert_ne!(docs[0].document_id, docs[1].document_id);
    assert_ne!(docs[0].state_id, docs[1].state_id);
    assert_ne!(docs[0].roots, docs[1].roots);
    assert_eq!(docs[1].document_id, docs[2].document_id);
    assert_eq!(docs[1].state_id, docs[2].state_id);
    assert_eq!(docs[1].roots, docs[2].roots);
    assert_ne!(
        reports[0]["assessment_sha256"],
        reports[1]["assessment_sha256"]
    );
    assert_eq!(reports[1], reports[2]);
    let mut c = client();
    for (index, report) in [(0, &reports[0]), (1, &reports[1]), (1, &reports[2])] {
        let imported = c.ok(
            "import_image",
            json!({"path":input,"page":index,"discard_changes":true,"allow_losses":[META,PAGES]}),
        );
        assert_eq!(imported["conversion"]["source_assessment"], *report);
        assert_eq!(imported["state_id"], docs[index].state_id.to_string());
    }
}
#[test]
fn cancellation_of_full_source_preparation_keeps_the_live_session_history_and_source() {
    let folder = Folder::new();
    let input = folder.source("rgba8-le-strips-none.tif");
    let mut bytes = fs::read(&input).unwrap();
    bytes.resize(32 << 20, 0);
    fs::write(&input, &bytes).unwrap();
    // Establish that the same padded full-source fixture is accepted without
    // cancellation; a malformed-input refusal is not a cancellation proof.
    let baseline = folder.path("accepted-padded.pcnd");
    ok(cli(&input, &baseline, Some(1), &[META, PAGES], "studio"));
    pixels(&baseline, &oracle("rgba8-le-strips-none.tif", 1));
    let mut c = client();
    let before = dirty(&mut c);
    c.cancel_call(
        "import_image",
        json!({"path":input,"page":1,"discard_changes":true,"allow_losses":[META,PAGES]}),
    );
    assert_eq!(c.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&input).unwrap(), bytes);
    c.ok("history", json!({}));
    assert_ne!(
        c.ok("inspect_document", json!({}))["state_id"],
        before["state_id"]
    );
    c.ok("history", json!({"redo":true}));
    assert_eq!(c.ok("inspect_document", json!({})), before);
}
