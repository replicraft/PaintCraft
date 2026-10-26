use super::*;
use crate::precision::{ColorEvidence, ColorMetadata, LinearSpace, SourceAlpha};
use paintcraft_core::types::json;
use std::cell::Cell;
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/exr-fixtures/");
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{ROOT}{name}")).unwrap()
}
fn raw_attributes(bytes: &[u8]) -> (usize, Vec<(String, usize, usize, usize)>) {
    let mut input = Bytes { bytes, at: 8 };
    let mut fields = Vec::new();
    loop {
        let start = input.at;
        let name = input.text().unwrap();
        if name.is_empty() {
            return (input.at, fields);
        }
        input.text().unwrap();
        let size = input.i32().unwrap() as usize;
        let offset = input.at;
        input.take(size).unwrap();
        fields.push((name.to_owned(), start, offset, size));
    }
}
fn mutate(name: &str, change: impl FnOnce(&mut [u8])) -> Vec<u8> {
    let mut bytes = fixture("half-rgb-none.exr");
    let (_, fields) = raw_attributes(&bytes);
    let (_, _, offset, len) = fields.iter().find(|a| a.0 == name).unwrap();
    change(&mut bytes[*offset..*offset + *len]);
    bytes
}
#[test]
fn official_openexr_owned_headers_chunks_color_origin_and_alpha_admit() {
    let manifest = json::parse(&fixture("manifest.json")).unwrap();
    for case in manifest.get("fixtures").unwrap().as_array().unwrap() {
        let name = case.get("file").unwrap().as_str().unwrap();
        let source = fixture(name);
        let before = source.clone();
        let result =
            inspect(&source, PIXELS_MAX, &|| Ok(())).unwrap_or_else(|e| panic!("{name}: {e}"));
        let header = result.header();
        assert_eq!(
            i64::from(header.width),
            case.get("width").unwrap().as_i64().unwrap()
        );
        assert_eq!(
            i64::from(header.height),
            case.get("height").unwrap().as_i64().unwrap()
        );
        let origin = case.get("origin").unwrap().as_array().unwrap();
        assert_eq!(
            header.origin,
            [
                origin[0].as_i64().unwrap() as i32,
                origin[1].as_i64().unwrap() as i32
            ]
        );
        assert_eq!(
            header.source_alpha(),
            if case.get("alpha").unwrap().as_bool().unwrap() {
                SourceAlpha::Premultiplied
            } else {
                SourceAlpha::Opaque
            }
        );
        assert_eq!(
            result.chunk_count(),
            if header.compression == 3 {
                header.height.div_ceil(16)
            } else {
                header.height
            }
        );
        let digest: [u8; 32] = Sha256::digest(&source).into();
        assert_eq!(result.sha256(), digest);
        assert_eq!(result.source_len(), source.len());
        assert_eq!(source, before);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, case.get("sha256").unwrap().as_str().unwrap());
        if name.starts_with("unlabelled")
            || name.starts_with("unknown-")
            || name.starts_with("conflicting-")
        {
            assert_eq!(header.color_metadata(), ColorMetadata::Unspecified);
        } else {
            assert_eq!(
                header.color_metadata(),
                ColorMetadata::Declared {
                    space: LinearSpace::Rec709D65,
                    evidence: if name.starts_with("interop-") {
                        ColorEvidence::InteropId
                    } else {
                        ColorEvidence::Chromaticities
                    }
                }
            );
        }
        let reference = fixture(&name.replace(".exr", ".f32le"));
        assert_eq!(
            reference.len() as u64,
            u64::from(header.width) * u64::from(header.height) * header.channels.len() as u64 * 4
        );
        let reference_hash: String = Sha256::digest(&reference)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            reference_hash,
            case.get("reference_sha256").unwrap().as_str().unwrap()
        );
        assert!(header.attributes.iter().any(|a| a.name == "comments"));
    }
}
#[test]
fn header_channel_window_extent_and_feature_refusals_precede_decoder() {
    for source in [
        mutate("compression", |v| v[0] = 4),
        mutate("lineOrder", |v| v[0] = 1),
        mutate("dataWindow", |v| {
            v[..4].copy_from_slice(&i32::MIN.to_le_bytes())
        }),
        mutate("pixelAspectRatio", |v| {
            v.copy_from_slice(&f32::NAN.to_le_bytes())
        }),
        mutate("channels", |v| v[0] = b'Z'),
        mutate("channels", |v| v[2..6].copy_from_slice(&0u32.to_le_bytes())),
        mutate("channels", |v| {
            v[10..14].copy_from_slice(&2i32.to_le_bytes())
        }),
    ] {
        assert!(inspect(&source, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let source = fixture("half-rgb-none.exr");
    for flags in [0, 1, 2 | 0x200, 2 | 0x400, 2 | 0x800, 2 | 0x1000] {
        let mut changed = source.clone();
        changed[4..8].copy_from_slice(&(flags as u32).to_le_bytes());
        assert!(inspect(&changed, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let (end, fields) = raw_attributes(&source);
    let (_, start, offset, len) = fields.iter().find(|a| a.0 == "comments").unwrap();
    let mut duplicate = source.clone();
    duplicate.splice(
        end - 1..end - 1,
        source[*start..*offset + *len].iter().copied(),
    );
    assert_eq!(
        inspect(&duplicate, PIXELS_MAX, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    let mut huge = source.clone();
    huge[*offset - 4..*offset].copy_from_slice(&i32::MAX.to_le_bytes());
    assert_eq!(
        inspect(&huge, PIXELS_MAX, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        inspect(&source, 1, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn hostile_offsets_chunks_prefixes_and_cancel_never_publish_inspection() {
    let source = fixture("half-rgb-none.exr");
    let (table, _) = raw_attributes(&source);
    let offset = u64::from_le_bytes(source[table..table + 8].try_into().unwrap()) as usize;
    for value in [0, u64::MAX, offset as u64 + 1] {
        let mut changed = source.clone();
        changed[table..table + 8].copy_from_slice(&value.to_le_bytes());
        assert!(inspect(&changed, PIXELS_MAX, &|| Ok(())).is_err());
    }
    for (position, value) in [(offset, 999i32), (offset + 4, -1), (offset + 4, i32::MAX)] {
        let mut changed = source.clone();
        changed[position..position + 4].copy_from_slice(&value.to_le_bytes());
        assert!(inspect(&changed, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let mut overlap = source.clone();
    overlap[table + 8..table + 16].copy_from_slice(&(offset as u64).to_le_bytes());
    assert!(inspect(&overlap, PIXELS_MAX, &|| Ok(())).is_err());
    for length in 0..source.len() {
        assert!(
            inspect(&source[..length], PIXELS_MAX, &|| Ok(())).is_err(),
            "prefix {length}"
        );
    }
    let calls = Cell::new(0);
    let error = inspect(&source, PIXELS_MAX, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 7 {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel metadata"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let mut trailing = source.clone();
    trailing.push(0);
    assert_eq!(
        inspect(&trailing, PIXELS_MAX, &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn explicit_input_attribute_count_name_and_signed_window_limits() {
    let source = fixture("half-rgb-none.exr");
    let (end, _) = raw_attributes(&source);
    let mut fields = Vec::new();
    for i in 0..ATTRIBUTES_MAX {
        fields.extend_from_slice(format!("extra{i}\0string\0").as_bytes());
        fields.extend_from_slice(&0i32.to_le_bytes());
    }
    let mut excessive = source.clone();
    excessive.splice(end - 1..end - 1, fields);
    assert_eq!(
        inspect(&excessive, PIXELS_MAX, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut long = source.clone();
    long.splice(end - 1..end - 1, [b'a'; 33]);
    assert_eq!(
        inspect(&long, PIXELS_MAX, &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let (_, _, offset, len) = raw_attributes(&source)
        .1
        .into_iter()
        .find(|a| a.0 == "comments")
        .unwrap();
    let growth = HEADER_BYTES_MAX + 1 - end;
    let mut terminator = source.clone();
    terminator[offset - 4..offset].copy_from_slice(&((len + growth) as i32).to_le_bytes());
    terminator.splice(offset + len..offset + len, vec![b'x'; growth]);
    assert_eq!(
        inspect(&terminator, PIXELS_MAX, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let huge = vec![0; INPUT_BYTES_MAX + 1];
    assert_eq!(
        inspect(&huge, PIXELS_MAX, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    // Equal windows still cannot smuggle i32 subtraction overflow or an
    // unbounded display canvas through a tiny stored raster.
    let mut overflow = source.clone();
    let (_, fields) = raw_attributes(&overflow);
    for name in ["dataWindow", "displayWindow"] {
        let (_, _, offset, _) = fields.iter().find(|a| a.0 == name).unwrap();
        overflow[*offset..*offset + 16].copy_from_slice(
            &[i32::MIN, 0, i32::MAX, 1]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(
        inspect(&overflow, PIXELS_MAX, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
