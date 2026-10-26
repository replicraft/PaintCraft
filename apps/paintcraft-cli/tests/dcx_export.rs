//! Production PCND/session -> single-page DCX routes, without launching a window.
use paintcraft_core::types::Rgba;
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_formats::{dcx, pcnd};
use paintcraft_raster::{Admission, RasterLayer};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;

fn document() -> Document {
    let mut doc = new_document(3, 2, Some(Rgba([0, 0, 0, 255])), 0, POLICY).unwrap();
    let rgba: Vec<u8> = expected()
        .into_iter()
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster = RasterLayer::from_rgba(
        3,
        2,
        Rgba([0, 0, 0, 255]),
        &rgba,
        &mut Admission::new(65536),
    )
    .unwrap();
    doc
}
fn expected() -> [[u8; 3]; 6] {
    [
        [193, 255, 192],
        [17, 51, 93],
        [193, 255, 192],
        [0, 7, 255],
        [12, 34, 56],
        [17, 51, 93],
    ]
}
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn assert_output(path: &Path, indexed: bool) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(dcx::inspect(&bytes).unwrap().len(), 1);
    let decoded = dcx::decode_page(&bytes, 0, 6).unwrap();
    assert_eq!(
        decoded.rgba,
        expected()
            .into_iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect::<Vec<_>>()
    );
    assert_eq!(bytes[4100 + 65] == 1, indexed);
    if indexed {
        assert_eq!(
            &bytes[bytes.len() - 768..][..12],
            &[193, 255, 192, 17, 51, 93, 0, 7, 255, 12, 34, 56]
        );
    }
}
#[test]
fn cli_explicit_modes_receipts_source_guards_and_consumer_files() {
    let root = std::env::temp_dir().join(format!("paintcraft-cli-dcx-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.pcnd");
    let original = pcnd::write(&document()).unwrap();
    fs::write(&source, &original).unwrap();
    for mode in ["rgb24", "indexed-exact", "quantized256"] {
        let output = root.join(format!("{mode}.dcx"));
        let result = run(&[
            "export-dcx",
            source.to_str().unwrap(),
            "--pcx-mode",
            mode,
            "--output",
            output.to_str().unwrap(),
            "--overwrite",
            "--json",
        ]);
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(receipt["outcome"], "success");
        assert_output(&output, mode != "rgb24");
    }
    for extra in [
        vec![],
        vec!["--pcx-mode", "rgb24", "--frontend", "classic-v11"],
        vec!["--pcx-mode", "rgb24", "--allow-loss", "invented"],
    ] {
        let output = root.join("denied.dcx");
        let mut args = vec![
            "export-dcx",
            source.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ];
        args.extend(extra);
        assert!(!run(&args).status.success());
        assert!(!output.exists());
    }
    let alpha_source = root.join("alpha.pcnd");
    let alpha_doc = new_document(1, 1, Some(Rgba([17, 51, 93, 128])), 0, POLICY).unwrap();
    fs::write(&alpha_source, pcnd::write(&alpha_doc).unwrap()).unwrap();
    let alpha_output = root.join("alpha.dcx");
    let alpha_args = [
        "export-dcx",
        alpha_source.to_str().unwrap(),
        "--pcx-mode",
        "rgb24",
        "--output",
        alpha_output.to_str().unwrap(),
    ];
    assert!(!run(&alpha_args).status.success());
    assert!(!alpha_output.exists());
    let mut allowed = alpha_args.to_vec();
    allowed.extend(["--allow-loss", "alpha-discarded", "--json"]);
    let result = run(&allowed);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&alpha_output).unwrap();
    assert_eq!(
        dcx::decode_page(&bytes, 0, 1).unwrap().rgba,
        [136, 153, 174, 255]
    );
    let alias = root.join("alias.dcx");
    fs::hard_link(&source, &alias).unwrap();
    assert!(
        !run(&[
            "export-dcx",
            source.to_str().unwrap(),
            "--pcx-mode",
            "rgb24",
            "--output",
            alias.to_str().unwrap(),
            "--overwrite"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&source).unwrap(), original);
    if let Some(folder) = std::env::var_os("PAINTCRAFT_DCX_CONSUMER_OUTPUT") {
        let folder = std::path::PathBuf::from(folder);
        for mode in ["rgb24", "indexed-exact", "quantized256"] {
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(folder.join(format!("cli-{mode}.dcx")))
                .unwrap();
            std::io::Write::write_all(
                &mut output,
                &fs::read(root.join(format!("{mode}.dcx"))).unwrap(),
            )
            .unwrap();
        }
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn mcp_keeps_document_dirty_state_and_protects_renamed_native_project() {
    let root = std::env::temp_dir().join(format!("paintcraft-mcp-dcx-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.pcnd");
    let original = pcnd::write(&document()).unwrap();
    fs::write(&source, &original).unwrap();
    let mut client = Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("open_project", json!({"path":source}));
    let before = client.ok("inspect_document", json!({}));
    let renamed = root.join("renamed.pcnd");
    fs::rename(&source, &renamed).unwrap();
    for mode in ["rgb24", "indexed-exact", "quantized256"] {
        let output = root.join(format!("{mode}.dcx"));
        client.ok("export_dcx", json!({"path":output,"pcx_mode":mode}));
        assert_output(&output, mode != "rgb24");
    }
    let alias = root.join("alias.dcx");
    fs::hard_link(&renamed, &alias).unwrap();
    assert_eq!(
        client.call(
            "export_dcx",
            json!({"path":alias,"pcx_mode":"rgb24","overwrite":true})
        )["isError"],
        true
    );
    let unrelated = root.join("after-alias.dcx");
    client.ok(
        "export_dcx",
        json!({"path":unrelated,"pcx_mode":"indexed-exact"}),
    );
    assert_output(&unrelated, true);
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&renamed).unwrap(), original);
    let invalid = root.join("invalid.dcx");
    assert_eq!(
        client.call("export_dcx", json!({"path":invalid}))["isError"],
        true
    );
    assert!(!invalid.exists());
    if let Some(folder) = std::env::var_os("PAINTCRAFT_DCX_CONSUMER_OUTPUT") {
        let folder = std::path::PathBuf::from(folder);
        for mode in ["rgb24", "indexed-exact", "quantized256"] {
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(folder.join(format!("mcp-{mode}.dcx")))
                .unwrap();
            std::io::Write::write_all(
                &mut output,
                &fs::read(root.join(format!("{mode}.dcx"))).unwrap(),
            )
            .unwrap();
        }
    }
    drop(client);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mcp_actual_cancelled_rpc_preserves_session_existing_destination_and_cleans_stage() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-mcp-dcx-cancel-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let output = root.join("existing.dcx");
    let original = b"retain prior destination";
    fs::write(&output, original).unwrap();
    let source = root.join("source.pcnd");
    let doc = new_document(2048, 2048, Some(Rgba([17, 51, 93, 255])), 0, POLICY).unwrap();
    let source_bytes = pcnd::write(&doc).unwrap();
    fs::write(&source, &source_bytes).unwrap();
    let mut client = Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok("open_project", json!({"path":source}));
    let before = client.ok("inspect_document", json!({}));
    client.cancel_call(
        "export_dcx",
        json!({"path":output,"pcx_mode":"indexed-exact","overwrite":true}),
    );
    // This waits for the same session mutex after cancellation; no timing sleep.
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&output).unwrap(), original);
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}
