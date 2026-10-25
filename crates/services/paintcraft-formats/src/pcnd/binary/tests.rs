use super::super::read as open;
use super::*;
use paintcraft_document::new_document;
use paintcraft_raster::BlendMode;

fn document() -> Document {
    let mut doc = new_document(257, 259, Some(Rgba([11, 22, 33, 0])), 0, POLICY).unwrap();
    doc.title = Some("日本語 Español हिन्दी".into());
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.name = "透明なレイヤー".into();
    layer.origin = (-103, 27);
    layer.visible = false;
    layer.opacity = 92;
    layer
        .raster
        .blend_pixel(
            256,
            258,
            Rgba([42, 43, 44, 0]),
            BlendMode::Replace,
            &mut Admission::new(STAGED_PIXELS_MAX),
        )
        .unwrap();
    doc
}
fn seal(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes.truncate(bytes.len() - 32);
    let hash = Sha256::digest(&bytes);
    bytes.extend_from_slice(&hash);
    bytes
}
fn first_block_offset(bytes: &[u8]) -> usize {
    let body = payload(bytes, &|| Ok(())).unwrap();
    let mut input = Input::new(body);
    input.take(32).unwrap();
    input.dimension().unwrap();
    input.dimension().unwrap();
    if input.byte().unwrap() == 1 {
        input.string(4096).unwrap();
    }
    input.number(LAYER_COUNT_MAX as u64).unwrap();
    input.id().unwrap();
    input.string(1024).unwrap();
    input.take(2).unwrap();
    input.signed().unwrap();
    input.signed().unwrap();
    input.dimension().unwrap();
    input.dimension().unwrap();
    input.take(4).unwrap();
    MAGIC.len() + body.len() - input.remaining()
}

#[test]
fn binary_roundtrip_preserves_every_field_and_detects_every_single_byte_change() {
    let doc = document();
    let bytes = write_compact(&doc).unwrap();
    assert_eq!(probe(&bytes).unwrap().container_version, 2);
    probe(&bytes).unwrap().require_supported().unwrap();
    assert_eq!(
        super::super::write(&open(&bytes).unwrap()).unwrap(),
        super::super::write(&doc).unwrap()
    );
    for index in 0..bytes.len() {
        let mut damaged = bytes.clone();
        damaged[index] ^= 1;
        assert!(open(&damaged).is_err(), "byte {index}");
        assert!(open(&bytes[..index]).is_err(), "truncated at {index}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(open(&trailing).is_err());
    let mut trailing_body = bytes.clone();
    trailing_body.insert(bytes.len() - 32, 0);
    assert!(open(&seal(trailing_body)).is_err());
}

#[test]
fn forged_decoder_payload_is_admitted_before_inflation_and_unknown_tags_are_rejected() {
    let mut doc = new_document(256, 256, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    doc.layers
        .get_mut(&doc.roots[0])
        .unwrap()
        .raster
        .blend_pixel(
            20,
            20,
            Rgba([0, 0, 0, 255]),
            BlendMode::Replace,
            &mut Admission::new(STAGED_PIXELS_MAX),
        )
        .unwrap();
    let bytes = write_compact(&doc).unwrap();
    let tag = first_block_offset(&bytes);
    let mut damaged = bytes.clone();
    damaged[tag] = 3; // RGBA runs are not a Zstandard frame.
    let damaged = seal(damaged);
    assert_eq!(
        read_with_limits(&damaged, u64::MAX, 0).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(open(&damaged).unwrap_err().code, ErrorCode::CodecFailed);
    let mut damaged = bytes.clone();
    damaged[tag] = 255;
    assert!(open(&seal(damaged)).is_err());
    assert!(read_with_limits(&bytes, 1024, STAGED_PIXELS_MAX).is_err());
}

#[test]
fn wire_rejects_oversized_noncanonical_and_truncated_lengths_without_allocation() {
    for bytes in [&[0x80, 0][..], &[0xff; 10], &[0x80], &[0x81, 0x80, 0]] {
        assert!(Input::new(bytes).number(u64::MAX).is_err(), "{bytes:?}");
    }
    assert!(Input::new(&[0]).dimension().is_err());
    assert!(Input::new(&[0xff, 0xff, 0x7f]).string(4096).is_err());
    assert!(Input::new(&[2, 0xff, 0xff]).string(4096).is_err());
    let mut buffer = Vec::new();
    let mut output = Output::new(&mut buffer);
    for value in [0, 1, 127, 128, 16_384, u64::MAX] {
        output.number(value).unwrap();
    }
    output.finish().unwrap();
    let mut input = Input::new(&buffer[..buffer.len() - 32]);
    for value in [0, 1, 127, 128, 16_384, u64::MAX] {
        assert_eq!(input.number(u64::MAX).unwrap(), value);
    }
}

#[test]
fn binary_writer_handles_partial_writes_and_never_reports_a_failed_sink_as_saved() {
    struct Sink {
        bytes: Vec<u8>,
        remaining: usize,
    }
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.remaining == 0 {
                return Err(std::io::Error::other("qualification sink failure"));
            }
            let count = bytes.len().min(7).min(self.remaining);
            self.bytes.extend_from_slice(&bytes[..count]);
            self.remaining -= count;
            Ok(count)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let doc = document();
    let expected = write_compact(&doc).unwrap();
    let mut sink = Sink {
        bytes: Vec::new(),
        remaining: usize::MAX,
    };
    write_compact_to(&doc, &mut sink).unwrap();
    assert_eq!(sink.bytes, expected);
    for remaining in 0..expected.len() {
        let mut sink = Sink {
            bytes: Vec::new(),
            remaining,
        };
        assert!(write_compact_to(&doc, &mut sink).is_err());
    }
}

#[test]
fn portable_encoder_reads_identically_and_compresses_run_descriptors() {
    let mut doc = new_document(1000, 486, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    let mut admission = Admission::new(STAGED_PIXELS_MAX);
    for x in 30..950 {
        let y = 200 + ((x as f64 / 35.0).sin() * 70.0) as i64;
        layer
            .raster
            .blend_pixel(
                x,
                y,
                Rgba([0, 0, 0, 255]),
                BlendMode::Replace,
                &mut admission,
            )
            .unwrap();
    }
    let mut portable = Vec::new();
    write_encoded(
        &doc,
        &mut portable,
        &|| Ok(()),
        compression::Encoder::portable(),
    )
    .unwrap();
    let native = write_compact(&doc).unwrap();
    assert_eq!(
        super::super::write(&open(&portable).unwrap()).unwrap(),
        super::super::write(&doc).unwrap()
    );
    assert!(portable.len() < 5000);
    println!(
        "portable-doodle={} native-doodle={}",
        portable.len(),
        native.len()
    );
    for source in [
        vec![19; 262_144],
        (0..65_536u32).flat_map(u32::to_le_bytes).collect(),
    ] {
        let mut encoder = compression::Encoder::portable();
        let (encoding, stored) = encoder.block(&source).unwrap();
        let decoded = match encoding {
            "raw" => stored.to_vec(),
            "rgba-runs" => runs::decode(&stored, source.len(), &|| Ok(())).unwrap(),
            "lz4" | "runs-lz4" => {
                compression::decode_lz4(&stored, source.len(), encoding == "runs-lz4", &|| Ok(()))
                    .unwrap()
            }
            _ => panic!("portable writer used {encoding}"),
        };
        assert_eq!(decoded, source);
    }
    for bad in [vec![], vec![255; 4], vec![0, 0, 0, 0], vec![255; 16]] {
        assert!(compression::decode_lz4(&bad, 262_144, true, &|| Ok(())).is_err());
    }
}
