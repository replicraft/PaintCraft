//! Real CLI profile parsing, discovery, pixel output and fail-closed file effects.
use paintcraft_formats::{image, pcnd};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    process::{Command, Output},
};
const RGBA: [u8; 16] = [
    12, 34, 56, 255, 78, 90, 123, 255, 222, 111, 7, 255, 19, 23, 29, 255,
];
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let dir =
            std::env::temp_dir().join(format!("paintcraft-retro-profile-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn actual_help_and_discovery_expose_only_real_initial_profile_operations() {
    let help = run(&["render", "--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("retro-win95"));
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(normalized.contains("modern convenience"));
    let value = success(run(&[
        "--json",
        "capabilities",
        "--frontend",
        "retro-win95",
    ]));
    assert_eq!(value["schema"], "paintcraft.build-capabilities/v1");
    assert_eq!(value["synthetic"], false);
    let rows: Vec<_> = value["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["profile"] == "frontend/retro-win95")
        .collect();
    let capabilities: BTreeSet<_> = rows
        .iter()
        .map(|row| row["capability"].as_str().unwrap())
        .collect();
    assert_eq!(
        capabilities,
        BTreeSet::from([
            "pcnd.rgba8.read",
            "png.rgba8.read",
            "bmp.rgba8.read",
            "png.rgba8.import",
            "bmp.rgba8.import",
            "png.rgba8.write",
            "bmp.rgba8.write",
            "pcnd.project.write"
        ])
    );
    for row in rows {
        assert_eq!(row["available"], true);
        assert_eq!(row["release_qualified"], false);
        assert_eq!(row["result_status"], "not-run");
    }
}

#[test]
fn actual_profile_project_to_raster_to_project_roundtrips_pixels_and_preserves_sources() {
    let folder = Folder::new();
    let project = folder.0.join("original.pcnd");
    let document = paintcraft_document::import_image(2, 2, &RGBA, "retro-cli-golden").unwrap();
    let original = pcnd::write_compact(&document).unwrap();
    fs::write(&project, &original).unwrap();
    for extension in ["png", "bmp"] {
        let raster = folder.0.join(format!("image.{extension}"));
        success(run(&[
            "--json",
            "render",
            project.to_str().unwrap(),
            "--frontend",
            "retro-win95",
            "-o",
            raster.to_str().unwrap(),
        ]));
        let encoded = fs::read(&raster).unwrap();
        let decoded = image::decode(&encoded).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(decoded.rgba, RGBA);
        let copy = folder.0.join(format!("from-{extension}.pcnd"));
        // The actual importer still requires each applicable conversion-loss consent.
        let allowed = image::KNOWN_IMPORT_LOSSES.join(",");
        success(run(&[
            "--json",
            "import",
            raster.to_str().unwrap(),
            "--frontend",
            "retro-win95",
            "--allow-loss",
            &allowed,
            "-o",
            copy.to_str().unwrap(),
        ]));
        let reopened = pcnd::read(&fs::read(copy).unwrap()).unwrap();
        assert_eq!(reopened.canvas, (2, 2));
        let layer = &reopened.layers[&reopened.roots[0]];
        for y in 0..2 {
            for x in 0..2 {
                let i = ((y * 2 + x) * 4) as usize;
                assert_eq!(layer.raster.get(x, y).0.as_slice(), &RGBA[i..i + 4]);
            }
        }
        assert_eq!(fs::read(&raster).unwrap(), encoded);
        assert_eq!(fs::read(&project).unwrap(), original);
    }
}

#[test]
fn forbidden_modes_refuse_before_codec_or_output_changes() {
    let folder = Folder::new();
    let missing = folder.0.join("never-acquired.pcnd");
    for extension in ["jpg", "gif", "tiff", "pcx", "msp", "jxl", "dds"] {
        let destination = folder.0.join(format!("existing.{extension}"));
        fs::write(&destination, b"existing output").unwrap();
        let result = run(&[
            "--json",
            "render",
            missing.to_str().unwrap(),
            "--frontend",
            "retro-win95",
            "--overwrite",
            "-o",
            destination.to_str().unwrap(),
        ]);
        assert_eq!(
            result.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("capability.unsupported"));
        assert!(text.contains("unsupported_frontend_format"));
        let value: Value = serde_json::from_str(&text).unwrap();
        assert!(value.get("schema").is_some());
        assert_eq!(fs::read(&destination).unwrap(), b"existing output");
    }
    let png = image::encode(image::ImageFormat::Png, 2, 2, &RGBA, 90).unwrap();
    for extension in ["jpg", "gif", "pcx", "msp", "jxl"] {
        let source = folder.0.join(format!("foreign.{extension}"));
        let output = folder.0.join(format!("denied-{extension}.pcnd"));
        fs::write(&source, &png).unwrap();
        let result = run(&[
            "--json",
            "import",
            source.to_str().unwrap(),
            "--frontend",
            "retro-win95",
            "-o",
            output.to_str().unwrap(),
        ]);
        assert_eq!(
            result.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("unsupported_frontend_format"));
        assert!(!output.exists());
        assert_eq!(fs::read(&source).unwrap(), png);
    }
}
