//! Fresh typed writer qualification, independent of public/original-app gates.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_pdn::{self as pdn, RasterDocument, RasterLayer, WriteLimits};
use std::{cell::Cell, io::Write};
const SOURCE: &[u8] = include_bytes!("fixtures/paintdotjs-2-layer.pdn");
fn layers<'a>(decoded: &'a pdn::Decoded<'a>) -> Vec<RasterLayer<'a>> {
    decoded
        .layers
        .iter()
        .map(|layer| RasterLayer {
            width: layer.width,
            height: layer.height,
            name: layer.name,
            visible: layer.visible,
            opacity: layer.opacity,
            is_background: layer.is_background,
            rgba: &layer.rgba,
            metadata: &[],
        })
        .collect()
}
#[test]
fn fresh_typed_graph_retains_exact_layers_versions_roles_and_hidden_rgb() {
    let decoded = pdn::decode_layers(SOURCE, &pdn::DecodeLimits::default(), &|| Ok(())).unwrap();
    let layers = layers(&decoded);
    let doc = RasterDocument {
        width: decoded.width,
        height: decoded.height,
        layers: &layers,
        saved_with: [0, 1, 0, 0],
        metadata: &[],
    };
    let limits = WriteLimits::default();
    let plan = pdn::preflight_write(&doc, &limits, &|| Ok(())).unwrap();
    let bytes = pdn::encode_document(&doc, &limits, &|| Ok(())).unwrap();
    assert!(bytes.len() <= plan.output_capacity);
    assert_eq!(
        bytes,
        pdn::encode_document(&doc, &limits, &|| Ok(())).unwrap()
    );
    let actual = pdn::decode_layers(&bytes, &pdn::DecodeLimits::default(), &|| Ok(())).unwrap();
    assert_eq!((actual.width, actual.height), (doc.width, doc.height));
    assert_eq!(actual.saved_with, doc.saved_with);
    assert_eq!(actual.header_version.as_deref(), Some("0.1.0.0"));
    for (actual, source) in actual.layers.iter().zip(&layers) {
        assert_eq!(actual.name, source.name);
        assert_eq!(actual.visible, source.visible);
        assert_eq!(actual.opacity, source.opacity);
        assert_eq!(actual.is_background, source.is_background);
        assert_eq!(actual.rgba, source.rgba);
    }
}
#[test]
fn declared_budget_geometry_metadata_and_names_refuse_before_sink_work() {
    let rgba = [1, 2, 3, 4];
    let layer = RasterLayer {
        width: 1,
        height: 1,
        name: "A",
        visible: true,
        opacity: 255,
        is_background: false,
        rgba: &rgba,
        metadata: &[],
    };
    let doc = RasterDocument {
        width: 1,
        height: 1,
        layers: std::slice::from_ref(&layer),
        saved_with: [0, 1, 0, 0],
        metadata: &[],
    };
    for case in 0..12 {
        let mut bad = doc;
        let mut bad_layer = layer;
        let mut limits = WriteLimits::default();
        match case {
            0 => bad.width = 0,
            1 => bad.width = 32769,
            2 => bad.layers = &[],
            3 => bad_layer.width = 2,
            4 => bad_layer.rgba = &rgba[..3],
            5 => bad_layer.metadata = &[("exif", "source")],
            6 => bad.metadata = &[("source", "retained")],
            7 => bad.saved_with = [u32::MAX; 4],
            8 => limits.metadata_bytes = 1,
            9 => limits.encoded_bytes = 1,
            10 => limits.transient_bytes = 1,
            _ => limits.pixels = 0,
        }
        if matches!(case, 3..=5) {
            bad.layers = std::slice::from_ref(&bad_layer);
        }
        let mut sink = Vec::new();
        assert!(
            pdn::write_document_to(&bad, &limits, &mut sink, &|| Ok(())).is_err(),
            "case{case}"
        );
        assert!(sink.is_empty());
    }
    let long = "界".repeat(1366);
    for name in [long.as_str(), "bad\0name"] {
        let bad_layer = RasterLayer { name, ..layer };
        let bad = RasterDocument {
            layers: std::slice::from_ref(&bad_layer),
            ..doc
        };
        assert!(pdn::preflight_write(&bad, &WriteLimits::default(), &|| Ok(())).is_err());
    }
}
#[test]
fn all_opacities_and_maximum_layer_count_have_exact_typed_records() {
    let rgba = [21, 49, 78, 0];
    for opacity in 0..=255 {
        let layer = RasterLayer {
            width: 1,
            height: 1,
            name: "日本語🙂",
            visible: opacity % 2 == 0,
            opacity,
            is_background: opacity % 3 == 0,
            rgba: &rgba,
            metadata: &[],
        };
        let doc = RasterDocument {
            width: 1,
            height: 1,
            layers: std::slice::from_ref(&layer),
            saved_with: [0, 1, 0, 0],
            metadata: &[],
        };
        let bytes = pdn::encode_document(&doc, &WriteLimits::default(), &|| Ok(())).unwrap();
        let actual = pdn::decode_layers(&bytes, &pdn::DecodeLimits::default(), &|| Ok(())).unwrap();
        assert_eq!(actual.layers[0].opacity, opacity);
        assert_eq!(actual.layers[0].rgba, rgba);
    }
    let layers = vec![
        RasterLayer {
            width: 1,
            height: 1,
            name: "same",
            visible: true,
            opacity: 255,
            is_background: false,
            rgba: &rgba,
            metadata: &[]
        };
        64
    ];
    let doc = RasterDocument {
        width: 1,
        height: 1,
        layers: &layers,
        saved_with: [0, 1, 0, 0],
        metadata: &[],
    };
    let bytes = pdn::encode_document(&doc, &WriteLimits::default(), &|| Ok(())).unwrap();
    assert_eq!(
        pdn::decode_layers(&bytes, &pdn::DecodeLimits::default(), &|| Ok(()))
            .unwrap()
            .layers
            .len(),
        64
    );
}
#[test]
fn every_cancellation_checkpoint_and_failed_staged_sink_discard_private_output() {
    // More than one compressed chunk, with original arithmetic samples.
    let rgba: Vec<u8> = (0..262_152).map(|i| (i * 37 + i / 17) as u8).collect();
    let source = rgba.clone();
    let layer = RasterLayer {
        width: 21846,
        height: 3,
        name: "chunked",
        visible: true,
        opacity: 127,
        is_background: false,
        rgba: &rgba,
        metadata: &[],
    };
    let doc = RasterDocument {
        width: layer.width,
        height: layer.height,
        layers: std::slice::from_ref(&layer),
        saved_with: [0, 1, 0, 0],
        metadata: &[],
    };
    let count = Cell::new(0);
    let bytes = pdn::encode_document(&doc, &WriteLimits::default(), &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        pdn::decode_layers(&bytes, &pdn::DecodeLimits::default(), &|| Ok(()))
            .unwrap()
            .layers[0]
            .rgba,
        rgba
    );
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let result = pdn::encode_document(&doc, &WriteLimits::default(), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
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
        pdn::write_document_to(&doc, &WriteLimits::default(), &mut Failed, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::StorageFailed
    );
    assert_eq!(rgba, source);
}
#[test]
fn export_owned_qualification_vectors_only_when_requested() {
    let Some(folder) = std::env::var_os("PAINTCRAFT_PDN_WRITER_OUTPUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    assert!(folder.is_dir());
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
    for count in [1usize, 2, 64] {
        let pixels: Vec<Vec<u8>> = (0..count)
            .map(|layer| (0..24).map(|i| (i * 37 + layer * 53) as u8).collect())
            .collect();
        let names: Vec<_> = (0..count).map(|i| format!("Layer {i} 日本語🙂")).collect();
        let layers: Vec<_> = (0..count)
            .map(|i| RasterLayer {
                width: 3,
                height: 2,
                name: &names[i],
                visible: i % 2 == 0,
                opacity: (i * 127 + 1) as u8,
                is_background: i == 0,
                rgba: &pixels[i],
                metadata: &[],
            })
            .collect();
        let doc = RasterDocument {
            width: 3,
            height: 2,
            layers: &layers,
            saved_with: [0, 1, 0, 0],
            metadata: &[],
        };
        let bytes = pdn::encode_document(&doc, &WriteLimits::default(), &|| Ok(())).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.join(format!("layers-{count}.pdn")))
            .unwrap();
        file.write_all(&bytes).unwrap();
    }
}
#[test]
fn incompressible_multi_chunk_output_stays_bounded_and_decodes_exactly() {
    let mut state = 0x9861_3cf5u32;
    let rgba: Vec<_> = (0..1024 * 257 * 4)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    let layer = RasterLayer {
        width: 1024,
        height: 257,
        name: "incompressible",
        visible: true,
        opacity: 255,
        is_background: false,
        rgba: &rgba,
        metadata: &[],
    };
    let doc = RasterDocument {
        width: 1024,
        height: 257,
        layers: std::slice::from_ref(&layer),
        saved_with: [0, 1, 0, 0],
        metadata: &[],
    };
    let limits = WriteLimits::default();
    let plan = pdn::preflight_write(&doc, &limits, &|| Ok(())).unwrap();
    let encoded = pdn::encode_document(&doc, &limits, &|| Ok(())).unwrap();
    assert!(encoded.len() > rgba.len());
    assert!(encoded.len() < plan.output_capacity);
    assert_eq!(
        pdn::decode_layers(&encoded, &pdn::DecodeLimits::default(), &|| Ok(()))
            .unwrap()
            .layers[0]
            .rgba,
        rgba
    );
}
