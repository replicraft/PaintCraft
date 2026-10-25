use paintcraft_formats::{image, pcnd};
use std::{fs, process::Command};
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn exact_and_quantized_pcx_are_explicit_guarded_and_match_parallel_export() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-pcx-options-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let pixels: Vec<u8> = (0..400u32)
        .flat_map(|i| [i as u8, (i >> 8) as u8, i.wrapping_mul(53) as u8, 255])
        .collect();
    let doc = paintcraft_document::import_image(40, 10, &pixels, "pcx-cli-fixture").unwrap();
    let source = folder.join("source.pcnd");
    let bytes = pcnd::write(&doc).unwrap();
    fs::write(&source, &bytes).unwrap();
    let destination = folder.join("image.pcx");
    let source = source.to_str().unwrap();
    let path = destination.to_str().unwrap();
    fs::write(path, b"existing file").unwrap();
    let base = [
        "--json",
        "render",
        source,
        "-o",
        path,
        "--overwrite",
        "--pcx-encoding",
    ];
    for mode in ["indexed-exact", "indexed-256"] {
        let mut args = base.to_vec();
        args.push(mode);
        assert_eq!(run(&args).status.code(), Some(3));
        assert_eq!(fs::read(path).unwrap(), b"existing file");
    }
    let mut args = base.to_vec();
    args.extend(["indexed-256", "--allow-loss", "palette-quantized"]);
    let result = run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let quantized = fs::read(path).unwrap();
    assert_eq!(quantized[65], 1);
    let decoded = image::decode(&quantized).unwrap();
    assert_eq!((decoded.width, decoded.height), (40, 10));
    assert_ne!(decoded.rgba, pixels);
    args.extend(["--render-workers", "2"]);
    assert!(run(&args).status.success());
    assert_eq!(fs::read(path).unwrap(), quantized);
    let png = folder.join("invalid.png");
    assert_eq!(
        run(&[
            "render",
            source,
            "-o",
            png.to_str().unwrap(),
            "--pcx-encoding",
            "rgb24"
        ])
        .status
        .code(),
        Some(2)
    );
    assert!(!png.exists());
    assert_eq!(fs::read(source).unwrap(), bytes);
    fs::remove_dir_all(folder).unwrap();
}
