use paintcraft_formats::pcnd;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn set<'a>(args: &mut [&'a str], flag: &str, value: &'a str) {
    let index = args.iter().position(|arg| *arg == flag).unwrap() + 1;
    args[index] = value;
}
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
#[test]
fn compressed_svg_uses_the_explicit_fit_and_rejects_conflicting_selectors() {
    let dir = std::env::temp_dir().join(format!("paintcraft-svgz-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("scene.svgz");
    let output = dir.join("scene.pcnd");
    let bytes = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/svg-fixtures/aligned.svgz"
    );
    fs::write(&source, bytes).unwrap();
    let args = [
        "import",
        source.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--svg-width",
        "64",
        "--svg-height",
        "64",
        "--svg-fit",
        "stretch",
        "--allow-loss",
        "svg-rasterized",
    ];
    let result = run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.get(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(layer.raster.get(63, 63).0, [0, 0, 255, 128]);
    let saved = fs::read(&output).unwrap();
    for selector in ["--page", "--entry"] {
        let mut refused = args.to_vec();
        refused.extend([selector, "0", "--overwrite"]);
        assert_eq!(run(&refused).status.code(), Some(2));
        assert_eq!(fs::read(&output).unwrap(), saved);
    }
    assert_eq!(fs::read(&source).unwrap(), bytes);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn svg_requires_explicit_dimensions_and_consent_and_preserves_source_and_profile() {
    let dir = std::env::temp_dir().join(format!("paintcraft-svg-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("scene.svg");
    let output = dir.join("scene.pcnd");
    let bytes =
        fs::read(root().join("crates/services/paintcraft-formats/tests/svg-fixtures/aligned.svg"))
            .unwrap();
    fs::write(&source, &bytes).unwrap();
    let a = source.to_str().unwrap();
    let b = output.to_str().unwrap();
    for args in [
        vec!["import", a, "-o", b],
        vec!["import", a, "-o", b, "--svg-width", "64"],
        vec![
            "import",
            a,
            "-o",
            b,
            "--svg-width",
            "64",
            "--svg-height",
            "64",
        ],
        vec![
            "import",
            a,
            "-o",
            b,
            "--svg-width",
            "64",
            "--svg-height",
            "64",
            "--allow-loss",
            "made-up",
        ],
    ] {
        assert!(!run(&args).status.success());
        assert!(!output.exists());
    }
    let args = [
        "import",
        a,
        "-o",
        b,
        "--svg-width",
        "64",
        "--svg-height",
        "64",
        "--allow-loss",
        "svg-rasterized",
    ];
    let absent = dir.join("absent.svg");
    let mut denied = args.to_vec();
    denied[1] = absent.to_str().unwrap();
    denied.extend(["--frontend", "classic-v7"]);
    assert_eq!(
        run(&denied).status.code(),
        Some(3),
        "frontend policy must win before missing-file acquisition"
    );
    assert_eq!(
        run(&["import", absent.to_str().unwrap(), "-o", b])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run(&[
            "import",
            absent.to_str().unwrap(),
            "-o",
            b,
            "--frontend",
            "classic-v7"
        ])
        .status
        .code(),
        Some(3)
    );
    let result = run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(doc.canvas, (64, 64));
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.get(0, 0).0, [0; 4]);
    assert_eq!(layer.raster.get(0, 16).0, [255, 0, 0, 255]);
    assert_eq!(layer.raster.get(63, 47).0, [0, 0, 255, 128]);
    for classic in ["classic-v7", "classic-v10", "classic-v11"] {
        let mut refused = args.to_vec();
        refused.extend(["--overwrite", "--frontend", classic]);
        assert!(!run(&refused).status.success());
        assert!(
            pcnd::read(&fs::read(&output).unwrap())
                .unwrap()
                .same_semantics(&doc)
        );
    }
    let mut alias = args;
    set(&mut alias, "-o", a);
    let mut alias = alias.to_vec();
    alias.push("--overwrite");
    assert!(!run(&alias).status.success());
    assert_eq!(fs::read(&source).unwrap(), bytes);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn text_uses_shared_fonts_and_font_inputs_cannot_be_conversion_destinations() {
    let dir = std::env::temp_dir().join(format!("paintcraft-svg-font-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("text.svg");
    let output = dir.join("text.pcnd");
    let font = dir.join("font.ttf");
    fs::copy(root().join("assets/fonts/selawik/selawk.ttf"), &font).unwrap();
    let original = fs::read(&font).unwrap();
    fs::write(&source,"<svg xmlns='http://www.w3.org/2000/svg' width='64' height='32'><text x='0' y='24' font-family='Selawik' font-size='22'>Paint</text></svg>").unwrap();
    let a = source.to_str().unwrap();
    let b = output.to_str().unwrap();
    let f = font.to_str().unwrap();
    let args = [
        "import",
        a,
        "-o",
        b,
        "--svg-width",
        "64",
        "--svg-height",
        "32",
        "--allow-loss",
        "svg-rasterized",
    ];
    let result = run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    assert!((0..32).any(|y| (0..64).any(|x| layer.raster.get(x, y).0[3] > 0)));
    let alias = dir.join("font-alias.pcnd");
    fs::hard_link(&font, &alias).unwrap();
    let mut protected = args.to_vec();
    set(&mut protected, "-o", alias.to_str().unwrap());
    protected.extend(["--svg-font", f, "--overwrite"]);
    assert!(!run(&protected).status.success());
    assert_eq!(fs::read(&font).unwrap(), original);
    fs::write(&source,"<svg xmlns='http://www.w3.org/2000/svg'><text y='24' font-family='Missing font'>Paint</text></svg>").unwrap();
    let mut fallback = args.to_vec();
    fallback.push("--overwrite");
    assert!(!run(&fallback).status.success());
    assert!(
        pcnd::read(&fs::read(&output).unwrap())
            .unwrap()
            .same_semantics(&doc)
    );
    set(
        &mut fallback,
        "--allow-loss",
        "svg-rasterized,svg-font-substituted",
    );
    assert!(run(&fallback).status.success());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn svg_dpi_changes_pixel_coverage_and_invalid_parameters_preserve_the_destination() {
    let dir = std::env::temp_dir().join(format!("paintcraft-svg-dpi-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("units.svg");
    let output = dir.join("units.pcnd");
    fs::write(&source,"<svg xmlns='http://www.w3.org/2000/svg' width='1in' height='0.5in'><rect width='1' height='1'/></svg>").unwrap();
    let a = source.to_str().unwrap();
    let b = output.to_str().unwrap();
    let args = [
        "import",
        a,
        "-o",
        b,
        "--svg-width",
        "96",
        "--svg-height",
        "48",
        "--svg-dpi",
        "96",
        "--allow-loss",
        "svg-rasterized",
        "--overwrite",
    ];
    assert!(run(&args).status.success());
    let first = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        first.layers[&first.roots[0]].raster.get(0, 0).0,
        [0, 0, 0, 255]
    );
    let mut changed = args;
    set(&mut changed, "--svg-dpi", "192");
    assert!(run(&changed).status.success());
    let second = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_ne!(first.state_id, second.state_id);
    assert_eq!(
        second.layers[&second.roots[0]].raster.get(0, 0).0,
        [0, 0, 0, 64]
    );
    let saved = fs::read(&output).unwrap();
    for invalid in ["0", "NaN", "9999"] {
        set(&mut changed, "--svg-dpi", invalid);
        assert!(!run(&changed).status.success());
        assert_eq!(fs::read(&output).unwrap(), saved);
    }
    fs::remove_dir_all(dir).unwrap();
}
