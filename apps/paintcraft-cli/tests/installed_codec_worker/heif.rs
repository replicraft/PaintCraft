use super::*;
use serde_json::json;
const LOSS: &str = "heif-color-reconstruction";
const NAMES: &str = "heif-item-names-not-retained";
const BOTH: &str = "heif-color-reconstruction,heif-item-names-not-retained";
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../../crates/services/paintcraft-formats/tests/heif-color-fixtures/",
            $name
        ))
    };
}
fn pixels(path: &Path, reference: &[u8], width: u32, height: u32, identity: bool) {
    let document = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(document.canvas, (width, height));
    assert_eq!(document.roots.len(), 1);
    let raster = &document.layers_in_order().next().unwrap().raster;
    assert_eq!(reference.len(), width as usize * height as usize * 3);
    for (i, pixel) in reference.as_chunks::<3>().0.iter().enumerate() {
        let x = i % width as usize;
        let y = i / width as usize;
        let actual = raster.get(x as i64, y as i64).0;
        for c in 0..3 {
            if identity {
                assert_eq!(actual[c], pixel[c]);
            } else {
                assert!(actual[c].abs_diff(pixel[c]) <= 1, "pixel{x},{y},channel{c}");
            }
        }
        assert_eq!(actual[3], [0, 1, 127, 128, 254, 255][(x + y) % 6]);
    }
}
pub(super) fn convert(
    installation: &Installation,
    input: &Path,
    output: &Path,
    flags: &[&str],
) -> (i32, serde_json::Value) {
    let result = Command::new(&installation.cli)
        .args(["--json", "import"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(["--frontend", "studio"])
        .args(flags)
        .output()
        .unwrap();
    let value = serde_json::from_slice(&result.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}: {} / {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )
    });
    (result.status.code().unwrap(), value)
}
#[test]
fn installed_cli_heif_identity_and_actual_conversion_receipts_match_reopened_independent_pixels() {
    let installation = Installation::new();
    for (name, source, reference, width, height, matrix, chroma, range) in [
        (
            "identity.heic",
            fixture!("identity444-alpha.heic").as_slice(),
            fixture!("identity444.rational-rgb").as_slice(),
            32,
            24,
            0,
            3,
            true,
        ),
        (
            "full.heif",
            fixture!("yuv420-full-alpha.heic").as_slice(),
            fixture!("yuv420-full.rational-rgb").as_slice(),
            32,
            24,
            1,
            1,
            true,
        ),
        (
            "limited.hif",
            fixture!("yuv444-limited-alpha.heic").as_slice(),
            fixture!("yuv444-limited.rational-rgb").as_slice(),
            32,
            24,
            1,
            3,
            false,
        ),
        (
            "odd.heic",
            fixture!("odd/yuv444-full-alpha.heic").as_slice(),
            fixture!("odd/yuv444-full.rational-rgb").as_slice(),
            33,
            25,
            1,
            3,
            true,
        ),
    ] {
        let input = installation.fixture(name, source);
        let output = installation.fixture("existing.pcnd", b"keep existing");
        let (code, _) = convert(&installation, &input, &output, &["--overwrite"]);
        assert_ne!(code, 0);
        assert_eq!(fs::read(&output).unwrap(), b"keep existing");
        let (code, _) = convert(
            &installation,
            &input,
            &output,
            &["--overwrite", "--allow-loss", "flatten"],
        );
        assert_ne!(code, 0);
        assert_eq!(fs::read(&output).unwrap(), b"keep existing");
        let flags = if matrix == 0 {
            vec!["--overwrite", "--allow-loss", NAMES]
        } else {
            vec!["--overwrite", "--allow-loss", BOTH]
        };
        let (code, receipt) = convert(&installation, &input, &output, &flags);
        assert_eq!(code, 0, "{receipt}");
        assert_eq!(
            receipt["engine"]["decoder_profile"],
            wire::NATIVE_HEIF_IMPORT_VARIANT
        );
        let assessment = &receipt["engine"]["source_assessment"];
        assert_eq!(assessment["source_sha256"], hex(&Sha256::digest(source)));
        assert_eq!(assessment["source_matrix"], matrix);
        assert_eq!(assessment["source_chroma_format"], chroma);
        assert_eq!(assessment["source_full_range"], range);
        assert_eq!(assessment["straight_alpha"], true);
        assert_eq!(assessment["preserving_save"], false);
        assert_eq!(
            assessment["losses"],
            if matrix == 0 {
                json!([NAMES])
            } else {
                json!([LOSS, NAMES])
            }
        );
        pixels(&output, reference, width, height, matrix == 0);
        let alias = installation.root.join(format!("{name}.pcnd"));
        fs::hard_link(&input, &alias).unwrap();
        assert_ne!(
            convert(
                &installation,
                &input,
                &alias,
                &["--overwrite", "--allow-loss", BOTH]
            )
            .0,
            0
        );
        assert_eq!(fs::read(&alias).unwrap(), source);
        assert_eq!(fs::read(&input).unwrap(), source);
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
fn installed_mcp_heif_consent_source_alias_cancel_and_helper_failures_preserve_session() {
    let installation = Installation::new();
    let mut client = mcp_client::Client::at(&installation.cli);
    client.ok("create_canvas", json!({"width":12,"height":9}));
    for (stem, source, reference, matrix) in [
        (
            "identity",
            fixture!("identity444-alpha.heic").as_slice(),
            fixture!("identity444.rational-rgb").as_slice(),
            0,
        ),
        (
            "color",
            fixture!("yuv420-limited-alpha.heic").as_slice(),
            fixture!("yuv420-limited.rational-rgb").as_slice(),
            1,
        ),
    ] {
        let input = installation.fixture(&format!("{stem}.heic"), source);
        let before = client.ok("inspect_document", json!({}));
        for allowed in [
            json!(["flatten"]),
            if matrix == 1 {
                json!([])
            } else {
                json!(["unknown"])
            },
        ] {
            assert_eq!(
                client.call(
                    "import_image",
                    json!({"path":input,"discard_changes":true,"allow_losses":allowed})
                )["isError"],
                true
            );
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
        let allowed = if matrix == 0 {
            json!([NAMES])
        } else {
            json!([LOSS, NAMES])
        };
        let imported = client.ok(
            "import_image",
            json!({"path":input,"discard_changes":true,"allow_losses":allowed}),
        );
        assert_eq!(imported["width"], 32);
        assert_eq!(imported["height"], 24);
        assert_eq!(imported["dirty"], true);
        assert_eq!(
            imported["conversion"]["decoder_profile"],
            wire::NATIVE_HEIF_IMPORT_VARIANT
        );
        assert_eq!(imported["conversion"]["preserving_save"], false);
        assert_eq!(
            imported["conversion"]["source_assessment"]["source_matrix"],
            matrix
        );
        assert_eq!(imported["conversion"]["losses_acknowledged"], allowed);
        let renamed = installation.root.join(format!("renamed-{stem}.heic"));
        fs::rename(&input, &renamed).unwrap();
        let alias = installation.root.join(format!("{stem}-alias.pcnd"));
        fs::hard_link(&renamed, &alias).unwrap();
        assert_eq!(
            client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
            true
        );
        let output = installation.root.join(format!("{stem}-converted.pcnd"));
        client.ok("save_project", json!({"path":output}));
        pixels(&output, reference, 32, 24, matrix == 0);
        assert_eq!(
            client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(fs::read(&renamed).unwrap(), source);
        let stable = client.ok("inspect_document", json!({}));
        client.cancel_call(
            "import_image",
            json!({"path":renamed,"discard_changes":true,"allow_losses":allowed}),
        );
        assert_eq!(client.ok("inspect_document", json!({})), stable);
    }
    let source = installation.fixture(
        "gray.heic",
        include_bytes!(
            "../../../../crates/services/paintcraft-formats/tests/heif-fixtures/primary.heic"
        ),
    );
    let stable = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":source,"discard_changes":true,"allow_losses":[LOSS]})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
    fs::remove_file(&installation.helper).unwrap();
    let source = installation.fixture("no-helper.heic", fixture!("identity444-alpha.heic"));
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":source,"discard_changes":true})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
}
#[test]
fn installed_cli_and_mcp_unnamed_identity_need_no_loss_consent_or_metadata_discard() {
    let installation = Installation::new();
    let source = fixture!("anonymous-identity444-alpha.heic");
    let input = installation.fixture("anonymous.heic", source);
    let output = installation.root.join("anonymous.pcnd");
    let (code, receipt) = convert(&installation, &input, &output, &[]);
    assert_eq!(code, 0, "{receipt}");
    assert_eq!(receipt["engine"]["discarded_metadata"], false);
    assert_eq!(receipt["engine"]["source_assessment"]["losses"], json!([]));
    pixels(&output, fixture!("identity444.rational-rgb"), 32, 24, true);
    let mut client = mcp_client::Client::at(&installation.cli);
    let imported = client.ok("import_image", json!({"path":input}));
    assert_eq!(imported["conversion"]["losses_acknowledged"], json!([]));
    assert_eq!(
        imported["conversion"]["source_assessment"]["losses"],
        json!([])
    );
    assert_eq!(imported["width"], 32);
    assert_eq!(imported["height"], 24);
    assert_eq!(imported["dirty"], true);
}
