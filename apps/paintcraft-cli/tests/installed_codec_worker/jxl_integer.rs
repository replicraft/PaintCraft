//! Real copied CLI/stdio MCP consumers; explicitly selected immutable helpers.
use super::*;
use serde_json::{Value as Json, json};
const LOSS: &str = "depth-16-reduced-to-8";
const EIGHT: &[u8] =
    include_bytes!("../../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl");
const SIXTEEN: &[u8] =
    include_bytes!("../../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba16.jxl");
const PIXELS8: &[u8] = &[
    10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
    170, 180, 255,
];
const PIXELS16: &[u8] = &[
    0, 0, 255, 1, 1, 127, 128, 0, 255, 4, 8, 128, 16, 32, 64, 255,
];
fn execute(app: &Installation, source: &Path, output: &Path, losses: &[&str]) -> (bool, Json) {
    let mut command = Command::new(&app.cli);
    command
        .args(["--json", "import"])
        .arg(source)
        .args(["--frontend", "studio", "--overwrite", "--output"])
        .arg(output);
    for loss in losses {
        command.args(["--allow-loss", loss]);
    }
    let result = command.output().unwrap();
    let value = serde_json::from_slice(&result.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}:{} / {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )
    });
    (result.status.success(), value)
}
fn pixels(path: &Path, expected: &[u8]) {
    let doc = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        doc.canvas.0 as usize * doc.canvas.1 as usize * 4,
        expected.len()
    );
    let raster = &doc.layers_in_order().next().unwrap().raster;
    for (index, color) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            raster
                .get(
                    (index % doc.canvas.0 as usize) as i64,
                    (index / doc.canvas.0 as usize) as i64
                )
                .0,
            *color
        );
    }
}
fn assessment(value: &Json, source: &[u8], rgba: &[u8], depth: u8) {
    assert_eq!(value["policy"], wire::jxl_integer::POLICY);
    assert_eq!(value["source_depth"], depth);
    assert_eq!(value["output_depth"], 8);
    assert_eq!(value["source_sha256"], hex(&Sha256::digest(source)));
    assert_eq!(value["output_sha256"], hex(&Sha256::digest(rgba)));
    let report = wire::jxl_integer::Report {
        source_depth: depth,
        width: if depth == 16 { 2 } else { 3 },
        height: 2,
        source_sha256: Sha256::digest(source).into(),
        output_sha256: Sha256::digest(rgba).into(),
        changed_rgb_components: if depth == 16 { 8 } else { 0 },
        changed_alpha: if depth == 16 { 2 } else { 0 },
    };
    assert_eq!(
        value["changed_rgb_components"],
        report.changed_rgb_components
    );
    assert_eq!(value["changed_alpha"], report.changed_alpha);
    assert_eq!(
        value["assessment_sha256"],
        hex(&Sha256::digest(report.encode().unwrap()))
    );
}

