//! Explicit V5 installed-helper qualification; build the optional helper first.
use super::*;
use serde_json::json;
const LOSS: &str = "avif-item-names-not-retained";
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../../crates/services/paintcraft-formats/tests/avif-fixtures/",
            $name
        ))
    };
}
fn installation() -> Installation {
    let installation = Installation::new();
    let manifest = json::parse(&fs::read(&installation.manifest).unwrap()).unwrap();
    assert!(
        matches!(
            manifest.get("profile").and_then(Value::as_str),
            Some(wire::AVIF_NATIVE_PROFILE) | Some(wire::AVIF_SDR_NATIVE_PROFILE)
        ),
        "Build an actual V5 or V6 AVIF helper first"
    );
    installation
}
fn pixels(path: &Path, expected: &[u8]) {
    let document = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(document.canvas, (16, 16));
    assert_eq!(document.roots.len(), 1);
    let raster = &document.layers_in_order().next().unwrap().raster;
    for (index, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            raster.get((index % 16) as i64, (index / 16) as i64).0,
            *pixel
        );
    }
}
#[test]
#[ignore = "requires the actual optional V5 AVIF helper alongside the CLI"]
fn installed_cli_avif_consent_and_independent_libaom_pixels() {
    let installation = installation();
    for (name, source, expected, named) in [
        (
            "opaque",
            fixture!("opaque.avif").as_slice(),
            fixture!("opaque.libaom.rgba").as_slice(),
            false,
        ),
        (
            "alpha",
            fixture!("alpha.avif").as_slice(),
            fixture!("alpha.libaom.rgba").as_slice(),
            false,
        ),
        (
            "named",
            fixture!("named-alpha.avif").as_slice(),
            fixture!("alpha.libaom.rgba").as_slice(),
            true,
        ),
    ] {
        let input = installation.fixture(&format!("{name}.avif"), source);
        let output = installation.root.join(format!("{name}.pcnd"));
        if named {
            for flags in [&[][..], &["--allow-loss", "unknown"][..]] {
                let (code, _) = heif::convert(&installation, &input, &output, flags);
                assert_ne!(code, 0);
                assert!(!output.exists());
            }
        }
        let flags = if named {
            &["--allow-loss", LOSS][..]
        } else {
            &[][..]
        };
        let (code, receipt) = heif::convert(&installation, &input, &output, flags);
        assert_eq!(code, 0, "{receipt}");
        assert_eq!(
            receipt["engine"]["decoder_profile"],
            wire::NATIVE_AVIF_IMPORT_VARIANT
        );
        assert_eq!(receipt["engine"]["discarded_metadata"], named);
        assert_eq!(
            receipt["engine"]["source_assessment"]["source_sha256"],
            hex(&Sha256::digest(source))
        );
        pixels(&output, expected);
        assert_eq!(fs::read(input).unwrap(), source);
    }
}
#[test]
#[ignore = "requires the actual optional V5 AVIF helper alongside the CLI"]
fn installed_mcp_avif_consent_cancel_and_renamed_source_guard() {
    let installation = installation();
    let mut client = mcp_client::Client::at(&installation.cli);
    client.ok("create_canvas", json!({"width":12,"height":9}));
    let input = installation.fixture("named.avif", fixture!("named-alpha.avif"));
    let before = client.ok("inspect_document", json!({}));
    for losses in [json!([]), json!(["unknown"])] {
        assert_eq!(
            client.call(
                "import_image",
                json!({"path":input,"discard_changes":true,"allow_losses":losses})
            )["isError"],
            true
        );
        assert_eq!(client.ok("inspect_document", json!({})), before);
    }
    let imported = client.ok(
        "import_image",
        json!({"path":input,"discard_changes":true,"allow_losses":[LOSS]}),
    );
    assert_eq!(
        imported["conversion"]["decoder_profile"],
        wire::NATIVE_AVIF_IMPORT_VARIANT
    );
    assert_eq!(imported["conversion"]["losses_acknowledged"], json!([LOSS]));
    let output = installation.root.join("painting.pcnd");
    client.ok("save_project", json!({"path":output}));
    pixels(&output, fixture!("alpha.libaom.rgba"));
    let renamed = installation.root.join("renamed.avif");
    fs::rename(&input, &renamed).unwrap();
    let alias = installation.root.join("source-alias.pcnd");
    fs::hard_link(&renamed, &alias).unwrap();
    let stable = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    client.cancel_call(
        "import_image",
        json!({"path":renamed,"discard_changes":true,"allow_losses":[LOSS]}),
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
    let manifest = fs::read(&installation.manifest).unwrap();
    let mut wrong: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    wrong["profile"] = json!(wire::NATIVE_PROFILE);
    wrong["qualified_import_variants"] = json!(wire::QUALIFIED_NATIVE_IMPORT_VARIANTS);
    fs::write(&installation.manifest, serde_json::to_vec(&wrong).unwrap()).unwrap();
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":renamed,"discard_changes":true,"allow_losses":[LOSS]})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
    fs::write(&installation.manifest, manifest).unwrap();
    fs::remove_file(&installation.helper).unwrap();
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":renamed,"discard_changes":true,"allow_losses":[LOSS]})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
    assert_eq!(fs::read(alias).unwrap(), fixture!("named-alpha.avif"));
}
