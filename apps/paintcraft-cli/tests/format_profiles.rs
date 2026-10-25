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
        assert_eq!(
            reply.status.code(),
            Some(if profile == "studio" { 2 } else { 3 })
        );
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
fn gif_and_tiff_profiles_require_explicit_conversion_loss_acknowledgements() {
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
            let exported = dir.join(format!("{profile}-export.{extension}"));
            let mut args = vec![
                "--json",
                "render",
                destination.to_str().unwrap(),
                "--frontend",
                profile,
                "-o",
                exported.to_str().unwrap(),
            ];
            if format == ImageFormat::Gif {
                args.extend(["--allow-loss", "palette-quantized"]);
            }
            let result = run(&args);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            let decoded_export = image::decode(&fs::read(&exported).unwrap()).unwrap();
            assert_eq!(decoded_export.rgba, decoded.rgba);
        }
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn studio_tga_webp_roundtrip_but_classic_does_not_inherit_these_formats() {
    use paintcraft_core::types::Rgba;
    use paintcraft_formats::{
        image::{self, ImageFormat},
        pcnd,
    };
    let dir = std::env::temp_dir().join(format!("paintcraft-studio-raster-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let document = paintcraft_document::new_document(
        3,
        2,
        Some(Rgba([17, 51, 93, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let project = dir.join("source.pcnd");
    fs::write(&project, pcnd::write(&document).unwrap()).unwrap();
    for (format, extension) in [(ImageFormat::Tga, "tga"), (ImageFormat::WebP, "webp")] {
        let output = dir.join(format!("output.{extension}"));
        let result = run(&[
            "--json",
            "render",
            project.to_str().unwrap(),
            "--frontend",
            "studio",
            "--format",
            extension,
            "-o",
            output.to_str().unwrap(),
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let encoded = fs::read(&output).unwrap();
        assert_eq!(ImageFormat::sniff(&encoded), Some(format));
        assert_eq!(
            image::decode(&encoded).unwrap().rgba,
            paintcraft_render::render_flat(&document).unwrap()
        );
        for profile in ["classic-v7", "classic-v10", "classic-v11"] {
            let denied = dir.join(format!("{profile}-denied.{extension}"));
            assert!(
                !run(&[
                    "--json",
                    "render",
                    project.to_str().unwrap(),
                    "--frontend",
                    profile,
                    "-o",
                    denied.to_str().unwrap()
                ])
                .status
                .success()
            );
            assert!(!denied.exists());
            let imported = dir.join(format!("{profile}-import-denied.pcnd"));
            assert!(
                !run(&[
                    "--json",
                    "import",
                    output.to_str().unwrap(),
                    "--frontend",
                    profile,
                    "-o",
                    imported.to_str().unwrap()
                ])
                .status
                .success()
            );
            assert!(!imported.exists());
        }
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn studio_specialist_exports_require_named_losses_and_refuse_classic_routes() {
    use paintcraft_core::types::Rgba;
    use paintcraft_formats::{image::ImageFormat, pcnd};
    let dir = std::env::temp_dir().join(format!(
        "paintcraft-specialist-export-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let document = paintcraft_document::new_document(
        4,
        3,
        Some(Rgba([17, 51, 93, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let project = dir.join("source.pcnd");
    fs::write(&project, pcnd::write(&document).unwrap()).unwrap();
    for (format, extension) in [
        (ImageFormat::Jxl, "jxl"),
        (ImageFormat::Dds, "dds"),
        (ImageFormat::Avif, "avif"),
        (ImageFormat::Pcx, "pcx"),
    ] {
        let output = dir.join(format!("output.{extension}"));
        let args = [
            "--json",
            "render",
            project.to_str().unwrap(),
            "--frontend",
            "studio",
            "--format",
            extension,
            "-o",
            output.to_str().unwrap(),
        ];
        let result = run(&args);
        if matches!(format, ImageFormat::Avif | ImageFormat::Pcx) {
            assert_eq!(result.status.code(), Some(3));
            assert!(!output.exists());
            let mut acknowledged = args.to_vec();
            acknowledged.extend([
                "--allow-loss",
                if format == ImageFormat::Pcx {
                    "alpha-discarded"
                } else {
                    "lossy-avif,alpha-lossy"
                },
            ]);
            assert!(run(&acknowledged).status.success());
        } else {
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
        }
        let bytes = fs::read(&output).unwrap();
        assert_eq!(ImageFormat::sniff(&bytes), Some(format));
        for profile in ["classic-v7", "classic-v10", "classic-v11"] {
            let denied = dir.join(format!("{profile}.{extension}"));
            assert!(
                !run(&[
                    "--json",
                    "render",
                    project.to_str().unwrap(),
                    "--frontend",
                    profile,
                    "-o",
                    denied.to_str().unwrap()
                ])
                .status
                .success()
            );
            assert!(!denied.exists());
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
