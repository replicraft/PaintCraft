use std::{fs, process::Command};
#[test]
fn msp_external_image_placement_requires_metadata_consent_and_guards_source_aliases() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-msp-placement-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.msp");
    let project = root.join("base.pcnd");
    let output = root.join("placed.pcnd");
    let bytes =
        include_bytes!("../../../crates/services/paintcraft-formats/tests/msp-fixtures/raw-9.msp");
    fs::write(&source, bytes).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        run(&[
            "new",
            "--width",
            "16",
            "--height",
            "12",
            "--background",
            "#00000000",
            "-o",
            project.to_str().unwrap()
        ])
        .status
        .success()
    );
    let args = [
        "paste-image",
        project.to_str().unwrap(),
        source.to_str().unwrap(),
        "--x",
        "2",
        "--y",
        "3",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    assert_eq!(run(&args).status.code(), Some(3));
    assert!(!output.exists());
    let mut accepted = args.to_vec();
    accepted.extend(["--allow-loss", "msp-metadata-ignored"]);
    let result = run(&accepted);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        receipt["file"]["losses_acknowledged"],
        serde_json::json!(["msp-metadata-ignored"])
    );
    let doc = paintcraft_formats::pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(doc.layers.len(), 2);
    let layer = &doc.layers[doc.roots.last().unwrap()];
    assert_eq!(layer.origin, (2, 3));
    assert_eq!(layer.raster.get(0, 0).0, [255; 4]);
    assert_eq!(layer.raster.get(1, 0).0, [0, 0, 0, 255]);
    let alias = root.join("alias.pcnd");
    fs::hard_link(&source, &alias).unwrap();
    accepted[8] = alias.to_str().unwrap();
    accepted.push("--overwrite");
    assert_eq!(run(&accepted).status.code(), Some(2));
    assert_eq!(fs::read(&source).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}
