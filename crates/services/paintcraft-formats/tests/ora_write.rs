use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    ora::{self, Limits},
    project_import,
};
use std::{cell::Cell, io::Write};

fn document() -> paintcraft_document::Document {
    project_import::decode_ora(include_bytes!("ora-fixtures/deflated.ora"), 100, &|| Ok(()))
        .unwrap()
        .document
}

#[test]
fn all_8bit_layer_opacities_survive_the_xml_float_representation_exactly() {
    let mut doc = document();
    let id = doc.roots[1];
    let limits = Limits::default();
    for opacity in 0..=255u8 {
        doc.layers.get_mut(&id).unwrap().opacity = opacity;
        let bytes = ora::encode_document(&doc, &limits, &[], &|| Ok(())).unwrap();
        let decoded = ora::decode_layers(&bytes, &limits, &|| Ok(())).unwrap();
        assert_eq!(decoded.layers[1].opacity, opacity);
        assert!(decoded.losses.is_empty(), "{opacity}: {:?}", decoded.losses);
    }
}

#[test]
fn layered_copy_keeps_every_pixel_and_layer_property_without_mutating_source() {
    let mut doc = document();
    let id = doc.roots[1];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.name = "日本語 & \"ink\"\tline\nnext\rreturn".into();
    layer.opacity = 201;
    let before = doc.clone();
    assert!(ora::preflight(&doc, &Limits::default()).unwrap().is_empty());
    let bytes = ora::encode_document(&doc, &Limits::default(), &[], &|| Ok(())).unwrap();
    let again = ora::encode_document(&doc, &Limits::default(), &[], &|| Ok(())).unwrap();
    assert_eq!(bytes, again, "fresh copies are deterministic");
    let decoded = ora::decode_layers(&bytes, &Limits::default(), &|| Ok(())).unwrap();
    assert!(decoded.losses.is_empty());
    assert_eq!((decoded.width, decoded.height), doc.canvas);
    for (actual, id) in decoded.layers.iter().zip(&doc.roots) {
        let expected = &doc.layers[id];
        assert_eq!(actual.name, expected.name);
        assert_eq!(
            actual.origin,
            (expected.origin.0 as i32, expected.origin.1 as i32)
        );
        assert_eq!(actual.opacity, expected.opacity);
        assert_eq!(actual.visible, expected.visible);
        for y in 0..actual.height {
            for x in 0..actual.width {
                assert_eq!(
                    &actual.rgba[((y * actual.width + x) * 4) as usize..][..4],
                    &expected.raster.get(i64::from(x), i64::from(y)).0
                );
            }
        }
    }
    assert!(doc.same_semantics(&before));
    doc.title = Some("A native title".into());
    assert_eq!(
        ora::preflight(&doc, &Limits::default()).unwrap(),
        [ora::TITLE_LOSS]
    );
    assert_eq!(
        ora::encode_document(&doc, &Limits::default(), &[], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let bytes = ora::encode_document(&doc, &Limits::default(), &[ora::TITLE_LOSS.into()], &|| {
        Ok(())
    })
    .unwrap();
    assert!(
        ora::decode_layers(&bytes, &Limits::default(), &|| Ok(()))
            .unwrap()
            .title
            .is_none()
    );
    assert_eq!(
        ora::encode_document(&doc, &Limits::default(), &["unknown-loss".into()], &|| Ok(
            ()
        ))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}

#[test]
fn every_observed_cancellation_discards_private_output_and_failed_sink_is_reported() {
    let doc = document();
    let before = doc.clone();
    let limits = Limits::default();
    let calls = Cell::new(0);
    ora::encode_document(&doc, &limits, &[], &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let error = ora::encode_document(&doc, &limits, &[], &|| {
            current.set(current.get() + 1);
            if current.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    struct Failed;
    impl Write for Failed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected sink failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        ora::write_document_to(&doc, Failed, &limits, &[], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::StorageFailed
    );
    assert!(doc.same_semantics(&before));
}

#[test]
fn geometry_roots_xml_and_archive_limits_refuse_unrepresentable_copies() {
    let doc = document();
    for limits in [
        Limits {
            archive_bytes: 64,
            ..Limits::default()
        },
        Limits {
            entry_bytes: 8,
            ..Limits::default()
        },
        Limits {
            inflated_bytes: 32,
            ..Limits::default()
        },
        Limits {
            entries: 6,
            ..Limits::default()
        },
        Limits {
            pixels: 7,
            ..Limits::default()
        },
        Limits {
            decoded_bytes: 28,
            ..Limits::default()
        },
        Limits {
            xml_bytes: 1,
            ..Limits::default()
        },
        Limits {
            metadata_bytes: 1,
            ..Limits::default()
        },
        Limits {
            path_bytes: 8,
            ..Limits::default()
        },
        Limits {
            layers: 2,
            ..Limits::default()
        },
    ] {
        assert!(
            ora::encode_document(&doc, &limits, &[], &|| Ok(())).is_err(),
            "{limits:?}"
        );
    }
    let mut invalid = doc.clone();
    invalid.roots[1] = invalid.roots[0];
    assert!(ora::encode_document(&invalid, &Limits::default(), &[], &|| Ok(())).is_err());
    let mut invalid = doc.clone();
    invalid.layers.get_mut(&doc.roots[0]).unwrap().origin.0 = i64::from(i32::MAX) + 1;
    assert!(ora::encode_document(&invalid, &Limits::default(), &[], &|| Ok(())).is_err());
    let mut invalid = doc.clone();
    invalid.layers.get_mut(&doc.roots[0]).unwrap().name = "invalid\0name".into();
    assert!(ora::encode_document(&invalid, &Limits::default(), &[], &|| Ok(())).is_err());
}

#[test]
#[ignore = "requires an independent Python standard-library ZIP/XML/PNG oracle"]
fn independent_python_decodes_layered_writer_and_checks_every_preview_pixel() {
    let doc = document();
    let bytes = ora::encode_document(&doc, &Limits::default(), &[], &|| Ok(())).unwrap();
    let path =
        std::env::temp_dir().join(format!("paintcraft-ora-writer-{}.ora", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let output = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/ora-fixtures/verify_writer.py"
        ))
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires a supplied GIMP 3 console with Python, GEGL and OpenRaster plugins"]
fn gimp_reopens_edits_and_saves_exported_layers() {
    let program = std::env::var_os("PAINTCRAFT_GIMP").expect("PAINTCRAFT_GIMP console executable");
    let folder = std::env::temp_dir().join(format!("paintcraft-gimp-ora-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("source.ora");
    let report = folder.join("report.json");
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ora-fixtures");
    let bytes = ora::encode_document(&document(), &Limits::default(), &[], &|| Ok(())).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let script = fixtures.join("verify_gimp.py");
    let batch = format!(
        "exec(compile(open({:?}, encoding='utf-8').read(), {:?}, 'exec'))",
        script.to_str().unwrap(),
        script.to_str().unwrap()
    );
    let mut command = std::process::Command::new(program);
    command
        .args([
            "--no-interface",
            "--no-splash",
            "--no-data",
            "--no-fonts",
            "--new-instance",
            "--console-messages",
            "--batch-interpreter=python-fu-eval",
            "--batch",
            &batch,
            "--quit",
        ])
        .env("GIMP3_DIRECTORY", folder.join("profile"))
        .env("XDG_CONFIG_HOME", folder.join("config"))
        .env("GIMP3_CACHEDIR", folder.join("cache"))
        .env("XDG_CACHE_HOME", folder.join("cache"))
        .env("PAINTCRAFT_ORA_INPUT", &path)
        .env("PAINTCRAFT_ORA_REPORT", &report)
        .env("PAINTCRAFT_ORA_FIXTURES", fixtures);
    #[cfg(unix)]
    if let Some(root) = std::env::var_os("PAINTCRAFT_GIMP_PLUGINS") {
        let plugins = folder.join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        for name in ["file-openraster", "file-png", "python-eval"] {
            std::os::unix::fs::symlink(std::path::Path::new(&root).join(name), plugins.join(name))
                .unwrap();
        }
        let config = folder.join("gimprc");
        std::fs::write(
            &config,
            format!("(plug-in-path {:?})\n", plugins.to_str().unwrap()),
        )
        .unwrap();
        command.arg("--gimprc").arg(config);
    }
    let output = bounded_reference(command, &folder);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result = std::fs::read_to_string(&report).unwrap_or_else(|_| {
        panic!(
            "No oracle report: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert!(result.contains("\"status\": \"verified\""));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    println!("{result}");
    std::fs::remove_dir_all(folder).unwrap();
}

fn bounded_reference(
    mut command: std::process::Command,
    folder: &std::path::Path,
) -> std::process::Output {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let stdout = folder.join("stdout.log");
    let stderr = folder.join("stderr.log");
    command.stdout(Stdio::from(std::fs::File::create(&stdout).unwrap()));
    command.stderr(Stdio::from(std::fs::File::create(&stderr).unwrap()));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "External GIMP oracle exceeded 120 seconds; logs retained in {}",
                folder.display()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    std::process::Output {
        status,
        stdout: std::fs::read(stdout).unwrap(),
        stderr: std::fs::read(stderr).unwrap(),
    }
}
