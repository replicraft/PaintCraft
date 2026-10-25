use paintcraft_core::types::json;
use std::{fs, path::PathBuf, process::Command};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn frontend_flags_share_admission_and_keep_the_existing_json_envelope() {
    let dir = std::env::temp_dir().join(format!(
        "paintcraft-profile-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("source.png");
    let png = paintcraft_formats::image::encode(
        paintcraft_formats::image::ImageFormat::Png,
        1,
        1,
        &[9, 8, 7, 0],
        90,
    )
    .unwrap();
    fs::write(&source, &png).unwrap();
    let path = |name: &str| -> PathBuf { dir.join(name) };
    for profile in ["classic-v7", "classic-v10", "classic-v11", "studio"] {
        let output = path(&format!("{profile}.pcnd"));
        let reply = run(&[
            "--json",
            "import",
            source.to_str().unwrap(),
            "--frontend",
            profile,
            "-o",
            output.to_str().unwrap(),
        ]);
        assert!(
            reply.status.success(),
            "{} {}",
            String::from_utf8_lossy(&reply.stdout),
            String::from_utf8_lossy(&reply.stderr)
        );
        let rendered = path(&format!("{profile}.png"));
        assert!(
            run(&[
                "--json",
                "render",
                output.to_str().unwrap(),
                "--frontend",
                profile,
                "--allow-loss",
                "hidden-rgb",
                "-o",
                rendered.to_str().unwrap()
            ])
            .status
            .success()
        );
        let unsupported = path("foreign.svg");
        fs::write(&unsupported, &png).unwrap();
        let denied = path(&format!("{profile}-denied.pcnd"));
        let reply = run(&[
            "--json",
            "import",
            unsupported.to_str().unwrap(),
            "--frontend",
            profile,
            "-o",
            denied.to_str().unwrap(),
        ]);
        assert_eq!(reply.status.code(), Some(3));
        assert!(!denied.exists());
        let value = json::parse(&reply.stdout).unwrap();
        assert!(value.get("schema").is_some());
        let reply = run(&["--json", "capabilities", "--frontend", profile]);
        assert!(reply.status.success());
        let value = json::parse(&reply.stdout).unwrap();
        let text = String::from_utf8(reply.stdout).unwrap();
        assert!(text.contains("frontend/"));
        assert!(text.contains("paintcraft.build-capabilities/v1"));
        assert!(value.get("schema").is_some());
    }
    let wrong = path("wrong.jpg");
    fs::write(&wrong, &png).unwrap();
    let output = path("wrong.pcnd");
    assert_eq!(
        run(&[
            "--json",
            "import",
            wrong.to_str().unwrap(),
            "-o",
            output.to_str().unwrap()
        ])
        .status
        .code(),
        Some(2)
    );
    assert!(!output.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn gif_and_tiff_read_profiles_require_loss_acknowledgement_without_enabling_exports() {
    use paintcraft_formats::image::{self, ImageFormat};
    let dir = std::env::temp_dir().join(format!(
        "paintcraft-qualified-read-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let allowed = image::KNOWN_IMPORT_LOSSES.join(",");
    for (format, extension) in [(ImageFormat::Gif, "gif"), (ImageFormat::Tiff, "tiff")] {
        let bytes = image::encode(format, 2, 1, &[255, 0, 0, 255, 0, 255, 0, 255], 90).unwrap();
        let source = dir.join(format!("image.{extension}"));
        fs::write(&source, &bytes).unwrap();
        let decoded = image::decode(&bytes).unwrap();
        assert!(!decoded.losses.is_empty());
        for profile in ["classic-v7", "classic-v10", "classic-v11", "studio"] {
            let destination = dir.join(format!("{profile}-{extension}.pcnd"));
            let result = run(&[
                "--json",
                "import",
                source.to_str().unwrap(),
                "--frontend",
                profile,
                "-o",
                destination.to_str().unwrap(),
            ]);
            assert_eq!(result.status.code(), Some(3));
            assert!(!destination.exists());
            let result = run(&[
                "--json",
                "import",
                source.to_str().unwrap(),
                "--frontend",
                profile,
                "--allow-loss",
                &allowed,
                "-o",
                destination.to_str().unwrap(),
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            let doc = paintcraft_formats::pcnd::read(&fs::read(&destination).unwrap()).unwrap();
            let layer = &doc.layers[&doc.roots[0]];
            assert_eq!(layer.raster.get(0, 0).0.as_slice(), &decoded.rgba[..4]);
            let rejected = dir.join(format!("{profile}-export.{extension}"));
            assert_eq!(
                run(&[
                    "--json",
                    "render",
                    destination.to_str().unwrap(),
                    "--frontend",
                    profile,
                    "-o",
                    rejected.to_str().unwrap()
                ])
                .status
                .code(),
                Some(3)
            );
            assert!(!rejected.exists());
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
