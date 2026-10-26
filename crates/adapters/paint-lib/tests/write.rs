//! Fresh observed-profile copies, not a claim of Windows Paint qualification.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_paint::{self as paint, WriteDocument, WriteLayer, WriteLimits};
use std::{cell::Cell, io::Write};
const PIXELS: [u8; 24] = [
    10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
    170, 180, 255,
];
const SECOND: [u8; 24] = [
    9, 8, 7, 255, 6, 5, 4, 254, 3, 2, 1, 128, 200, 180, 160, 127, 140, 120, 100, 1, 80, 60, 40, 0,
];
fn layer() -> WriteLayer<'static> {
    WriteLayer {
        width: 3,
        height: 2,
        origin: (0, 0),
        item_name: "literal 日本語🙂",
        rgba: &PIXELS,
        visible: true,
        opacity: 255,
        metadata: &[],
    }
}
fn doc<'a>(layers: &'a [WriteLayer<'a>]) -> WriteDocument<'a> {
    WriteDocument {
        width: 9,
        height: 7,
        canvas_fill: [0x1234, 0xabcd, 0x7f00, 0xffff],
        layers,
        metadata: &[],
    }
}
#[test]
fn exact_pixels_fill16_names_geometry_and_signed_offsets_are_deterministic() {
    for origin in [(1, -2), (60000, -70000), (i32::MIN, i32::MAX)] {
        let layers = [
            WriteLayer { origin, ..layer() },
            WriteLayer {
                width: 2,
                height: 3,
                origin: (-3, 4),
                item_name: "another item",
                rgba: &SECOND,
                ..layer()
            },
        ];
        let source = doc(&layers);
        let plan = paint::preflight_write(&source, &Default::default(), &|| Ok(())).unwrap();
        assert_eq!(
            plan.wide_overlay,
            i16::try_from(origin.0).is_err() || i16::try_from(origin.1).is_err()
        );
        let bytes = paint::encode_document(&source, &Default::default(), &|| Ok(())).unwrap();
        assert_eq!(
            bytes,
            paint::encode_document(&source, &Default::default(), &|| Ok(())).unwrap()
        );
        assert!(bytes.len() <= plan.output_capacity);
        let decoded = paint::decode_layers(&bytes, &Default::default(), &|| Ok(())).unwrap();
        assert_eq!((decoded.width, decoded.height), (9, 7));
        assert_eq!(decoded.canvas_fill, source.canvas_fill);
        for (actual, expected) in decoded.layers.iter().zip(&layers) {
            assert_eq!(actual.item_name, expected.item_name);
            assert_eq!(actual.origin, expected.origin);
            assert_eq!(
                (actual.width, actual.height),
                (expected.width, expected.height)
            );
            assert_eq!(actual.rgba, expected.rgba);
        }
    }
    let layers = [layer()];
    let mut source = doc(&layers);
    source.width = 65536;
    source.height = 1;
    let bytes = paint::encode_document(&source, &Default::default(), &|| Ok(())).unwrap();
    assert_eq!(
        paint::decode_layers(&bytes, &Default::default(), &|| Ok(()))
            .unwrap()
            .width,
        65536
    );
}
#[test]
fn unsupported_semantics_and_all_declared_limits_refuse_before_sink_work() {
    let original = layer();
    let source = doc(std::slice::from_ref(&original));
    for case in 0..14 {
        let mut bad_layer = original;
        let mut bad = source;
        let mut limits = WriteLimits::default();
        match case {
            0 => bad.width = 0,
            1 => bad.width = 65537,
            2 => bad.layers = &[],
            3 => bad_layer.width = 0,
            4 => bad_layer.rgba = &PIXELS[..23],
            5 => bad_layer.visible = false,
            6 => bad_layer.opacity = 254,
            7 => bad_layer.metadata = &[("unknown", "source")],
            8 => bad.metadata = &[("unknown", "document")],
            9 => bad_layer.item_name = "bad\0name",
            10 => limits.pixels = 1,
            11 => limits.metadata_bytes = 1,
            12 => limits.encoded_bytes = 1,
            _ => limits.transient_bytes = 1,
        }
        if matches!(case, 3..=7 | 9) {
            bad.layers = std::slice::from_ref(&bad_layer);
        }
        let mut sink = Vec::new();
        assert!(
            paint::write_document_to(&bad, &limits, &mut sink, &|| Ok(())).is_err(),
            "case{case}"
        );
        assert!(sink.is_empty());
    }
    let too_many = vec![original; 65];
    assert!(paint::preflight_write(&doc(&too_many), &Default::default(), &|| Ok(())).is_err());
    let boundary = "界".repeat(341) + "x";
    for name in ["", boundary.as_str()] {
        let valid = WriteLayer {
            item_name: name,
            ..original
        };
        let bytes =
            paint::encode_document(&doc(&[valid]), &Default::default(), &|| Ok(())).unwrap();
        assert_eq!(
            paint::decode_layers(&bytes, &Default::default(), &|| Ok(()))
                .unwrap()
                .layers[0]
                .item_name,
            name
        );
    }
    let long = "x".repeat(1025);
    let bad = WriteLayer {
        item_name: &long,
        ..original
    };
    assert!(paint::preflight_write(&doc(&[bad]), &Default::default(), &|| Ok(())).is_err());
}
#[test]
fn many_layers_and_incompressible_multichunk_streams_stay_bounded() {
    let mut rng = 0x9861_3cf5u32;
    let rgba: Vec<u8> = (0..1024 * 257 * 4)
        .map(|_| {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            rng as u8
        })
        .collect();
    let layers = [WriteLayer {
        width: 1024,
        height: 257,
        rgba: &rgba,
        ..layer()
    }];
    let source = doc(&layers);
    let plan = paint::preflight_write(&source, &Default::default(), &|| Ok(())).unwrap();
    let bytes = paint::encode_document(&source, &Default::default(), &|| Ok(())).unwrap();
    assert!(bytes.len() > rgba.len());
    assert!(bytes.len() < plan.output_capacity);
    assert_eq!(
        paint::decode_layers(&bytes, &Default::default(), &|| Ok(()))
            .unwrap()
            .layers[0]
            .rgba,
        rgba
    );
    let layers = vec![layer(); 64];
    let bytes = paint::encode_document(&doc(&layers), &Default::default(), &|| Ok(())).unwrap();
    assert_eq!(
        paint::decode_layers(&bytes, &Default::default(), &|| Ok(()))
            .unwrap()
            .layers
            .len(),
        64
    );
}
#[test]
fn every_observed_cancellation_checkpoint_and_failed_staged_sink_discard_output() {
    let rgba: Vec<u8> = (0..65544).map(|i| (i * 37) as u8).collect();
    let layers = [WriteLayer {
        width: 5462,
        height: 3,
        rgba: &rgba,
        ..layer()
    }];
    let source = doc(&layers);
    let original = rgba.clone();
    let count = Cell::new(0);
    paint::encode_document(&source, &Default::default(), &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let mut sink = Vec::new();
        let result = paint::write_document_to(&source, &Default::default(), &mut sink, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert!(sink.is_empty());
        assert_eq!(rgba, original);
    }
    struct Failed;
    impl Write for Failed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("failed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        paint::write_document_to(&source, &Default::default(), &mut Failed, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::StorageFailed
    );
}
#[test]
fn export_owned_writer_vectors_only_when_requested() {
    let folder = std::env::var_os("PAINTCRAFT_PAINT_WRITER_OUTPUT").map(std::path::PathBuf::from);
    if let Some(folder) = &folder {
        assert!(folder.is_dir());
        assert_eq!(std::fs::read_dir(folder).unwrap().count(), 0);
    }
    for (name, origin, canvas) in [
        ("narrow", (1, -2), (9, 7)),
        ("wide", (60000, -70000), (65536, 1)),
        ("extreme", (i32::MIN, i32::MAX), (9, 7)),
    ] {
        let layers = [
            WriteLayer { origin, ..layer() },
            WriteLayer {
                width: 2,
                height: 3,
                origin: (-3, 4),
                item_name: "second literal",
                rgba: &SECOND,
                ..layer()
            },
        ];
        let source = WriteDocument {
            width: canvas.0,
            height: canvas.1,
            ..doc(&layers)
        };
        let bytes = paint::encode_document(&source, &Default::default(), &|| Ok(())).unwrap();
        let qualified: &[u8] = match name {
            "narrow" => include_bytes!("fixtures/writer/narrow.paint"),
            "wide" => include_bytes!("fixtures/writer/wide.paint"),
            "extreme" => include_bytes!("fixtures/writer/extreme.paint"),
            _ => unreachable!(),
        };
        assert_eq!(bytes, qualified, "qualified {name} byte drift");
        if let Some(folder) = &folder {
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(folder.join(format!("{name}.paint")))
                .unwrap();
            output.write_all(&bytes).unwrap();
        }
    }
}
