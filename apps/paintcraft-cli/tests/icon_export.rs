use paintcraft_formats::{icons, pcnd};
use std::{fs, process::Command};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn exports_explicit_sizes_encodings_and_hotspots_through_guarded_save() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-cli-icon-export-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let source = folder.join("source.pcnd");
    let doc =
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], "cli-icon")
            .unwrap();
    let source_bytes = pcnd::write_compact(&doc).unwrap();
    fs::write(&source, &source_bytes).unwrap();
    let input = source.to_str().unwrap();
    for (suffix, first, second) in [
        ("ico", "16:png", "32:bitmap"),
        ("cur", "16:png:3:7", "32:bitmap:9:11"),
    ] {
        let path = folder.join(format!("image.{suffix}"));
        let dest = path.to_str().unwrap();
        fs::write(&path, b"existing destination").unwrap();
        let base = [
            "export-icons",
            input,
            "-o",
            dest,
            "--overwrite",
            "--entry",
            first,
            "--entry",
            second,
            "--resampling",
            "nearest",
        ];
        let refused = run(&base);
        assert_eq!(refused.status.code(), Some(3));
        assert!(String::from_utf8_lossy(&refused.stderr).contains("icon-resized"));
        assert_eq!(fs::read(&path).unwrap(), b"existing destination");
        let mut args = base.to_vec();
        args.extend(["--allow-loss", "icon-resized", "--json"]);
        let output = run(&args);
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(report.to_string().contains("icon-resized"));
        let bytes = fs::read(&path).unwrap();
        for (index, (size, encoding)) in [(16, icons::Encoding::Png), (32, icons::Encoding::Bitmap)]
            .into_iter()
            .enumerate()
        {
            let decoded = icons::decode_entry(&bytes, index as u16, 65536).unwrap();
            assert_eq!((decoded.entry.width, decoded.entry.height), (size, size));
            assert_eq!(decoded.entry.encoding, encoding);
            assert_eq!(
                decoded.entry.hotspot,
                if suffix == "cur" {
                    Some(if index == 0 { (3, 7) } else { (9, 11) })
                } else {
                    None
                }
            );
            let pixel = |x, y| &decoded.rgba[((y * size + x) * 4) as usize..][..4];
            assert_eq!(pixel(0, 0), [0; 4]);
            assert_eq!(pixel(0, size / 4), [255, 0, 0, 255]);
            assert_eq!(pixel(size - 1, size / 4), [0, 0, 255, 255]);
            assert_eq!(pixel(0, size - 1), [0; 4]);
        }
        for extra in [
            vec!["--frontend", "classic-v11"],
            vec!["--expect-input-sha256", &"0".repeat(64)],
        ] {
            let mut rejected = args.clone();
            rejected.extend(extra);
            assert!(!run(&rejected).status.success());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        fs::remove_file(&path).unwrap();
        fs::hard_link(&source, &path).unwrap();
        assert_eq!(run(&args).status.code(), Some(2));
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        fs::remove_file(&path).unwrap();
    }
    for entry in [
        "0:png",
        "257:png",
        "16:jpeg",
        "16:png:16:0",
        "16x:bitmap",
        "16:png:1",
    ] {
        assert_eq!(
            run(&[
                "export-icons",
                input,
                "-o",
                folder.join("bad.ico").to_str().unwrap(),
                "--entry",
                entry
            ])
            .status
            .code(),
            Some(2)
        );
    }
    assert_eq!(fs::read(source).unwrap(), source_bytes);
    fs::remove_dir_all(folder).unwrap();
}