#[test]
#[ignore = "requires an exact PAINTCRAFT_JXL_INTEGER_HELPER or both immutable V9/V10 helpers"]
fn actual_cli_and_mcp_integer_jxl_consent_pixels_receipts_and_guards() {
    let helpers = if let Some(helper) = std::env::var_os("PAINTCRAFT_JXL_INTEGER_HELPER") {
        vec![PathBuf::from(helper)]
    } else {
        ["PAINTCRAFT_V9_HELPER", "PAINTCRAFT_V10_HELPER"]
            .map(|key| PathBuf::from(std::env::var_os(key).expect("qualified helper")))
            .to_vec()
    };
    for helper in helpers {
        let bytes = fs::read(&helper).unwrap();
        let (profile, _) =
            codec_contract::helper_contract(&bytes, env!("CARGO_PKG_VERSION")).unwrap();
        assert!(matches!(
            profile,
            wire::jxl_integer::contracts::base::NATIVE_PROFILE
                | wire::jxl_integer::contracts::writer::NATIVE_PROFILE
        ));
        let app = if let Some(directory) = std::env::var_os("PAINTCRAFT_QUALIFIED_INSTALLATION") {
            let installed = Installation::from_installed(&PathBuf::from(directory));
            assert_eq!(
                installed.helper.canonicalize().unwrap(),
                helper.canonicalize().unwrap()
            );
            installed
        } else {
            Installation::with_helper(Some(&helper))
        };
        let mut client = mcp_client::Client::at(&app.cli);
        client.ok("create_canvas", json!({"width":7,"height":5}));
        for (name, source, expected, depth) in [
            ("eight.jxl", EIGHT, PIXELS8, 8),
            ("sixteen.jxl", SIXTEEN, PIXELS16, 16),
        ] {
            let input = app.fixture(name, source);
            let output = app.fixture("copy.pcnd", b"existing destination");
            let prior = client.ok("inspect_document", json!({}));
            if depth == 16 {
                for losses in [&[][..], &["unknown"][..], &["flatten"][..]] {
                    assert!(!execute(&app, &input, &output, losses).0);
                    assert_eq!(fs::read(&output).unwrap(), b"existing destination");
                    assert_eq!(
                        client.call(
                            "import_image",
                            json!({"path":input,"discard_changes":true,"allow_losses":losses})
                        )["isError"],
                        true
                    );
                    assert_eq!(client.ok("inspect_document", json!({})), prior);
                }
            }
            let losses: &[&str] = if depth == 16 { &[LOSS] } else { &[] };
            let (ok, receipt) = execute(&app, &input, &output, losses);
            assert!(ok, "{receipt}");
            pixels(&output, expected);
            assessment(
                &receipt["engine"]["source_assessment"],
                source,
                expected,
                depth,
            );
            let imported = client.ok(
                "import_image",
                json!({"path":input,"discard_changes":true,"allow_losses":losses}),
            );
            assessment(
                &imported["conversion"]["source_assessment"],
                source,
                expected,
                depth,
            );
            assert_eq!(imported["conversion"]["losses_acknowledged"], json!(losses));
            assert_eq!(imported["dirty"], true);
            let saved = app.root.join(format!("{name}.pcnd"));
            client.ok("save_project", json!({"path":saved}));
            pixels(&saved, expected);
            let alias = app.root.join(format!("{name}-source-alias.pcnd"));
            fs::hard_link(&input, &alias).unwrap();
            assert!(!execute(&app, &input, &alias, losses).0);
            assert_eq!(fs::read(&alias).unwrap(), source);
            let before = client.ok("inspect_document", json!({}));
            assert_eq!(
                client.call("save_project", json!({"path":alias,"overwrite":true}))["isError"],
                true
            );
            assert_eq!(client.ok("inspect_document", json!({})), before);
            assert_eq!(fs::read(&input).unwrap(), source);
        }
        for (name, bytes) in [
            (
                "icc.jxl",
                include_bytes!(
                    "../../../../crates/services/paintcraft-formats/tests/jxl-fixtures/icc.jxl"
                )
                .as_slice(),
            ),
            (
                "mixed.jxl",
                include_bytes!(
                    "../../../../crates/services/paintcraft-formats/tests/jxl-fixtures/alpha-profiles/color8-alpha16.jxl"
                ),
            ),
        ] {
            let source = app.fixture(name, bytes);
            let output = app.fixture("refused.pcnd", b"unchanged");
            assert!(!execute(&app, &source, &output, &[LOSS]).0);
            assert_eq!(fs::read(output).unwrap(), b"unchanged");
            let before = client.ok("inspect_document", json!({}));
            assert_eq!(
                client.call(
                    "import_image",
                    json!({"path":source,"discard_changes":true,"allow_losses":[LOSS]})
                )["isError"],
                true
            );
            assert_eq!(client.ok("inspect_document", json!({})), before);
        }
    }
}

#[test]
#[ignore = "requires immutable PAINTCRAFT_V7_HELPER and PAINTCRAFT_V8_HELPER"]
fn actual_old_helpers_keep_exact8_and_refuse16_without_fallback() {
    for key in ["PAINTCRAFT_V7_HELPER", "PAINTCRAFT_V8_HELPER"] {
        let helper = PathBuf::from(std::env::var_os(key).expect("qualified old helper"));
        let app = Installation::with_helper(Some(&helper));
        let mut client = mcp_client::Client::at(&app.cli);
        let input = app.fixture("eight.jxl", EIGHT);
        let output = app.root.join("eight.pcnd");
        let (ok, value) = execute(&app, &input, &output, &[]);
        assert!(ok, "{value}");
        pixels(&output, PIXELS8);
        assert!(value["engine"]["source_assessment"].is_null());
        client.ok("import_image", json!({"path":input}));
        let before = client.ok("inspect_document", json!({}));
        let input = app.fixture("sixteen.jxl", SIXTEEN);
        let output = app.fixture("refused.pcnd", b"unchanged");
        assert!(!execute(&app, &input, &output, &[LOSS]).0);
        assert_eq!(fs::read(output).unwrap(), b"unchanged");
        assert_eq!(
            client.call(
                "import_image",
                json!({"path":input,"discard_changes":true,"allow_losses":[LOSS]})
            )["isError"],
            true
        );
        assert_eq!(client.ok("inspect_document", json!({})), before);
    }
}
