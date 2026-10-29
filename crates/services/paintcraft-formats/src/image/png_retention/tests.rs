use super::*;
use paintcraft_core::types::{ErrorCode, json, policy::hex, resources::SharedBudget};
use paintcraft_document::source_metadata::{SourceColorModel, WorkingColor};
use sha2::{Digest, Sha256};
use std::{cell::Cell, path::Path};
const PIXELS: u64 = 16384;
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/png-retention-fixtures")
            .join(name),
    )
    .unwrap()
}
fn prepared(
    bytes: &[u8],
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    decode(
        bytes,
        PIXELS,
        crate::pcnd::MetadataAdmission {
            bytes: paintcraft_document::source_metadata::BYTES_MAX,
            budget,
        },
        check,
    )
}
fn pinned_original_samples(bytes: &[u8]) -> Decoded {
    let decoder = ::image::codecs::png::PngDecoder::with_limits(
        std::io::Cursor::new(bytes),
        super::super::decode_limits(PIXELS),
    )
    .unwrap();
    super::super::decode_pixels(decoder, ImageFormat::Png, vec![], PIXELS).unwrap()
}

#[test]
fn all_independent_models_depths_transparency_adam7_and_palette_match_legacy_samples() {
    let manifest = json::parse(&fixture("oracle.json")).unwrap();
    let mut cases = 0;
    for row in manifest.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let original = bytes.clone();
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let budget = SharedBudget::new(1 << 20);
        let captured =
            prepared(&bytes, &budget, &|| Ok(())).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected = fixture(&format!("{name}.rgba"));
        assert_eq!(
            hex(&Sha256::digest(&captured.image.rgba)),
            hex(&Sha256::digest(&expected)),
            "{name}"
        );
        let old = pinned_original_samples(&bytes);
        assert_eq!(old.rgba, captured.image.rgba, "legacy {name}");
        let icc = row.get("icc").unwrap().as_bool().unwrap();
        let phys = row.get("physical").unwrap().as_bool().unwrap();
        assert_eq!(captured.attachment.is_some(), icc || phys);
        assert_eq!(
            captured
                .image
                .losses
                .iter()
                .any(|s| s == ICC_RETAINED_UNAPPLIED),
            icc
        );
        assert_eq!(
            captured
                .image
                .losses
                .iter()
                .any(|s| s == RESOLUTION_RETAINED_INACTIVE),
            phys
        );
        assert!(
            !captured
                .image
                .losses
                .iter()
                .any(|s| s == "icc-profile-ignored" || s == "resolution-metadata-ignored")
        );
        if let Some(meta) = &captured.attachment {
            assert_eq!(
                meta.descriptor().source_sha256,
                <[u8; 32]>::from(Sha256::digest(&bytes))
            );
            assert_eq!(meta.descriptor().source_bytes, bytes.len() as u64);
            assert_eq!(meta.working_color(), WorkingColor::UnmanagedRgba8);
            assert_eq!(meta.descriptor().orientation, None);
            if icc {
                assert_eq!(
                    hex(&Sha256::digest(meta.profile().unwrap())),
                    row.get("profile_sha256").unwrap().as_str().unwrap()
                );
                assert!(
                    captured
                        .image
                        .losses
                        .iter()
                        .any(|s| s == "png-metadata-ignored")
                );
            }
            let model = row.get("model").unwrap().as_i64().unwrap();
            assert_eq!(
                meta.descriptor().source_model,
                match model {
                    0 | 4 => SourceColorModel::Gray,
                    3 => SourceColorModel::Indexed,
                    _ => SourceColorModel::Rgb,
                }
            );
            if phys {
                let r = meta.descriptor().resolution.unwrap();
                assert_eq!((r.horizontal.numerator, r.vertical.numerator), (3780, 3779));
                assert_eq!((r.horizontal.denominator, r.vertical.denominator), (1, 1));
                assert_eq!(r.raw_unit, Some(1));
            }
            assert_eq!(budget.usage().reserved, meta.storage_usage().1);
        }
        let doc = captured
            .document_with_admission(&mut paintcraft_raster::Admission::new(65536), &|| Ok(()))
            .unwrap();
        let back = crate::pcnd::read(&crate::pcnd::write_compact(&doc).unwrap()).unwrap();
        assert!(back.same_semantics(&doc));
        assert_eq!(
            prepared(&bytes, &SharedBudget::new(1 << 20), &|| Ok(()))
                .unwrap()
                .document_key,
            captured.document_key
        );
        assert_eq!(bytes, original);
        drop(doc);
        drop(captured);
        assert_eq!(budget.usage().reserved, 0);
        cases += 1;
    }
    assert_eq!(cases, 27);
}

