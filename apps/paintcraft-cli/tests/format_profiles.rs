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

#[test]
fn capabilities_report_exact_native_jxl_variant_without_generic_read_claim() {
    use paintcraft_core::types::json::Value;
    for profile in ["studio", "classic-v7", "classic-v10", "classic-v11"] {
        let output = run(&["--json", "capabilities", "--frontend", profile]);
        assert!(output.status.success());
        let value = json::parse(&output.stdout).unwrap();
        let rows = value.get("capabilities").and_then(Value::as_array).unwrap();
        let variants: Vec<_> = rows
            .iter()
            .filter(|row| {
                row.get("variant").and_then(Value::as_str)
                    == Some(paintcraft_formats::codec_worker::NATIVE_JXL_IMPORT_VARIANT)
            })
            .collect();
        assert_eq!(variants.len(), usize::from(profile == "studio"));
        if let Some(row) = variants.first() {
            assert_eq!(row.get("operation").and_then(Value::as_str), Some("import"));
            assert_eq!(row.get("platform").and_then(Value::as_str), Some("native"));
            assert_eq!(
                row.get("installation_preflight_required")
                    .and_then(Value::as_bool),
                Some(true)
            );
        }
        assert!(
            !rows
                .iter()
                .any(|row| row.get("capability").and_then(Value::as_str) == Some("jxl.rgba8.read"))
        );
    }
}

#[test]
fn capabilities_report_avif_exact_native_import_with_required_v5_preflight() {
    use paintcraft_core::types::json::Value;
    use paintcraft_formats::codec_worker as wire;
    for profile in [
        None,
        Some("studio"),
        Some("classic-v7"),
        Some("classic-v10"),
        Some("classic-v11"),
    ] {
        let mut args = vec!["--json", "capabilities"];
        if let Some(profile) = profile {
            args.extend(["--frontend", profile]);
        }
        let output = run(&args);
        assert!(output.status.success());
        let value = json::parse(&output.stdout).unwrap();
        let rows = value.get("capabilities").and_then(Value::as_array).unwrap();
        let variants: Vec<_> = rows
            .iter()
            .filter(|row| {
                row.get("variant").and_then(Value::as_str) == Some(wire::NATIVE_AVIF_IMPORT_VARIANT)
            })
            .collect();
        assert_eq!(
            variants.len(),
            usize::from(profile.is_none_or(|value| value == "studio"))
        );
        if let Some(row) = variants.first() {
            let accepted: Vec<_> = row
                .get("accepted_helper_profiles")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .map(|profile| profile.as_str().expect("explicit profile string"))
                .collect();
            assert_eq!(
                accepted.as_slice(),
                &[
                    wire::AVIF_NATIVE_PROFILE,
                    wire::AVIF_SDR_NATIVE_PROFILE,
                    wire::FLOAT_NATIVE_PROFILE,
                    wire::jxr_encode::qualified::NATIVE_PROFILE
                ]
            );
            assert_eq!(
                row.get("required_helper_profile_is_minimum")
                    .and_then(Value::as_bool),
                Some(true)
            );
            for (key, expected) in [
                ("capability", "avif.srgb8.import"),
                ("profile", "frontend/studio"),
                ("operation", "import"),
                ("backend_id", "dav1d/native-worker"),
                ("platform", "native"),
                ("required_helper_profile", wire::AVIF_NATIVE_PROFILE),
            ] {
                assert_eq!(row.get(key).and_then(Value::as_str), Some(expected));
            }
            for (key, expected) in [
                ("installation_preflight_required", true),
                ("runtime_installation_verified", false),
                ("release_qualified", false),
            ] {
                assert_eq!(row.get(key).and_then(Value::as_bool), Some(expected));
            }
        }
        assert!(!rows.iter().any(|row| row.get("capability").and_then(Value::as_str) == Some("avif.rgba8.read")));
    }
}

