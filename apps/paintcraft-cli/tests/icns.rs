use paintcraft_formats::{icns, pcnd};
use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};
const SOURCE: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/icns-fixtures/modern.icns");
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
fn expected(x: u32, y: u32, seed: u32) -> [u8; 4] {
    [
        ((seed * 31 + x % 4 * 17) % 256) as u8,
        ((seed * 13 + y % 4 * 29) % 256) as u8,
        ((seed * 19 + (x + y) % 5) % 256) as u8,
        [0, 64, 128, 255][((x + y) % 4) as usize],
    ]
}
#[test]
fn selected_icns_import_needs_consent_preserves_source_and_exact_density_entry_pixels() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-cli-icns-import-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.icns");
    fs::write(&source, SOURCE).unwrap();
    let project = root.join("selected.pcnd");
    let input = source.to_str().unwrap();
    let dest = project.to_str().unwrap();
    let discovery = ok(run(&["inspect-icns", input, "--json"]));
    assert_eq!(discovery["assessment_only"], true);
    assert_eq!(discovery["entries"][6]["tag"], "ic11");
    assert_eq!(discovery["entries"][6]["logical_width"], 16);
    assert_eq!(discovery["entries"][6]["scale"], 2);
    assert_eq!(discovery["entries"][6]["width"], 32);
    assert!(discovery["source_sha256"].as_str().unwrap().len() == 64);
    assert!(
        !run(&["inspect-icns", input, "--frontend", "classic-v11"])
            .status
            .success()
    );
    let losses = icns::KNOWN_IMPORT_LOSSES.join(",");
    for extra in [
        vec![],
        vec!["--entry", "6"],
        vec!["--entry", "99", "--allow-loss", &losses],
        vec![
            "--entry",
            "6",
            "--allow-loss",
            &losses,
            "--frontend",
            "classic-v11",
        ],
    ] {
        let mut args = vec!["import", input, "-o", dest];
        args.extend(extra);
        assert!(!run(&args).status.success());
        assert!(!project.exists());
    }
    let receipt = ok(run(&[
        "import",
        input,
        "-o",
        dest,
        "--entry",
        "6",
        "--allow-loss",
        &losses,
        "--json",
    ]));
    assert_eq!(receipt["file"]["source_lineage_adopted"], false);
    assert!(
        receipt["file"]["losses_acknowledged"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "icon-density-not-retained-in-flat-document")
    );
    let doc = pcnd::read(&fs::read(&project).unwrap()).unwrap();
    assert_eq!(doc.canvas, (32, 32));
    let raster = &doc.layers[&doc.roots[0]].raster;
    for y in 0..32 {
        for x in 0..32 {
            assert_eq!(raster.get(x, y).0, expected(x as u32, y as u32, 7));
        }
    }
    let alias = root.join("source-alias.pcnd");
    fs::hard_link(&source, &alias).unwrap();
    assert!(
        !run(&[
            "import",
            input,
            "-o",
            alias.to_str().unwrap(),
            "--entry",
            "6",
            "--allow-loss",
            &losses,
            "--overwrite"
        ])
        .status
        .success()
    );
    let unqualified = root.join("unqualified.icns");
    fs::write(
        &unqualified,
        include_bytes!(
            "../../../crates/services/paintcraft-formats/tests/icns-fixtures/mixed-jp2.icns"
        ),
    )
    .unwrap();
    let denied = root.join("denied.pcnd");
    assert!(
        !run(&[
            "import",
            unqualified.to_str().unwrap(),
            "-o",
            denied.to_str().unwrap(),
            "--entry",
            "1",
            "--allow-loss",
            &losses
        ])
        .status
        .success()
    );
    assert!(!denied.exists());
    assert_eq!(fs::read(&source).unwrap(), SOURCE);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn typed_fresh_icns_export_has_independent_fitted_pixels_and_failures_preserve_files() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-cli-icns-export-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let project = root.join("source.pcnd");
    let input = project.to_str().unwrap();
    ok(run(&[
        "new",
        "--width",
        "2",
        "--height",
        "1",
        "--background",
        "#FF0000FF",
        "-o",
        input,
        "--json",
    ]));
    let original = fs::read(&project).unwrap();
    let output = root.join("set.icns");
    let dest = output.to_str().unwrap();
    fs::write(&output, b"original destination").unwrap();
    for options in [
        vec!["--type", "ic11"],
        vec!["--type", "icp4", "--allow-loss", "icon-resized"],
        vec!["--type", "is32", "--allow-loss", "icon-resized"],
        vec![
            "--type",
            "ic11",
            "--type",
            "ic11",
            "--allow-loss",
            "icon-resized",
        ],
        vec![
            "--type",
            "ic11",
            "--frontend",
            "classic-v11",
            "--allow-loss",
            "icon-resized",
        ],
    ] {
        let mut args = vec!["export-icns", input, "-o", dest, "--overwrite"];
        args.extend(options);
        assert!(!run(&args).status.success());
        assert_eq!(fs::read(&output).unwrap(), b"original destination");
    }
    let receipt = ok(run(&[
        "export-icns",
        input,
        "-o",
        dest,
        "--overwrite",
        "--type",
        "ic11",
        "--type",
        "ic12",
        "--resampling",
        "nearest",
        "--allow-loss",
        "icon-resized",
        "--json",
    ]));
    assert_eq!(
        receipt["file"]["losses_acknowledged"],
        serde_json::json!(["icon-resized"])
    );
    let bytes = fs::read(&output).unwrap();
    for (index, side, logical) in [(0, 32, 16), (1, 64, 32)] {
        let decoded = icns::decode_entry(&bytes, index, icns::PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(
            (decoded.entry.logical_width, decoded.entry.scale),
            (logical, 2)
        );
        for y in 0..side {
            for x in 0..side {
                let pixel = &decoded.rgba[((y * side + x) * 4) as usize..][..4];
                assert_eq!(
                    pixel,
                    if y >= side / 4 && y < side * 3 / 4 {
                        &[255, 0, 0, 255]
                    } else {
                        &[0; 4]
                    }
                );
            }
        }
    }
    let alias = root.join("source-alias.icns");
    fs::hard_link(&project, &alias).unwrap();
    assert!(
        !run(&[
            "export-icns",
            input,
            "-o",
            alias.to_str().unwrap(),
            "--overwrite",
            "--type",
            "ic11",
            "--allow-loss",
            "icon-resized"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&project).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
