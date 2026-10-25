use paintcraft_formats::{icons, pcnd};
use std::{fs, process::Command};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn explicit_entry_requires_exact_consent_and_reopens_selected_pixels() {
    let folder = std::env::temp_dir().join(format!("paintcraft-cli-icons-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/icon-fixtures");
    for suffix in ["ico", "cur"] {
        let bytes = fs::read(fixtures.join(format!("mixed.{suffix}"))).unwrap();
        let source = folder.join(format!("mixed.{suffix}"));
        let output = folder.join(format!("{suffix}.pcnd"));
        fs::write(&source, &bytes).unwrap();
        let input = source.to_str().unwrap();
        let dest = output.to_str().unwrap();
        let losses = icons::KNOWN_IMPORT_LOSSES.join(",");
        for extra in [
            vec![],
            vec!["--entry", "1"],
            vec!["--entry", "8"],
            vec!["--entry", "1", "--page", "0"],
            vec![
                "--entry",
                "1",
                "--frontend",
                "classic-v11",
                "--allow-loss",
                &losses,
            ],
        ] {
            let mut args = vec!["import", input, "-o", dest];
            args.extend(extra);
            assert!(!run(&args).status.success());
            assert!(!output.exists());
        }
        for index in [1u16, 5] {
            let selected = index.to_string();
            let result = run(&[
                "import",
                input,
                "-o",
                dest,
                "--entry",
                &selected,
                "--allow-loss",
                &losses,
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let doc = pcnd::read(&fs::read(&output).unwrap()).unwrap();
            let decoded = icons::decode_entry(&bytes, index, 65536).unwrap();
            assert_eq!(doc.canvas, (decoded.entry.width, decoded.entry.height));
            let raster = &doc.layers[&doc.roots[0]].raster;
            for (pixel, rgba) in decoded.rgba.as_chunks::<4>().0.iter().enumerate() {
                assert_eq!(
                    raster
                        .get(
                            (pixel as u32 % doc.canvas.0) as i64,
                            (pixel as u32 / doc.canvas.0) as i64
                        )
                        .0,
                    *rgba
                );
            }
            fs::remove_file(&output).unwrap();
        }
        assert!(
            !run(&[
                "import",
                input,
                "-o",
                input,
                "--overwrite",
                "--entry",
                "1",
                "--allow-loss",
                &losses
            ])
            .status
            .success()
        );
        assert_eq!(fs::read(source).unwrap(), bytes);
    }
    fs::remove_dir_all(folder).unwrap();
}
