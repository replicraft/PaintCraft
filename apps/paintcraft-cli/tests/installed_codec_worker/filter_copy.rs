//! Exact golden native writer output after explicit editable-intention consent.
use super::*;
use paintcraft_core::types::filters::Descriptor;
use paintcraft_document::{FilterInstance, FilterStack};
use serde_json::json;
#[test]
fn installed_jxr_filter_copy_requires_explicit_bake_and_keeps_editable_authoring() {
    let installation = Installation::new();
    let (profile, _) = codec_contract::helper_contract(
        &fs::read(&installation.helper).unwrap(),
        env!("CARGO_PKG_VERSION"),
    )
    .unwrap();
    let writer = profile == wire::jxr_encode::qualified::NATIVE_PROFILE
        || profile == wire::jxl_integer::contracts::writer::NATIVE_PROFILE;
    eprintln!("filter-copy actual helper profile={profile}; writer={writer}");
    let rgba = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/jxr-export-fixtures/4x3.rgba"
    ));
    let golden = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/jxr-export-fixtures/4x3.jxr"
    ));
    for enabled in [true, false] {
        let source_pixels: Vec<u8> = rgba
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if enabled && index % 4 != 3 {
                    255 - *value
                } else {
                    *value
                }
            })
            .collect();
        let mut document =
            paintcraft_document::import_image(4, 3, &source_pixels, "installed-filter-copy-golden")
                .unwrap();
        document.layers.get_mut(&document.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(1),
                enabled,
                Descriptor::InvertRgbV1,
            )])
            .unwrap();
        document.state_id = uuid::Uuid::from_u128(if enabled { 700 } else { 701 });
        document.source_metadata=paintcraft_formats::pcnd::read(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"))).unwrap().source_metadata;
        let original = paintcraft_formats::pcnd::write_compact(&document).unwrap();
        let source = installation.fixture(&format!("filters-{enabled}.pcnd"), &original);
        let mut client = mcp_client::Client::at(&installation.cli);
        client.ok("open_project", json!({"path":source}));
        let before = client.ok("inspect_document", json!({}));
        for (name, omit, bake) in [
            ("metadata-only", true, false),
            ("filter-only", false, true),
            ("both", true, true),
        ] {
            let success = writer && omit && bake;
            let cli = installation
                .root
                .join(format!("filter-cli-{enabled}-{name}.jxr"));
            let remote = installation
                .root
                .join(format!("filter-mcp-{enabled}-{name}.jxr"));
            for p in [&cli, &remote] {
                fs::write(p, b"existing destination").unwrap();
            }
            let mut command = Command::new(&installation.cli);
            command
                .arg("export-jxr")
                .arg(&source)
                .arg("--output")
                .arg(&cli)
                .args(["--overwrite", "--json"]);
            if omit {
                command.args(["--source-metadata", "omit"]);
            }
            if bake {
                command.args(["--allow-loss", "filters-baked"]);
            }
            let output = command.output().unwrap();
            assert_eq!(
                output.status.success(),
                success,
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let mut args = json!({"path":remote,"overwrite":true,"allow_losses":if bake{vec!["filters-baked"]}else{vec![]}});
            if omit {
                args["source_metadata"] = "omit".into();
            }
            let result = client.call("export_jxr", args);
            assert_eq!(result["isError"] != true, success, "{result}");
            if success {
                // Active invert restores the independently C-qualified golden
                // plane from its authored RGB complement. Disabled keeps it.
                // Dropping the active filter would produce different bytes.
                assert_eq!(fs::read(&cli).unwrap(), golden);
                assert_eq!(fs::read(&remote).unwrap(), golden);
                assert!(String::from_utf8_lossy(&output.stdout).contains("filters-baked"));
                assert!(result.to_string().contains("filters-baked"));
            } else {
                for p in [&cli, &remote] {
                    assert_eq!(fs::read(p).unwrap(), b"existing destination");
                }
            }
            assert_eq!(fs::read(&source).unwrap(), original);
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
    }
}
