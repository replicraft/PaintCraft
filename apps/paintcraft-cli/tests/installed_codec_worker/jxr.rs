use super::*;
use serde_json::json;
const LOSS: &str = "jxr-resolution-not-retained";
const RAW: &[u8] = include_bytes!(
    "../../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-4x3.jxr"
);
const DPI: &[u8] = include_bytes!(
    "../../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-dpi96x144.jxr"
);
fn exact_pixels(path: &Path) {
    let document = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(document.canvas, (4, 3));
    assert_eq!(document.roots.len(), 1);
    let raster = &document.layers_in_order().next().unwrap().raster;
    // Independently encoded/reopened original C JXRLib fixture, not selected decoder output.
    for y in 0..3 {
        for x in 0..4 {
            assert_eq!(
                raster.get(x, y).0,
                [
                    ((x * 47 + y * 13) % 256) as u8,
                    ((x * 47 + y * 13 + 71) % 256) as u8,
                    ((x * 47 + y * 13 + 142) % 256) as u8,
                    255,
                ]
            );
        }
    }
}
#[test]
fn installed_cli_jxr_requires_loss_and_preserves_source_aliases() {
    let installation = Installation::new();
    for (name, bytes) in [
        ("default.jxr", RAW),
        ("density.wdp", DPI),
        ("alias.wmp", RAW),
    ] {
        let input = installation.fixture(name, bytes);
        let output = installation.fixture("existing.pcnd", b"keep destination");
        let base = ["--frontend", "studio", "--overwrite"];
        assert_ne!(import(&installation, "import", &input, &output, &base), 0);
        assert_eq!(fs::read(&output).unwrap(), b"keep destination");
        assert_ne!(
            import(
                &installation,
                "import",
                &input,
                &output,
                &[
                    "--frontend",
                    "studio",
                    "--overwrite",
                    "--allow-loss",
                    "flatten"
                ]
            ),
            0
        );
        assert_eq!(fs::read(&output).unwrap(), b"keep destination");
        assert_eq!(
            import(
                &installation,
                "import",
                &input,
                &output,
                &["--frontend", "studio", "--overwrite", "--allow-loss", LOSS]
            ),
            0
        );
        exact_pixels(&output);
        let alias = installation.root.join(format!("{name}.pcnd"));
        fs::hard_link(&input, &alias).unwrap();
        assert_ne!(
            import(
                &installation,
                "import",
                &input,
                &alias,
                &["--frontend", "studio", "--overwrite", "--allow-loss", LOSS]
            ),
            0
        );
        assert_eq!(fs::read(&alias).unwrap(), bytes);
        assert_eq!(fs::read(&input).unwrap(), bytes);
        let forbidden = installation.root.join("classic.pcnd");
        assert_ne!(
            import(
                &installation,
                "import",
                &input,
                &forbidden,
                &["--frontend", "classic-v11", "--allow-loss", LOSS]
            ),
            0
        );
        assert!(!forbidden.exists());
    }
}
#[test]
fn installed_mcp_jxr_receipts_defaults_consent_and_publication_guards() {
    let installation = Installation::new();
    let mut client = mcp_client::Client::at(&installation.cli);
    client.ok("create_canvas", json!({"width":12,"height":9}));
    for (name, bytes, raw_x, y, inferred) in [
        ("default.jxr", RAW, "00000000", "42c00000", true),
        ("density.jxr", DPI, "42c00000", "43100000", false),
    ] {
        let input = installation.fixture(name, bytes);
        let before = client.ok("inspect_document", json!({}));
        for allowed in [json!([]), json!(["flatten"])] {
            assert_eq!(
                client.call(
                    "import_image",
                    json!({"path":input,"discard_changes":true,"allow_losses":allowed})
                )["isError"],
                true
            );
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
        let result = client.ok(
            "import_image",
            json!({"path":input,"discard_changes":true,"allow_losses":[LOSS]}),
        );
        assert_eq!(result["width"], 4);
        assert_eq!(result["height"], 3);
        assert_eq!(result["dirty"], true);
        let conversion = &result["conversion"];
        assert_eq!(
            conversion["decoder_profile"],
            wire::NATIVE_JXR_IMPORT_VARIANT
        );
        assert_eq!(conversion["preserving_save"], false);
        assert_eq!(conversion["source_lineage_adopted"], false);
        assert_eq!(conversion["losses_acknowledged"], json!([LOSS]));
        let assessment = &conversion["source_assessment"];
        assert_eq!(assessment["resolution_x"]["raw_f32_bits"], raw_x);
        assert_eq!(assessment["resolution_x"]["effective_f32_bits"], "42c00000");
        assert_eq!(assessment["resolution_y"]["effective_f32_bits"], y);
        assert_eq!(assessment["resolution_x"]["inferred_96_dpi"], inferred);
        assert!(
            assessment["color_policy"]
                .as_str()
                .unwrap()
                .contains("srgb")
        );
        let renamed = installation.root.join(format!("renamed-{name}"));
        fs::rename(&input, &renamed).unwrap();
        let alias = installation.root.join(format!("{name}.pcnd"));
        fs::hard_link(&renamed, &alias).unwrap();
        assert_eq!(
            client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
            true
        );
        let output = installation.root.join(format!("converted-{name}.pcnd"));
        client.ok("save_project", json!({"path":output}));
        exact_pixels(&output);
        assert_eq!(
            client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(fs::read(&renamed).unwrap(), bytes);
        let stable = client.ok("inspect_document", json!({}));
        client.cancel_call(
            "import_image",
            json!({"path":renamed,"discard_changes":true,"allow_losses":[LOSS]}),
        );
        assert_eq!(client.ok("inspect_document", json!({})), stable);
    }
    fs::remove_file(&installation.helper).unwrap();
    let input = installation.fixture("missing-helper.jxr", RAW);
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":input,"discard_changes":true,"allow_losses":[LOSS]})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
}
