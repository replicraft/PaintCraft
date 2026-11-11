//! Real CLI/profile discovery and persisted full-RGBA output; no mock command runner.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("paintcraft-rotation-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn genuine_schema_and_atomic_file_application_roundtrip_exact_pixels() {
    let output = run(&["schema", "batch-rotation"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        schema["properties"]["schema"]["const"],
        "paintcraft.batch/rotation-v1"
    );
    let s = Scratch::new();
    let input = s.path("base.pcnd");
    let operations = s.path("ops.json");
    let destination = s.path("result.pcnd");
    assert!(
        run(&[
            "new",
            "--width",
            "2",
            "--height",
            "2",
            "--background",
            "#FFFFFFFF",
            "-o",
            &input
        ])
        .status
        .success()
    );
    let source = fs::read(&input).unwrap();
    let document = paintcraft_formats::pcnd::read(&source).unwrap();
    let id = document.roots[0];
    fs::write(&operations,format!(r##"{{"schema":"paintcraft.batch/rotation-v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{}","publication_epoch":"0"}},"required_filters":[],"required_layer_features":[],"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#11223300","mode":"replace"}},{{"op":"canvas.rotate","coefficients":[644245094,858993459]}}]}}"##,document.state_id)).unwrap();
    let out = run(&[
        "apply",
        &input,
        "--operations",
        &operations,
        "-o",
        &destination,
        "--json",
    ]);
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let d = paintcraft_formats::pcnd::read(&fs::read(&destination).unwrap()).unwrap();
    assert_eq!(d.canvas, (3, 3));
    let l = &d.layers[&id];
    // Frozen independent inverse-center image: hidden RGB remains straight RGBA.
    let expected = [
        [0, 0, 0, 0],
        [17, 34, 51, 0],
        [0, 0, 0, 0],
        [255; 4],
        [255; 4],
        [255; 4],
        [0, 0, 0, 0],
        [255; 4],
        [0, 0, 0, 0],
    ];
    for y in 0..3 {
        for x in 0..3 {
            assert_eq!(l.raster.get(x, y).0, expected[(y * 3 + x) as usize]);
        }
    }
    assert_eq!(fs::read(&input).unwrap(), source);
    let invalid = fs::read_to_string(&operations)
        .unwrap()
        .replace("paintcraft.batch/rotation-v1", "paintcraft.batch/v2");
    fs::write(&operations, invalid).unwrap();
    fs::write(&destination, b"unchanged").unwrap();
    assert!(
        !run(&[
            "apply",
            &input,
            "--operations",
            &operations,
            "-o",
            &destination,
            "--overwrite",
            "--json"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&destination).unwrap(), b"unchanged");
}
