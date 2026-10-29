//! Host-only omission authority through actual installed CLI/stdio JXR copies.
use super::*;
use serde_json::{Value as Json, json};
#[test]
fn installed_jxr_metadata_omission_is_explicit_exact_and_preserves_authoring_state() {
    let installation = Installation::new();
    let (profile, _) = codec_contract::helper_contract(
        &fs::read(&installation.helper).unwrap(),
        env!("CARGO_PKG_VERSION"),
    )
    .unwrap();
    let writer = profile == wire::jxr_encode::qualified::NATIVE_PROFILE
        || profile == wire::jxl_integer::contracts::writer::NATIVE_PROFILE;
    let rgba = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/jxr-export-fixtures/4x3.rgba"
    ));
    let golden = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/jxr-export-fixtures/4x3.jxr"
    ));
    let reference: Json = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/jxr-export-fixtures/provenance.json"
    )))
    .unwrap();
    assert_eq!(
        reference["reference_sha256"],
        "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce"
    );
    let case = &reference["cases"][0];
    assert_eq!(case["name"], "4x3");
    assert_eq!(case["input_rgba_sha256"], hex(&Sha256::digest(rgba)));
    assert_eq!(case["encoded_sha256"], hex(&Sha256::digest(golden)));
    let mut document =
        paintcraft_document::import_image(4, 3, rgba, "installed-jxr-metadata-copy").unwrap();
    document.source_metadata = paintcraft_formats::pcnd::read(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    )))
    .unwrap()
    .source_metadata;
    assert!(document.source_metadata.is_some());
    let original = paintcraft_formats::pcnd::write_compact(&document).unwrap();
    let source = installation.fixture("source-metadata.pcnd", &original);
    let mut mcp = mcp_client::Client::at(&installation.cli);
    mcp.ok("open_project", json!({"path":source}));
    let state = mcp.ok("inspect_document", json!({}));
    for (name, intent) in [
        ("default", None),
        ("refuse", Some("refuse")),
        ("omit", Some("omit")),
    ] {
        let expected = writer && intent == Some("omit");
        let cli = installation.root.join(format!("metadata-cli-{name}.jxr"));
        let remote = installation.root.join(format!("metadata-mcp-{name}.jxr"));
        fs::write(&cli, b"existing").unwrap();
        fs::write(&remote, b"existing").unwrap();
        let mut command = Command::new(&installation.cli);
        command
            .arg("export-jxr")
            .arg(&source)
            .arg("--output")
            .arg(&cli)
            .args(["--overwrite", "--json"]);
        if let Some(intent) = intent {
            command.args(["--source-metadata", intent]);
        }
        let result = command.output().unwrap();
        assert_eq!(
            result.status.success(),
            expected,
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let mut args = json!({"path":remote,"overwrite":true,"allow_losses":[]});
        if let Some(intent) = intent {
            args["source_metadata"] = intent.into();
        }
        let result = mcp.call("export_jxr", args);
        assert_eq!(result["isError"] == true, !expected, "{result}");
        if expected {
            assert_eq!(fs::read(&cli).unwrap(), golden);
            assert_eq!(fs::read(&remote).unwrap(), golden);
            assert!(
                result
                    .to_string()
                    .contains(paintcraft_formats::export::metadata_policy::OMITTED_REASON)
            );
        } else {
            assert_eq!(fs::read(&cli).unwrap(), b"existing");
            assert_eq!(fs::read(&remote).unwrap(), b"existing");
        }
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(mcp.ok("inspect_document", json!({})), state);
    }
}
