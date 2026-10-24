use super::*;
use paintcraft_document::{import_image, new_document};
use paintcraft_raster::BlendMode;
use std::cell::Cell;

fn chunks(bytes: &[u8]) -> usize {
    zip::read(bytes, ENTRIES_MAX, |_| Some(CHUNK_MAX))
        .unwrap()
        .iter()
        .filter(|(name, _)| name.starts_with("data/"))
        .map(|(_, data)| data.len())
        .sum()
}

fn painted(points: &[(i64, i64)]) -> Document {
    let mut doc = new_document(1000, 486, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let mut admission = Admission::new(STAGED_PIXELS_MAX);
    for &(x, y) in points {
        doc.layers
            .get_mut(&id)
            .unwrap()
            .raster
            .blend_pixel(
                x,
                y,
                Rgba([17, 34, 51, 0]),
                BlendMode::Replace,
                &mut admission,
            )
            .unwrap();
    }
    doc
}

#[test]
fn compact_storage_sizes_and_exact_roundtrip_for_blank_one_pixel_sparse_and_dense() {
    let sparse = (0..2)
        .flat_map(|y| (0..4).map(move |x| (x * 256 + 2, y * 256 + 3)))
        .collect::<Vec<_>>();
    let mut state = 0x1234ABCDu32;
    let dense: Vec<u8> = (0..1000 * 486)
        .flat_map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state.to_le_bytes()
        })
        .collect();
    for (name, doc) in [
        ("blank", painted(&[])),
        ("one-pixel", painted(&[(20, 20)])),
        ("sparse-eight-tiles", painted(&sparse)),
        (
            "dense-noise",
            import_image(1000, 486, &dense, "compact-noise").unwrap(),
        ),
    ] {
        let raw = write(&doc).unwrap();
        let compact = write_compact(&doc).unwrap();
        let restored = read(&compact).unwrap();
        assert!(restored.same_semantics(&doc), "{name}");
        assert_eq!(restored.document_id, doc.document_id);
        assert_eq!(restored.state_id, doc.state_id);
        assert_eq!(
            write(&restored).unwrap(),
            raw,
            "canonical output remains unchanged"
        );
        assert_eq!(
            write_compact(&doc).unwrap(),
            compact,
            "pinned encoder deterministic"
        );
        println!(
            "{name}: raw_file={} raw_chunks={} compact_file={} compact_chunks={}",
            raw.len(),
            chunks(&raw),
            compact.len(),
            chunks(&compact)
        );
        if name == "one-pixel" {
            assert_eq!(chunks(&raw), 262_144);
            assert!(chunks(&compact) < 128);
        }
        if name == "blank" {
            assert_eq!(compact, raw);
            assert_eq!(chunks(&compact), 0);
        }
        if name == "sparse-eight-tiles" {
            assert_eq!(chunks(&raw), 1000 * 486 * 4);
            assert!(chunks(&compact) < 1024);
        }
        if name == "dense-noise" {
            assert_eq!(compact, raw, "incompressible blocks fall back to raw");
        }
    }
}

#[test]
fn compact_partial_boundary_blocks_and_raw_fallback_preserve_hidden_rgb_and_origin() {
    let mut doc = new_document(257, 259, Some(Rgba([4, 5, 6, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.origin = (-19, 27);
    let mut admission = Admission::new(STAGED_PIXELS_MAX);
    for (x, y, pixel) in [(0, 0, [120, 30, 50, 0]), (256, 258, [9, 8, 7, 0])] {
        layer
            .raster
            .blend_pixel(x, y, Rgba(pixel), BlendMode::Replace, &mut admission)
            .unwrap();
    }
    let raw = write(&doc).unwrap();
    let compact = write_compact(&doc).unwrap();
    assert!(compact.len() < raw.len());
    let restored = read(&compact).unwrap();
    assert!(restored.same_semantics(&doc));
    assert_eq!(restored.layers[&id].raster.get(256, 258).0, [9, 8, 7, 0]);
    assert_eq!(write(&restored).unwrap(), raw);
    assert!(
        read_with_limits(&compact, u64::MAX, 0).is_err(),
        "decoded admission precedes block inflation"
    );
}

#[test]
fn cancellation_at_each_writer_and_reader_checkpoint_never_changes_source_or_returns_partial_doc() {
    let doc = painted(&[(2, 3), (258, 3), (770, 260)]);
    let before = write(&doc).unwrap();
    let calls = Cell::new(0usize);
    let mut compact = Vec::new();
    write_compact_to_checked(&doc, &mut compact, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for failure in 1..=calls.get() {
        let seen = Cell::new(0);
        let mut incomplete = Vec::new();
        let error = write_compact_to_checked(&doc, &mut incomplete, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == failure {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "stop compact save",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    calls.set(0);
    read_with_limits_checked(&compact, u64::MAX, STAGED_PIXELS_MAX, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for failure in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = read_with_limits_checked(&compact, u64::MAX, STAGED_PIXELS_MAX, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == failure {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "stop compact open",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    assert_eq!(write(&doc).unwrap(), before);
}

#[test]
fn corrupt_compressed_archive_is_rejected_and_decoded_quota_is_checked_before_inflation() {
    let doc = painted(&[(20, 20)]);
    let compact = write_compact(&doc).unwrap();
    let mut members = zip::read(&compact, ENTRIES_MAX, |_| Some(CHUNK_MAX)).unwrap();
    let (_, chunk) = members
        .iter_mut()
        .find(|(name, _)| name.starts_with("data/"))
        .unwrap();
    *chunk.last_mut().unwrap() ^= 1;
    // Rewrite ZIP CRC to isolate the block's Zstandard checksum, not ZIP corruption.
    let damaged = zip::write_stored(&members);
    assert_eq!(read(&damaged).unwrap_err().code, ErrorCode::CodecFailed);
    assert_eq!(
        read_with_limits(&damaged, u64::MAX, 0).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(read(&compact).unwrap().same_semantics(&doc));
}
