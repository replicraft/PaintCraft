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
fn layered_copy_keeps_every_pixel_and_layer_property_without_mutating_source() {
    let mut doc = document();
    let id = doc.roots[1];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.name = "日本語 & \"ink\"\tline\nnext\rreturn".into();
    layer.opacity = 201;
    let before = doc.clone();
    assert!(ora::preflight(&doc, &Limits::default()).unwrap().is_empty());
    let bytes = ora::encode_document(&doc, &Limits::default(), &|| Ok(())).unwrap();
    let again = ora::encode_document(&doc, &Limits::default(), &|| Ok(())).unwrap();
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
}

#[test]
fn every_observed_cancellation_discards_private_output_and_failed_sink_is_reported() {
    let doc = document();
    let before = doc.clone();
    let limits = Limits::default();
    let calls = Cell::new(0);
    ora::encode_document(&doc, &limits, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let error = ora::encode_document(&doc, &limits, &|| {
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
        ora::write_document_to(&doc, Failed, &limits, &|| Ok(()))
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
            ora::encode_document(&doc, &limits, &|| Ok(())).is_err(),
            "{limits:?}"
        );
    }
    let mut invalid = doc.clone();
    invalid.roots[1] = invalid.roots[0];
    assert!(ora::encode_document(&invalid, &Limits::default(), &|| Ok(())).is_err());
    let mut invalid = doc.clone();
    invalid.layers.get_mut(&doc.roots[0]).unwrap().origin.0 = i64::from(i32::MAX) + 1;
    assert!(ora::encode_document(&invalid, &Limits::default(), &|| Ok(())).is_err());
    let mut invalid = doc.clone();
    invalid.layers.get_mut(&doc.roots[0]).unwrap().name = "invalid\0name".into();
    assert!(ora::encode_document(&invalid, &Limits::default(), &|| Ok(())).is_err());
}

#[test]
#[ignore = "requires an independent Python standard-library ZIP/XML/PNG oracle"]
fn independent_python_decodes_layered_writer_and_checks_every_preview_pixel() {
    let doc = document();
    let bytes = ora::encode_document(&doc, &Limits::default(), &|| Ok(())).unwrap();
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
