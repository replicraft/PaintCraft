//! Exact nonprivate V8 contract; optional producer until final promotion.
use super::*;
pub const SCHEMA_VERSION: u16 = 3;
pub const NATIVE_PROFILE: &str = "native-v8-jxr-rgb8-lossless-seekcap2-jxrread1-parent-lease1";
pub const BUILD_RECORD_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_RGB8\0";
/// Compile-time worker record; the versioned builder is checked against it.
pub const BUILD_RECORD: &[u8] = concat!(
    "PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_RGB8\0",
    env!("CARGO_PKG_VERSION"),
    "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576\0PCENC001\0PCJXR001\0JXR_RGB8_LOSSLESS_SEEK_CAP2_INPUT3145728_PIXELS1048576_OUTPUT67108864\0JXR_WRITER_RGB8_READ1_INPUT67108864_PIXELS1048576_SIDES65536\0END_CODEC_WORKER_BUILD\0"
).as_bytes();

pub const EXPORT_VARIANT: &str = "native-jxr-rgb8-lossless-worker-v1";
pub const EXPORT_VARIANTS: &[&str] = &[EXPORT_VARIANT];
pub const WRITER_IMPORT_VARIANT: &str = "native-jxr-writer-rgb8-worker-v1";
pub const IMPORT_VARIANTS: &[&str] = &[
    super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS[0],
    super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS[1],
    super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS[2],
    super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS[3],
    super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS[4],
    super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS[5],
    super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS[6],
    WRITER_IMPORT_VARIANT,
];
/// The optional worker feature must include float-codecs before declaring these
/// eight imports. The default worker still emits its unchanged V7 record.
pub fn build_record(version: &str) -> Result<Vec<u8>> {
    const END: &[u8] = b"END_CODEC_WORKER_BUILD\0";
    let base = super::super::native_float_build_record(version)?;
    let mut record = BUILD_RECORD_MARKER.to_vec();
    record.extend_from_slice(
        &base[super::super::FLOAT_BUILD_RECORD_MARKER.len()..base.len() - END.len()],
    );
    record.extend_from_slice(b"PCENC001\0PCJXR001\0JXR_RGB8_LOSSLESS_SEEK_CAP2_INPUT3145728_PIXELS1048576_OUTPUT67108864\0");
    record.extend_from_slice(b"JXR_WRITER_RGB8_READ1_INPUT67108864_PIXELS1048576_SIDES65536\0");
    record.extend_from_slice(END);
    Ok(record)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualified_record_is_distinct_from_private_and_binds_all_semantics() {
        let record = build_record(env!("CARGO_PKG_VERSION")).unwrap();
        assert!(record.starts_with(BUILD_RECORD_MARKER));
        assert_eq!(record, BUILD_RECORD);
        assert!(record.len() <= 512);
        let maximum_version = build_record(&"v".repeat(64)).unwrap();
        assert!(maximum_version.len() <= 512);
        assert_ne!(
            BUILD_RECORD_MARKER,
            super::super::candidate::BUILD_RECORD_MARKER
        );
        assert_ne!(NATIVE_PROFILE, super::super::candidate::NATIVE_PROFILE);
        assert_eq!(
            &IMPORT_VARIANTS[..7],
            super::super::super::QUALIFIED_FLOAT_IMPORT_VARIANTS
        );
        assert_eq!(IMPORT_VARIANTS.len(), 8);
        assert_eq!(IMPORT_VARIANTS[7], WRITER_IMPORT_VARIANT);
        assert_eq!(EXPORT_VARIANTS, [EXPORT_VARIANT]);
        for token in [
            b"PCDEC002".as_slice(),
            b"FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576",
            b"JXR_WRITER_RGB8_READ1_INPUT67108864_PIXELS1048576_SIDES65536",
            b"PCENC001",
            b"PCJXR001",
            b"JXR_RGB8_LOSSLESS_SEEK_CAP2_INPUT3145728_PIXELS1048576_OUTPUT67108864",
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
