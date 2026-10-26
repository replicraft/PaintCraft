//! Exact private encoder candidate; never part of production decoder discovery.
use super::*;
pub const SCHEMA_VERSION: u16 = 3;
pub const NATIVE_PROFILE: &str = "private-native-v8-jxr-rgb8-lossless-seekcap1-parent-lease1";
pub const BUILD_RECORD_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_ENCODE_PRIVATE\0";
/// Compile-time worker record; the versioned builder is checked against it.
pub const BUILD_RECORD: &[u8] = concat!(
    "PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_ENCODE_PRIVATE\0",
    env!("CARGO_PKG_VERSION"),
    "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576\0PCENC001\0PCJXR001\0JXR_RGB8_LOSSLESS_SEEK_CAP1_INPUT3145728_PIXELS1048576_OUTPUT67108864\0END_CODEC_WORKER_BUILD\0"
).as_bytes();

pub const EXPORT_VARIANT: &str = "native-jxr-rgb8-lossless-worker-v1";
pub const EXPORT_VARIANTS: &[&str] = &[EXPORT_VARIANT];
pub const IMPORT_VARIANTS: &[&str] = super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS;
/// The optional worker feature must include float-codecs before declaring these
/// seven imports. The default worker still emits its unchanged V7 record.
pub fn build_record(version: &str) -> Result<Vec<u8>> {
    const END: &[u8] = b"END_CODEC_WORKER_BUILD\0";
    let base = super::super::native_float_build_record(version)?;
    let mut record = BUILD_RECORD_MARKER.to_vec();
    record.extend_from_slice(
        &base[super::super::FLOAT_BUILD_RECORD_MARKER.len()..base.len() - END.len()],
    );
    record.extend_from_slice(b"PCENC001\0PCJXR001\0JXR_RGB8_LOSSLESS_SEEK_CAP1_INPUT3145728_PIXELS1048576_OUTPUT67108864\0");
    record.extend_from_slice(END);
    Ok(record)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_record_keeps_decode_semantics_and_separate_exact_encoder_contract() {
        let record = build_record(env!("CARGO_PKG_VERSION")).unwrap();
        assert!(record.starts_with(BUILD_RECORD_MARKER));
        assert_eq!(record, BUILD_RECORD);
        assert!(record.len() <= 512);
        let maximum_version = build_record(&"v".repeat(64)).unwrap();
        assert_eq!(maximum_version.len(), 412);
        assert!(maximum_version.len() <= 512);
        assert_eq!(
            IMPORT_VARIANTS,
            super::super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS
        );
        assert_eq!(IMPORT_VARIANTS.len(), 7);
        assert_eq!(EXPORT_VARIANTS, [EXPORT_VARIANT]);
        for token in [
            b"PCDEC002".as_slice(),
            b"FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576",
            b"PCENC001",
            b"PCJXR001",
            b"JXR_RGB8_LOSSLESS_SEEK_CAP1_INPUT3145728_PIXELS1048576_OUTPUT67108864",
        ] {
            assert_eq!(
                record
                    .windows(token.len())
                    .filter(|part| *part == token)
                    .count(),
                1
            );
        }
        assert!(build_record("").is_err());
        assert!(build_record(&"x".repeat(65)).is_err());
    }
}