#[test]
fn capabilities_report_avif_sdr_exact_native_import_with_required_v6_preflight() {
    use paintcraft_core::types::json::Value;
    use paintcraft_formats::codec_worker as wire;
    for profile in [
        None,
        Some("studio"),
        Some("classic-v7"),
        Some("classic-v10"),
        Some("classic-v11"),
    ] {
        let mut args = vec!["--json", "capabilities"];
        if let Some(profile) = profile {
            args.extend(["--frontend", profile]);
        }
        let output = run(&args);
        assert!(output.status.success());
        let value = json::parse(&output.stdout).unwrap();
        let rows = value.get("capabilities").and_then(Value::as_array).unwrap();
        let variants: Vec<_> = rows
            .iter()
            .filter(|row| {
                row.get("variant").and_then(Value::as_str)
                    == Some(wire::NATIVE_AVIF_SDR_IMPORT_VARIANT)
            })
            .collect();
        assert_eq!(
            variants.len(),
            usize::from(profile.is_none_or(|value| value == "studio"))
        );
        if let Some(row) = variants.first() {
            let accepted: Vec<_> = row
                .get("accepted_helper_profiles")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .map(|profile| profile.as_str().expect("explicit profile string"))
                .collect();
            assert_eq!(
                accepted.as_slice(),
                &[
                    wire::AVIF_SDR_NATIVE_PROFILE,
                    wire::FLOAT_NATIVE_PROFILE,
                    wire::jxr_encode::qualified::NATIVE_PROFILE
                ]
            );
            assert_eq!(
                row.get("required_helper_profile_is_minimum")
                    .and_then(Value::as_bool),
                Some(true)
            );
            for (key, expected) in [
                ("capability", "avif.sdr-ycbcr8.import"),
                ("profile", "frontend/studio"),
                ("operation", "import"),
                ("backend_id", "dav1d/native-worker"),
                ("platform", "native"),
                ("required_helper_profile", wire::AVIF_SDR_NATIVE_PROFILE),
            ] {
                assert_eq!(row.get(key).and_then(Value::as_str), Some(expected));
            }
            for (key, expected) in [
                ("installation_preflight_required", true),
                ("runtime_installation_verified", false),
                ("release_qualified", false),
            ] {
                assert_eq!(row.get(key).and_then(Value::as_bool), Some(expected));
            }
        }
        assert!(!rows.iter().any(|row| row.get("capability").and_then(Value::as_str) == Some("avif.rgba8.read")));
    }
}

#[test]
fn capabilities_report_float_conversion_only_with_exact_v7_preflight() {
    use paintcraft_core::types::json::Value;
    use paintcraft_formats::codec_worker as wire;
    for profile in [
        None,
        Some("studio"),
        Some("classic-v7"),
        Some("classic-v10"),
        Some("classic-v11"),
    ] {
        let mut args = vec!["--json", "capabilities"];
        if let Some(profile) = profile {
            args.extend(["--frontend", profile]);
        }
        let output = run(&args);
        assert!(output.status.success());
        let value = json::parse(&output.stdout).unwrap();
        let rows = value.get("capabilities").and_then(Value::as_array).unwrap();
        for (family, variant, capability, backend) in [
            (
                "hdr",
                wire::NATIVE_HDR_SDR_IMPORT_VARIANT,
                "hdr.sdr8.import",
                "image-hdr/native-worker",
            ),
            (
                "exr",
                wire::NATIVE_EXR_SDR_IMPORT_VARIANT,
                "exr.sdr8.import",
                "exr/native-worker",
            ),
        ] {
            let variants: Vec<_> = rows
                .iter()
                .filter(|row| row.get("variant").and_then(Value::as_str) == Some(variant))
                .collect();
            assert_eq!(
                variants.len(),
                usize::from(profile.is_none_or(|value| value == "studio"))
            );
            if let Some(row) = variants.first() {
                for (key, expected) in [
                    ("capability", capability),
                    ("profile", "frontend/studio"),
                    ("operation", "import"),
                    ("backend_id", backend),
                    ("platform", "native"),
                    ("required_helper_profile", wire::FLOAT_NATIVE_PROFILE),
                ] {
                    assert_eq!(row.get(key).and_then(Value::as_str), Some(expected));
                }
                for (key, expected) in [
                    ("installation_preflight_required", true),
                    ("runtime_installation_verified", false),
                    ("release_qualified", false),
                    ("required_helper_profile_is_minimum", false),
                ] {
                    assert_eq!(row.get(key).and_then(Value::as_bool), Some(expected));
                }
                let accepted: Vec<_> = row
                    .get("accepted_helper_profiles")
                    .and_then(Value::as_array)
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap())
                    .collect();
                assert_eq!(
                    accepted,
                    [
                        wire::FLOAT_NATIVE_PROFILE,
                        wire::jxr_encode::qualified::NATIVE_PROFILE
                    ]
                );
            }
            assert!(!rows.iter().any(|row| {
                row.get("capability")
                    .and_then(Value::as_str)
                    .is_some_and(|value| {
                        value.starts_with(&format!("{family}.")) && value.ends_with(".read")
                    })
            }));
        }
    }
}

