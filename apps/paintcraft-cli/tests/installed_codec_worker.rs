//! Real copied CLI + helper + manifest installations. Normal image import stays
//! disabled; build both packages first (workspace tests already do this).
#![cfg(any(
    target_os = "macos",
    all(target_os = "linux", target_env = "gnu"),
    all(target_os = "windows", target_env = "msvc")
))]
use paintcraft_core::types::{
    json::{self, Value},
    policy::hex,
};
use paintcraft_formats::codec_worker as wire;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
struct Installation {
    root: PathBuf,
    cli: PathBuf,
    helper: PathBuf,
    manifest: PathBuf,
}
impl Installation {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "paintcraft-installed-dispatch-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let cli_source = Path::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
        let name = if cfg!(windows) {
            "paintcraft-codec-worker.exe"
        } else {
            "paintcraft-codec-worker"
        };
        let source = cli_source.parent().unwrap().join(name);
        assert!(
            source.is_file(),
            "Build the real helper first: cargo build -p paintcraft-codec-worker; full workspace tests build it automatically"
        );
        let cli = root.join(cli_source.file_name().unwrap());
        let helper = root.join(name);
        fs::copy(cli_source, &cli).unwrap();
        fs::copy(&source, &helper).unwrap();
        let manifest = root.join("paintcraft-codec-worker.json");
        let value = Value::object([
            ("schema_version", Value::Int(2)),
            ("embedded_build_record_verified", Value::Bool(true)),
            ("application_version", Value::str(env!("CARGO_PKG_VERSION"))),
            ("target", Value::str(target())),
            ("executable", Value::str(name)),
            (
                "sha256",
                Value::str(hex(&Sha256::digest(fs::read(&helper).unwrap()))),
            ),
            (
                "protocol_version",
                Value::Int(i64::from(wire::PROTOCOL_VERSION)),
            ),
            ("profile", Value::str(wire::NATIVE_PROFILE)),
            (
                "requested_heap_ceiling_bytes",
                Value::Int(wire::HEAP_BYTES as i64),
            ),
            (
                "qualified_import_variants",
                Value::Array(vec![Value::str(wire::NATIVE_JXL_IMPORT_VARIANT)]),
            ),
        ]);
        fs::write(&manifest, json::canonical(&value)).unwrap();
        Self {
            root,
            cli,
            helper,
            manifest,
        }
    }
    fn run(&self, source: &Path, codec: &str, extra: &[&str]) -> (i32, Value) {
        let output = Command::new(&self.cli)
            .arg("--json")
            .arg("codec-inspect")
            .arg(source)
            .args(["--codec", codec])
            .args(extra)
            .output()
            .unwrap();
        let value = json::parse(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "Invalid CLI JSON {e}: {} / {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        (output.status.code().unwrap(), value)
    }
    fn fixture(&self, name: &str, data: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, data).unwrap();
        path
    }
}
impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("release target required"),
    }
}
#[test]
fn installed_actual_helper_dispatches_independent_jxl_and_heif_fixtures_without_source_changes() {
    let installation = Installation::new();
    let jxl =
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl");
    let input = installation.fixture("source.jxl", jxl);
    let (exit, value) = installation.run(&input, "jxl", &[]);
    assert_eq!(exit, 0, "{value:?}");
    assert_eq!(
        value.get("model").and_then(Value::as_str),
        Some("srgb-rgba8")
    );
    assert_eq!(value.get("width").and_then(Value::as_i64), Some(3));
    assert_eq!(value.get("height").and_then(Value::as_i64), Some(2));
    let expected = [
        10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
        170, 180, 255,
    ];
    assert_eq!(
        value.get("samples_sha256").and_then(Value::as_str),
        Some(hex(&Sha256::digest(expected)).as_str())
    );
    assert_eq!(fs::read(&input).unwrap(), jxl);
    let heif = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/heif-fixtures/alpha.heic"
    );
    let input = installation.fixture("source.heic", heif);
    let (exit, value) = installation.run(&input, "heif", &[]);
    assert_eq!(exit, 0, "{value:?}");
    assert_eq!(
        value.get("model").and_then(Value::as_str),
        Some("native-gray8")
    );
    assert_eq!(
        value
            .get("encoding")
            .and_then(|v| v.get("transfer"))
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        value.get("reader_route").and_then(Value::as_str),
        Some("internal-qualification")
    );
    assert_eq!(fs::read(&input).unwrap(), heif);
    let jxr = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-4x3.jxr"
    );
    let input = installation.fixture("source.jxr", jxr);
    let (exit, value) = installation.run(&input, "jxr", &[]);
    assert_eq!(exit, 0, "{value:?}");
    assert_eq!(value.get("width").and_then(Value::as_i64), Some(4));
    assert_eq!(value.get("height").and_then(Value::as_i64), Some(3));
    let expected: Vec<u8> = (0..3)
        .flat_map(|y| {
            (0..4).flat_map(move |x| {
                [
                    (x * 47 + y * 13) % 256,
                    (x * 47 + y * 13 + 71) % 256,
                    (x * 47 + y * 13 + 142) % 256,
                    255,
                ]
                .map(|v| v as u8)
            })
        })
        .collect();
    assert_eq!(
        value.get("samples_sha256").and_then(Value::as_str),
        Some(hex(&Sha256::digest(expected)).as_str())
    );
    assert_eq!(
        value.get("source_color_policy").and_then(Value::as_str),
        Some("t832-uint-rgb-absent-context-default-srgb")
    );
    assert_eq!(
        value
            .get("source_resolution_f32_bits")
            .and_then(|v| v.get("x"))
            .and_then(Value::as_str),
        Some("00000000")
    );
    assert_eq!(
        value
            .get("source_resolution_f32_bits")
            .and_then(|v| v.get("y"))
            .and_then(Value::as_str),
        Some("00000000")
    );
    assert_eq!(fs::read(&input).unwrap(), jxr);
}
#[test]
fn missing_stale_or_changed_helper_and_source_guards_fail_closed() {
    let installation = Installation::new();
    let input = installation.fixture(
        "source.jxl",
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl"),
    );
    let (exit, _) = installation.run(&input, "jxl", &["--memory-limit", "1024"]);
    assert_eq!(exit, 5);
    let bad_hash = "0".repeat(64);
    let (exit, _) = installation.run(&input, "jxl", &["--expect-input-sha256", &bad_hash]);
    assert_eq!(exit, 4);
    let (exit, _) = installation.run(&input, "jxl", &["--pixels", "5"]);
    assert_eq!(exit, 5);
    let original = fs::read(&installation.manifest).unwrap();
    let stale = String::from_utf8(original.clone())
        .unwrap()
        .replace(env!("CARGO_PKG_VERSION"), "999.0.0");
    fs::write(&installation.manifest, stale).unwrap();
    let (exit, _) = installation.run(&input, "jxl", &[]);
    assert_eq!(exit, 3);
    fs::write(&installation.manifest, original).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&installation.helper)
        .unwrap()
        .write_all(b"changed helper bytes")
        .unwrap();
    let (exit, _) = installation.run(&input, "jxl", &[]);
    assert_eq!(exit, 3);
    fs::remove_file(&installation.helper).unwrap();
    let (exit, _) = installation.run(&input, "jxl", &[]);
    assert_eq!(exit, 3);
}
use std::io::Write;