#[test]
fn no_retained_fields_need_no_retention_budget_but_omissions_still_need_review() {
    let bytes = fixture("rgba8-omissions-only.png");
    let zero = SharedBudget::new(0);
    let decoded = prepared(&bytes, &zero, &|| Ok(())).unwrap();
    assert!(decoded.attachment.is_none());
    assert!(
        decoded
            .image
            .losses
            .iter()
            .any(|s| s == "exif-metadata-ignored")
    );
    assert!(
        decoded
            .image
            .losses
            .iter()
            .any(|s| s == "png-metadata-ignored")
    );
    assert_eq!(zero.usage().peak, 0);
    let with = fixture("rgba8.png");
    assert_eq!(
        prepared(&with, &zero, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn cancel_every_checkpoint_and_shared_realm_exhaustion_never_leak_metadata() {
    let bytes = fixture("rgba16-adam7.png");
    let budget = SharedBudget::new(1 << 20);
    let calls = Cell::new(0);
    drop(
        prepared(&bytes, &budget, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for stop in 0..calls.get() {
        let count = Cell::new(0);
        let result = prepared(&bytes, &budget, &|| {
            let at = count.get();
            count.set(at + 1);
            if at == stop {
                Err(paintcraft_core::types::Error::new(
                    ErrorCode::OperationCancelled,
                    "checkpoint cancel",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "checkpoint{stop}"
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let first = prepared(&bytes, &budget, &|| Ok(())).unwrap();
    let charge = budget.usage().reserved;
    let exact = SharedBudget::new(charge);
    let retained = prepared(&bytes, &exact, &|| Ok(())).unwrap();
    assert_eq!(
        prepared(&bytes, &exact, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(retained);
    assert_eq!(exact.usage().reserved, 0);
    drop(first);
    assert_eq!(budget.usage().reserved, 0);
}

fn mutate_chunk(bytes: &[u8], kind: &[u8; 4], mut change: impl FnMut(&mut [u8])) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    let mut at = 8;
    loop {
        let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        if &bytes[at + 4..at + 8] == kind {
            change(&mut bytes[at + 8..at + 8 + len]);
            let crc = crc32fast::hash(&bytes[at + 4..at + 8 + len]);
            bytes[at + 8 + len..at + 12 + len].copy_from_slice(&crc.to_be_bytes());
            return bytes;
        }
        at += 12 + len;
    }
}
#[test]
fn bad_source_crc_and_icc_adler_refuse_early_while_pixel_adler_drops_captured_profile() {
    let bytes = fixture("rgba8.png");
    let budget = SharedBudget::new(1 << 20);
    let mut crc = bytes.clone();
    crc[29] ^= 1;
    assert!(prepared(&crc, &budget, &|| Ok(())).is_err());
    assert_eq!(budget.usage().peak, 0);
    let bad = mutate_chunk(&bytes, b"iCCP", |data| {
        let n = data.len();
        data[n - 1] ^= 1;
    });
    assert!(prepared(&bad, &budget, &|| Ok(())).is_err());
    assert_eq!(budget.usage().peak, 0);
    let bad = mutate_chunk(&bytes, b"IDAT", |data| {
        let n = data.len();
        data[n - 1] ^= 1;
    });
    assert!(super::super::decode_with_pixel_limit(&bad, PIXELS).is_err());
    assert!(prepared(&bad, &budget, &|| Ok(())).is_err());
    assert!(budget.usage().peak > 0);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn trailing_source_bytes_and_zlib_payloads_refuse_without_retention_leaks() {
    let bytes = fixture("rgba8.png");
    let realm = SharedBudget::new(1 << 20);
    let mut trailing = bytes.clone();
    trailing.push(0);
    for source in [&trailing, &bytes[..bytes.len() - 1].to_vec()] {
        assert!(prepared(source, &realm, &|| Ok(())).is_err());
        assert!(super::super::decode_with_pixel_limit(source, PIXELS).is_err());
        assert_eq!(realm.usage().reserved, 0);
    }
}