#[path = "format_profiles/jxr.rs"]
mod jxr;

#[test]
fn capabilities_report_explicit_dds_texture_without_replacing_default_rgba() {
    use paintcraft_formats::dds;
    use serde_json::Value;
    for profile in [
        None,
        Some("studio"),
        Some("classic-v7"),
        Some("classic-v10"),
        Some("classic-v11"),
    ] {
        let mut args = vec!["--json", "capabilities"];
        if let Some(profile) = profile {
            args.extend(["--frontend", profile]);
        }
        let output = run(&args);
        assert!(output.status.success());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = value["capabilities"].as_array().unwrap();
        let advanced = rows
            .iter()
            .find(|row| row["capability"] == "dds.texture2d.write");
        assert_eq!(advanced.is_some(), matches!(profile, None | Some("studio")));
        if let Some(row) = advanced {
            assert_eq!(row["variant"], dds::NATIVE_TEXTURE_EXPORT_VARIANT);
            assert_eq!(row["platform"], "native");
            assert_eq!(row["operation"], "fresh-write");
            assert_eq!(row["may_be_lossy"], true);
            assert_eq!(row["preflight_required"], true);
            assert_eq!(row["external_helper_required"], false);
            assert_eq!(row["installation_preflight_required"], false);
            assert_eq!(row["runtime_installation_verified"], false);
            assert_eq!(row["release_qualified"], false);
            assert_eq!(row["evidence"], "unverified");
            assert_eq!(
                row["encodings"],
                serde_json::json!(["rgba8", "bc1", "bc2", "bc3"])
            );
            assert_eq!(row["filters"], serde_json::json!(["nearest", "box-stored"]));
            assert_eq!(row["mips"], serde_json::json!(["none", "full"]));
            assert_eq!(row["levels_min"], 1);
            assert_eq!(row["levels_max"], 17);
            assert_eq!(row["levels_geometry_bounded"], true);
            assert_eq!(row["mips_levels_mutually_exclusive"], true);
            assert_eq!(row["compression_policy"], dds::UNIFORM_LOSSY_POLICY);
            assert_eq!(row["pixel_limit"], dds::PIXELS_MAX);
        }
        if profile == Some("studio") {
            let old = rows
                .iter()
                .find(|row| row["capability"] == "dds.rgba8.write")
                .unwrap();
            assert_eq!(old["backend_id"], "paintcraft-dds-rgba8");
            assert!(old.get("variant").is_none());
        }
    }
}

#[test]
fn capabilities_discover_exact_native_msp2_without_expanding_classic_or_default() {
    use serde_json::Value;
    for profile in [
        None,
        Some("studio"),
        Some("classic-v7"),
        Some("classic-v10"),
        Some("classic-v11"),
    ] {
        let mut args = vec!["--json", "capabilities"];
        if let Some(profile) = profile {
            args.extend(["--frontend", profile]);
        }
        let reply = run(&args);
        assert!(reply.status.success());
        let value: Value = serde_json::from_slice(&reply.stdout).unwrap();
        let rows = value["capabilities"].as_array().unwrap();
        let row = rows
            .iter()
            .find(|row| row["capability"] == "msp.monochrome.v2.write");
        assert_eq!(row.is_some(), matches!(profile, None | Some("studio")));
        if let Some(row) = row {
            assert_eq!(
                row["variant"],
                paintcraft_formats::msp::NATIVE_V2_EXPORT_VARIANT
            );
            assert_eq!(row["parameter"], "msp_version");
            assert_eq!(row["parameter_value"], "v2");
            assert_eq!(row["default_output_version"], "v1");
            assert_eq!(row["platform"], "native");
            assert_eq!(row["release_qualified"], false);
            assert_eq!(row["external_helper_required"], false);
            assert_eq!(row["original_application_qualified"], false);
            assert_eq!(row["pixel_limit"], paintcraft_formats::msp::PIXELS_MAX);
            assert_eq!(
                row["encoded_bytes_limit"],
                paintcraft_formats::msp::BYTES_MAX
            );
        }
    }
}