fn import(
    installation: &Installation,
    command: &str,
    input: &Path,
    output: &Path,
    flags: &[&str],
) -> i32 {
    let reply = Command::new(&installation.cli)
        .args(["--json", command])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(flags)
        .output()
        .unwrap();
    assert!(
        !reply.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&reply.stderr)
    );
    reply.status.code().unwrap()
}
#[test]
fn installed_jxl_document_composition_preserves_exact_pixels_and_scoped_import_gate() {
    let installation = Installation::new();
    let bytes =
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl");
    let input = installation.fixture("painting.jxl", bytes);
    let output = installation.root.join("painting.pcnd");
    assert_eq!(
        import(&installation, "codec-import", &input, &output, &[]),
        0
    );
    let document = paintcraft_formats::pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(document.canvas, (3, 2));
    assert_eq!(document.roots.len(), 1);
    let raster = &document.layers_in_order().next().unwrap().raster;
    let expected = [
        [10, 20, 30, 0],
        [40, 50, 60, 1],
        [70, 80, 90, 127],
        [100, 110, 120, 128],
        [130, 140, 150, 254],
        [160, 170, 180, 255],
    ];
    for (index, pixel) in expected.into_iter().enumerate() {
        assert_eq!(raster.get((index % 3) as i64, (index / 3) as i64).0, pixel);
    }
    assert_eq!(fs::read(&input).unwrap(), bytes);
    let denied = installation.root.join("stock.pcnd");
    assert_eq!(
        import(
            &installation,
            "import",
            &input,
            &denied,
            &["--frontend", "studio"]
        ),
        0
    );
    assert!(denied.exists());
    fs::remove_file(&denied).unwrap();
    assert_eq!(
        import(
            &installation,
            "codec-import",
            &input,
            &denied,
            &["--frontend", "classic-v11"]
        ),
        3
    );
    assert!(!denied.exists());
}
#[test]
fn installed_composition_alias_codec_source_and_helper_failures_preserve_all_files() {
    let installation = Installation::new();
    let bytes =
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl");
    let input = installation.fixture("painting.jxl", bytes);
    assert_eq!(
        import(
            &installation,
            "codec-import",
            &input,
            &input,
            &["--overwrite"]
        ),
        2
    );
    let alias = installation.root.join("alias.pcnd");
    fs::hard_link(&input, &alias).unwrap();
    assert_eq!(
        import(
            &installation,
            "codec-import",
            &input,
            &alias,
            &["--overwrite"]
        ),
        2
    );
    assert_eq!(fs::read(&alias).unwrap(), bytes);
    let output = installation.fixture("existing.pcnd", b"keep existing destination");
    let bad = installation.fixture(
        "bad.jxl",
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba16.jxl"),
    );
    assert_eq!(
        import(
            &installation,
            "codec-import",
            &bad,
            &output,
            &["--overwrite"]
        ),
        3
    );
    assert_eq!(fs::read(&output).unwrap(), b"keep existing destination");
    let mut metadata = bytes.to_vec();
    metadata.extend([0, 0, 0, 12, b'E', b'x', b'i', b'f', 0, 0, 0, 0]);
    let metadata_source = installation.fixture("metadata.jxl", &metadata);
    assert_eq!(
        import(
            &installation,
            "codec-import",
            &metadata_source,
            &output,
            &["--overwrite"]
        ),
        3
    );
    assert_eq!(fs::read(&output).unwrap(), b"keep existing destination");
    let bad_hash = "0".repeat(64);
    assert_eq!(
        import(
            &installation,
            "codec-import",
            &input,
            &output,
            &["--overwrite", "--expect-input-sha256", &bad_hash]
        ),
        4
    );
    assert_eq!(fs::read(&output).unwrap(), b"keep existing destination");
    fs::remove_file(&installation.helper).unwrap();
    assert_eq!(
        import(
            &installation,
            "codec-import",
            &input,
            &output,
            &["--overwrite"]
        ),
        3
    );
    assert_eq!(fs::read(&output).unwrap(), b"keep existing destination");
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[path = "support/mcp_client.rs"]
mod mcp_client;
#[test]
fn installed_mcp_native_jxl_adoption_and_source_guards() {
    use serde_json::json;
    let installation = Installation::new();
    let bytes =
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl");
    let input = installation.fixture("source.jxl", bytes);
    let project = installation.root.join("converted.pcnd");
    let mut client = mcp_client::Client::at(&installation.cli);
    client.ok("create_canvas", json!({"width":12,"height":9}));
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call("import_image", json!({"path":input}))["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let imported = client.ok("import_image", json!({"path":input,"discard_changes":true}));
    assert_eq!(imported["width"], 3);
    assert_eq!(imported["height"], 2);
    assert_eq!(imported["dirty"], true);
    assert_eq!(
        imported["conversion"]["decoder_profile"],
        wire::NATIVE_JXL_IMPORT_VARIANT
    );
    let source_alias = installation.root.join("source-alias.pcnd");
    fs::hard_link(&input, &source_alias).unwrap();
    assert_eq!(
        client.call(
            "save_project",
            json!({"path":source_alias,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(
        client.call(
            "export_image",
            json!({"path":input,"overwrite":true,"allow_losses":["flatten"]})
        )["isError"],
        true
    );
    client.ok("save_project", json!({"path":project}));
    let document = paintcraft_formats::pcnd::read(&fs::read(&project).unwrap()).unwrap();
    assert_eq!(document.canvas, (3, 2));
    assert_eq!(
        document
            .layers_in_order()
            .next()
            .unwrap()
            .raster
            .get(0, 0)
            .0,
        [10, 20, 30, 0]
    );
    assert_eq!(fs::read(&input).unwrap(), bytes);
    let stable = client.ok("inspect_document", json!({}));
    client.cancel_call("import_image", json!({"path":input,"discard_changes":true}));
    assert_eq!(
        client.ok("inspect_document", json!({})),
        stable,
        "cancelled RPC must not publish a replacement"
    );

    let unsupported = installation.fixture(
        "high-depth.jxl",
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba16.jxl"),
    );
    assert_eq!(
        client.call(
            "import_image",
            json!({"path":unsupported,"discard_changes":true})
        )["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
    fs::remove_file(&installation.helper).unwrap();
    assert_eq!(
        client.call("import_image", json!({"path":input,"discard_changes":true}))["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), stable);
}
