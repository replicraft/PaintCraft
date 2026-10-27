//! Actual V6/V7 processes; independent libaom planes and Fraction RGB references.
use super::*;
use serde_json::json;
const COLOR: &str = "avif-color-reconstruction";
const NAMES: &str = "avif-item-names-not-retained";
const BOTH: &str = "avif-color-reconstruction,avif-item-names-not-retained";
fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/services/paintcraft-formats/tests/avif-sdr-fixtures")
            .join(name),
    )
    .unwrap()
}
fn installation() -> Installation {
    let installed = Installation::new();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&installed.manifest).unwrap()).unwrap();
    assert!(
        matches!(
            value["profile"].as_str(),
            Some(wire::AVIF_SDR_NATIVE_PROFILE)
                | Some(wire::FLOAT_NATIVE_PROFILE)
                | Some(wire::jxr_encode::qualified::NATIVE_PROFILE)
        ),
        "Build the actual V6/V7 SDR-capable helper; V4/V5 remain unsupported"
    );
    installed
}
fn pixels(path: &Path, expected: &[u8], size: (u32, u32)) {
    let doc = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(doc.canvas, size);
    assert_eq!(expected.len(), size.0 as usize * size.1 as usize * 4);
    let raster = &doc.layers_in_order().next().unwrap().raster;
    for (index, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            raster
                .get(
                    (index % size.0 as usize) as i64,
                    (index / size.0 as usize) as i64
                )
                .0,
            *pixel
        );
    }
}
#[test]
#[ignore = "requires an actual V6/V7 SDR-capable helper beside CLI"]
fn installed_v6_cli_sdr_profiles_consent_and_exact_fraction_pcnd() {
    let installed = installation();
    let mut cases = 0;
    for layout in ["444-fullchroma", "420-vertical", "420-colocated"] {
        for range in ["full", "limited"] {
            for (geometry, size) in [("even", (16, 16)), ("odd", (17, 13))] {
                for alpha in ["", "-alpha"] {
                    let name = format!("{layout}-{range}-{geometry}{alpha}");
                    let bytes = fixture(&format!("{name}.avif"));
                    let input = installed.fixture(&format!("{name}.avif"), &bytes);
                    let output = installed.root.join(format!("{name}.pcnd"));
                    let (code, receipt) =
                        heif::convert(&installed, &input, &output, &["--allow-loss", COLOR]);
                    assert_eq!(code, 0, "{receipt}");
                    assert_eq!(
                        receipt["engine"]["decoder_profile"],
                        wire::NATIVE_AVIF_SDR_IMPORT_VARIANT
                    );
                    let assessment = &receipt["engine"]["source_assessment"];
                    assert_eq!(assessment["source_sha256"], hex(&Sha256::digest(&bytes)));
                    assert_eq!(assessment["primary"]["matrix"], 1);
                    assert_eq!(assessment["primary"]["full_range"], range == "full");
                    assert_eq!(assessment["losses"], json!([COLOR]));
                    pixels(&output, &fixture(&format!("{name}.rational.rgba")), size);
                    assert_eq!(fs::read(input).unwrap(), bytes);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 24);
    let input = installed.fixture("named.avif", &fixture("named-both.avif"));
    let output = installed.root.join("named.pcnd");
    for flags in [
        &[][..],
        &["--allow-loss", "unknown"][..],
        &["--allow-loss", COLOR][..],
        &["--allow-loss", NAMES][..],
    ] {
        assert_ne!(heif::convert(&installed, &input, &output, flags).0, 0);
        assert!(!output.exists());
    }
    let (code, receipt) = heif::convert(&installed, &input, &output, &["--allow-loss", BOTH]);
    assert_eq!(code, 0, "{receipt}");
    assert_eq!(
        receipt["engine"]["source_assessment"]["primary"]["item_name"],
        "Color image"
    );
    assert_eq!(
        receipt["engine"]["source_assessment"]["alpha"]["item_name"],
        "Alpha mask"
    );
    pixels(
        &output,
        &fixture("420-vertical-limited-odd-alpha.rational.rgba"),
        (17, 13),
    );
    let unknown = installed.fixture("unknown.avif", &fixture("420-unknown-full-odd-alpha.avif"));
    assert_ne!(
        heif::convert(
            &installed,
            &unknown,
            &installed.root.join("unknown.pcnd"),
            &["--allow-loss", COLOR]
        )
        .0,
        0
    );
    let denied = Command::new(&installed.cli)
        .args(["--json", "import"])
        .arg(&input)
        .arg("--output")
        .arg(installed.root.join("classic.pcnd"))
        .args(["--frontend", "classic-v11", "--allow-loss", BOTH])
        .output()
        .unwrap();
    assert!(!denied.status.success());
    assert!(!installed.root.join("classic.pcnd").exists());
}
#[test]
#[ignore = "requires an actual V6/V7 SDR-capable helper beside CLI"]
fn installed_v6_mcp_sdr_consent_cancel_source_owner_and_exact_pcnd() {
    let installed = installation();
    let mut client = mcp_client::Client::at(&installed.cli);
    client.ok("create_canvas", json!({"width":12,"height":9}));
    let input = installed.fixture("named.avif", &fixture("named-both.avif"));
    let before = client.ok("inspect_document", json!({}));
    for losses in [
        json!([]),
        json!(["unknown"]),
        json!([COLOR]),
        json!([NAMES]),
    ] {
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
        json!({"path":input,"discard_changes":true,"allow_losses":[COLOR,NAMES]}),
    );
    assert_eq!(
        imported["conversion"]["decoder_profile"],
        wire::NATIVE_AVIF_SDR_IMPORT_VARIANT
    );
    assert_eq!(
        imported["conversion"]["source_assessment"]["losses"],
        json!([COLOR, NAMES])
    );
    let output = installed.root.join("converted.pcnd");
    client.ok("save_project", json!({"path":output}));
    pixels(
        &output,
        &fixture("420-vertical-limited-odd-alpha.rational.rgba"),
        (17, 13),
    );
    let renamed = installed.root.join("renamed.avif");
    fs::rename(&input, &renamed).unwrap();
    let alias = installed.root.join("source-alias.pcnd");
    fs::hard_link(&renamed, &alias).unwrap();
    let stable = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
        true
    );
    client.cancel_call(
        "import_image",
        json!({"path":renamed,"discard_changes":true,"allow_losses":[COLOR,NAMES]}),
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
    let manifest = fs::read(&installed.manifest).unwrap();
    for (profile, variants) in [
        (wire::NATIVE_PROFILE, wire::QUALIFIED_NATIVE_IMPORT_VARIANTS),
        (
            wire::AVIF_NATIVE_PROFILE,
            wire::QUALIFIED_AVIF_IMPORT_VARIANTS,
        ),
    ] {
        let mut wrong: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        wrong["profile"] = json!(profile);
        wrong["qualified_import_variants"] = json!(variants);
        fs::write(&installed.manifest, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert_eq!(
            client.call(
                "import_image",
                json!({"path":renamed,"discard_changes":true,"allow_losses":[COLOR,NAMES]})
            )["isError"],
            true,
            "SDR must refuse {profile}"
        );
        assert_eq!(client.ok("inspect_document", json!({})), stable);
    }
    fs::write(&installed.manifest, manifest).unwrap();
    fs::remove_file(&installed.helper).unwrap();
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":renamed,"discard_changes":true,"allow_losses":[COLOR,NAMES]})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
    assert_eq!(fs::read(alias).unwrap(), fixture("named-both.avif"));
}
