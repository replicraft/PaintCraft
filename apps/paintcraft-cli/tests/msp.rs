use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};
const SOURCE: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/msp-fixtures/raw-9.msp");
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn ok(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn explicit_msp_import_requires_metadata_consent_and_fresh_export_never_converts_color_or_alpha() {
    let root = std::env::temp_dir().join(format!("paintcraft-cli-msp-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("legacy.msp");
    fs::write(&source, SOURCE).unwrap();
    let source = source.to_str().unwrap();
    for profile in ["studio", "classic-v7", "classic-v10", "classic-v11"] {
        let project = root.join(format!("{profile}.pcnd"));
        let dest = project.to_str().unwrap();
        let missing = run(&[
            "import",
            source,
            "-o",
            dest,
            "--frontend",
            profile,
            "--json",
        ]);
        assert_eq!(missing.status.code(), Some(3));
        assert!(!project.exists());
        let receipt = ok(run(&[
            "import",
            source,
            "-o",
            dest,
            "--frontend",
            profile,
            "--allow-loss",
            "msp-metadata-ignored",
            "--json",
        ]));
        assert_eq!(
            receipt["file"]["losses_acknowledged"],
            serde_json::json!(["msp-metadata-ignored"])
        );
        assert_eq!(receipt["file"]["source_lineage_adopted"], false);
        let doc = paintcraft_formats::pcnd::read(&fs::read(&project).unwrap()).unwrap();
        assert_eq!(doc.canvas, (9, 3));
        assert_eq!(doc.layers.len(), 1);
        let layer = &doc.layers[&doc.roots[0]];
        assert_eq!(layer.raster.get(0, 0).0, [255; 4]);
        assert_eq!(layer.raster.get(1, 0).0, [0, 0, 0, 255]);
        let copy = root.join(format!("{profile}.msp"));
        let exported = run(&[
            "render",
            dest,
            "-o",
            copy.to_str().unwrap(),
            "--frontend",
            profile,
            "--json",
        ]);
        if profile == "studio" {
            let result = ok(exported);
            assert_eq!(result["file"]["losses_acknowledged"], serde_json::json!([]));
            assert_eq!(
                paintcraft_formats::image::decode(&fs::read(copy).unwrap())
                    .unwrap()
                    .rgba,
                paintcraft_formats::image::decode(SOURCE).unwrap().rgba
            );
        } else {
            assert_eq!(exported.status.code(), Some(3));
            assert!(!copy.exists());
        }
    }
    let alias = root.join("source-alias.pcnd");
    fs::hard_link(source, &alias).unwrap();
    assert_eq!(
        run(&[
            "import",
            source,
            "-o",
            alias.to_str().unwrap(),
            "--overwrite",
            "--allow-loss",
            "msp-metadata-ignored"
        ])
        .status
        .code(),
        Some(2)
    );
    assert_eq!(fs::read(source).unwrap(), SOURCE);
    for color in ["#FF0000FF", "#808080FF", "#00000000"] {
        let project = root.join(format!("bad-{}.pcnd", &color[1..]));
        ok(run(&[
            "new",
            "--width",
            "2",
            "--height",
            "2",
            "--background",
            color,
            "-o",
            project.to_str().unwrap(),
            "--json",
        ]));
        let output = root.join(format!("bad-{}.msp", &color[1..]));
        assert_eq!(
            run(&[
                "render",
                project.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
                "--json"
            ])
            .status
            .code(),
            Some(3)
        );
        assert!(!output.exists());
    }
    fs::remove_dir_all(root).unwrap();
}
