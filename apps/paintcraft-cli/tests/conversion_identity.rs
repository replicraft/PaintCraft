//! Real CLI and stdio MCP conversion identities, distinct from original file digests.
use paintcraft_document::Document;
use serde_json::json;
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
            .join("../../docs/validation/conversion-identity/runs")
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
fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 4100];
    bytes[..4].copy_from_slice(&987_654_321u32.to_le_bytes());
    for (index, rgb) in [[10, 20, 30], [200, 210, 220]].into_iter().enumerate() {
        let offset = bytes.len() as u32;
        bytes[4 + index * 4..8 + index * 4].copy_from_slice(&offset.to_le_bytes());
        let mut pcx = vec![0; 128];
        pcx[..4].copy_from_slice(&[10, 5, 1, 8]);
        pcx[65] = 3;
        pcx[66..68].copy_from_slice(&2u16.to_le_bytes());
        for value in rgb {
            if value >= 192 {
                pcx.extend([0xc1, value]);
            } else {
                pcx.push(value);
            }
            pcx.push(0);
        }
        bytes.extend(pcx);
    }
    bytes
}

fn read(path: &Path) -> Document {
    paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap()
}
fn assert_identity(docs: &[Document]) {
    for (doc, rgb) in docs
        .iter()
        .zip([[10, 20, 30], [200, 210, 220], [200, 210, 220]])
    {
        assert_eq!(
            doc.layers[&doc.roots[0]].raster.get(0, 0).0,
            [rgb[0], rgb[1], rgb[2], 255]
        );
    }
    assert_ne!(docs[0].document_id, docs[1].document_id);
    assert_ne!(docs[0].state_id, docs[1].state_id);
    assert_ne!(docs[0].roots, docs[1].roots);
    assert_eq!(docs[1].document_id, docs[2].document_id);
    assert_eq!(docs[1].state_id, docs[2].state_id);
    assert_eq!(docs[1].roots, docs[2].roots);
}
#[test]
fn real_cli_and_stdio_mcp_selected_pages_bind_actual_pixels_to_stable_distinct_ids() {
    let folder = Folder::new();
    let source = folder.0.join("pages.dcx");
    let bytes = fixture();
    fs::write(&source, &bytes).unwrap();
    let mut cli_docs = Vec::new();
    let mut mcp_docs = Vec::new();
    for (i, page) in [0u16, 1, 1].into_iter().enumerate() {
        let output = folder.0.join(format!("cli-{i}.pcnd"));
        let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .arg("import")
            .arg(&source)
            .arg("-o")
            .arg(&output)
            .args([
                "--frontend",
                "studio",
                "--page",
                &page.to_string(),
                "--allow-loss",
                "pcx-metadata-ignored,dcx-other-pages-not-imported",
                "--json",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        cli_docs.push(read(&output));
        let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        client.ok(
            "import_image",
            json!({"path":source,"page":page,"allow_losses":["pcx-metadata-ignored","dcx-other-pages-not-imported"]}),
        );
        let saved = folder.0.join(format!("mcp-{i}.pcnd"));
        client.ok("save_project", json!({"path":saved}));
        mcp_docs.push(read(&saved));
    }
    assert_identity(&cli_docs);
    assert_identity(&mcp_docs);
    for (a, b) in cli_docs.iter().zip(&mcp_docs) {
        assert_eq!(a.document_id, b.document_id);
        assert_eq!(a.state_id, b.state_id);
        assert_eq!(a.roots, b.roots);
    }
    assert_eq!(fs::read(&source).unwrap(), bytes);
}
