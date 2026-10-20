use super::*;
use crate::archive::{Blob, payload::corrupt_compressed_for_test};
use paintcraft_core::types::{Error, ErrorCode, Rgba};
use paintcraft_raster::{Admission, TileLease};

fn pixels() -> Vec<u8> {
    (0..65536)
        .flat_map(|i| {
            let (x, y) = ((i % 256) as u8, (i / 256) as u8);
            [x / 8, y / 8, (x / 16) ^ (y / 16), (x % 4) * 85]
        })
        .collect()
}
fn lease(pixels: &[u8]) -> TileLease {
    TileLease::materialize(Rgba::TRANSPARENT, &mut Admission::new(65536), |out| {
        out.copy_from_slice(pixels);
        Ok(())
    })
    .unwrap()
}
fn pack(pixels: &[u8], codec: Compression) -> Blob {
    let mut encoder = Encoder::new(codec);
    Blob::pack(
        lease(pixels),
        Rgba::TRANSPARENT,
        &mut (16 << 20),
        &|| Ok(()),
        &mut encoder,
    )
    .unwrap()
}

#[test]
fn independent_codec_blocks_release_sources_and_restore_all_rgba_bytes() {
    let pixels = pixels();
    for codec in [Compression::Lz4, Compression::Zstd1, Compression::Zstd3] {
        let blob = pack(&pixels, codec);
        assert!(blob.compressed_bytes() > 0 && blob.compressed_bytes() < pixels.len());
        assert_eq!(blob.source().resident_bytes(), 0);
        assert_eq!(
            blob.restore(&mut Admission::new(0)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        let restored = blob.restore(&mut Admission::new(65536)).unwrap();
        assert_eq!(restored.bytes(), pixels);
        let reused = blob.restore(&mut Admission::new(0)).unwrap();
        assert_eq!(reused.version(), restored.version());
        drop((restored, reused));
        assert_eq!(blob.source().resident_bytes(), 0);
        assert_eq!(
            blob.restore(&mut Admission::new(65536)).unwrap().bytes(),
            pixels
        );
    }
}
#[test]
fn corrupt_blocks_are_rejected_without_caching_failed_decodes() {
    for codec in [Compression::Lz4, Compression::Zstd1, Compression::Zstd3] {
        let mut blob = pack(&pixels(), codec);
        corrupt_compressed_for_test(&mut blob);
        assert_eq!(
            blob.restore(&mut Admission::new(65536)).unwrap_err().code,
            ErrorCode::CodecFailed
        );
        assert_eq!(blob.source().resident_bytes(), 0);
    }
}
#[test]
fn zstd_requires_exactly_one_frame_with_exact_bounded_output() {
    let pixels = pixels();
    let encoded = zstd::bulk::compress(&pixels, 1).unwrap();
    let mut out = vec![0; pixels.len()];
    for bad in [
        encoded[..encoded.len() - 1].to_vec(),
        [encoded.clone(), encoded.clone()].concat(),
        zstd::bulk::compress(&pixels[..128], 1).unwrap(),
        [encoded, vec![0]].concat(),
    ] {
        assert_eq!(
            decode(Compression::Zstd1, &bad, &mut out).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    assert_eq!(
        decode(Compression::None, &[1], &mut out).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        decode(Compression::Lz4, &[255], &mut out).unwrap_err().code,
        ErrorCode::CodecFailed
    );
}
#[test]
fn incompressible_and_small_payloads_retain_cheaper_recorded_representations() {
    let mut seed = 42u32;
    let noise: Vec<_> = (0..65536)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed.to_le_bytes()
        })
        .collect();
    let mut patch = vec![0; RasterLayer::TILE_BYTES];
    patch[0] = 7;
    for codec in [Compression::Lz4, Compression::Zstd1, Compression::Zstd3] {
        let blob = pack(&noise, codec);
        assert_eq!(blob.encoded_bytes(), 0);
        assert_eq!(blob.source().resident_bytes(), RasterLayer::TILE_BYTES);
        assert_eq!(blob.restore(&mut Admission::new(0)).unwrap().bytes(), noise);
        let blob = pack(&patch, codec);
        assert_eq!(blob.compressed_bytes(), 0);
        assert_eq!(blob.encoded_bytes(), 4);
        assert_eq!(
            blob.restore(&mut Admission::new(65536)).unwrap().bytes(),
            patch
        );
    }
}
#[test]
fn codec_staging_and_cancellation_are_admitted_before_retention() {
    for codec in [Compression::Lz4, Compression::Zstd1, Compression::Zstd3] {
        let mut encoder = Encoder::new(codec);
        assert_eq!(
            encoder
                .encode(&pixels(), usize::MAX, &mut 0, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(encoder.scratch_bytes(), 0);
        assert_eq!(
            encoder
                .encode(&pixels(), usize::MAX, &mut (16 << 20), &|| Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test"
                )))
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(encoder.scratch_bytes(), 0);
    }
}

#[test]
fn solid_payloads_bypass_codec_workspace_and_keep_hidden_color() {
    for color in [
        Rgba([0, 0, 0, 0]),
        Rgba([37, 19, 83, 0]),
        Rgba([1, 2, 3, 255]),
    ] {
        let pixels: Vec<u8> = (0..65536).flat_map(|_| color.0).collect();
        for codec in [Compression::Lz4, Compression::Zstd1, Compression::Zstd3] {
            let mut encoder = Encoder::new(codec);
            let blob = Blob::pack(
                lease(&pixels),
                Rgba::TRANSPARENT,
                &mut 1024,
                &|| Ok(()),
                &mut encoder,
            )
            .unwrap();
            assert_eq!(encoder.scratch_bytes(), 0);
            assert_eq!(encoder.workspace_peak, 0);
            assert_eq!(blob.encoded_bytes(), 0);
            assert_eq!(blob.source().resident_bytes(), 0);
            assert_eq!(
                blob.restore(&mut Admission::new(65536)).unwrap().bytes(),
                pixels
            );
        }
    }
}

#[test]
fn cancellation_after_encoding_does_not_return_a_payload() {
    use std::cell::Cell;
    for codec in [Compression::Lz4, Compression::Zstd1, Compression::Zstd3] {
        let checks = Cell::new(0);
        let mut encoder = Encoder::new(codec);
        let error = encoder
            .encode(&pixels(), usize::MAX, &mut (16 << 20), &|| {
                checks.set(checks.get() + 1);
                if checks.get() == 2 {
                    Err(Error::new(ErrorCode::OperationCancelled, "after encode"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(checks.get(), 2);
        assert!(encoder.scratch_bytes() > 0);
    }
}
