//! Two finite opt-in inventories; existing V7/V8 profiles remain unchanged.
use super::super::*;
pub const IMPORT_VARIANT: &str = "native-jxl-integer-srgb8-worker-v1";
pub mod base {
    use super::*;
    pub const NATIVE_PROFILE: &str = "native-v9-jxl-integer8-16-srgb8-parent-lease1";
    pub const BUILD_RECORD_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V9_JXL_INTEGER\0";
    pub const BUILD_RECORD: &[u8] = concat!("PAINTCRAFT_CODEC_WORKER_BUILD_V9_JXL_INTEGER\0", env!("CARGO_PKG_VERSION"), "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576\0JXL_INTEGER8_16_RGBA8_ROUND257_REPORT96_INPUT67108864_PIXELS4194304\0END_CODEC_WORKER_BUILD\0").as_bytes();
    pub const IMPORT_VARIANTS: &[&str] = &[
        QUALIFIED_FLOAT_IMPORT_VARIANTS[0],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[1],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[2],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[3],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[4],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[5],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[6],
        IMPORT_VARIANT,
    ];
    pub fn build_record(version: &str) -> Result<Vec<u8>> {
        let previous = native_float_build_record(version)?;
        let marker = FLOAT_BUILD_RECORD_MARKER;
        let mut out = BUILD_RECORD_MARKER.to_vec();
        out.extend_from_slice(
            &previous[marker.len()..previous.len() - b"END_CODEC_WORKER_BUILD\0".len()],
        );
        out.extend_from_slice(b"JXL_INTEGER8_16_RGBA8_ROUND257_REPORT96_INPUT67108864_PIXELS4194304\0END_CODEC_WORKER_BUILD\0");
        Ok(out)
    }
}
pub mod writer {
    use super::*;
    pub const NATIVE_PROFILE: &str =
        "native-v10-jxl-integer8-16-srgb8-jxr-rgb8-lossless-parent-lease1";
    pub const BUILD_RECORD_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V10_JXL_INTEGER_JXR\0";
    pub const BUILD_RECORD: &[u8] = concat!("PAINTCRAFT_CODEC_WORKER_BUILD_V10_JXL_INTEGER_JXR\0", env!("CARGO_PKG_VERSION"), "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576\0PCENC001\0PCJXR001\0JXR_RGB8_LOSSLESS_SEEK_CAP2_INPUT3145728_PIXELS1048576_OUTPUT67108864\0JXR_WRITER_RGB8_READ1_INPUT67108864_PIXELS1048576_SIDES65536\0JXL_INTEGER8_16_RGBA8_ROUND257_REPORT96_INPUT67108864_PIXELS4194304\0END_CODEC_WORKER_BUILD\0").as_bytes();
    pub const IMPORT_VARIANTS: &[&str] = &[
        QUALIFIED_FLOAT_IMPORT_VARIANTS[0],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[1],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[2],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[3],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[4],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[5],
        QUALIFIED_FLOAT_IMPORT_VARIANTS[6],
        jxr_encode::qualified::WRITER_IMPORT_VARIANT,
        IMPORT_VARIANT,
    ];
    pub fn build_record(version: &str) -> Result<Vec<u8>> {
        let previous = jxr_encode::qualified::build_record(version)?;
        let marker = jxr_encode::qualified::BUILD_RECORD_MARKER;
        let mut out = BUILD_RECORD_MARKER.to_vec();
        out.extend_from_slice(
            &previous[marker.len()..previous.len() - b"END_CODEC_WORKER_BUILD\0".len()],
        );
        out.extend_from_slice(b"JXL_INTEGER8_16_RGBA8_ROUND257_REPORT96_INPUT67108864_PIXELS4194304\0END_CODEC_WORKER_BUILD\0");
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_records_match_compiled_bytes_without_widening_legacy() {
        assert_eq!(
            base::build_record(env!("CARGO_PKG_VERSION")).unwrap(),
            base::BUILD_RECORD
        );
        assert_eq!(
            writer::build_record(env!("CARGO_PKG_VERSION")).unwrap(),
            writer::BUILD_RECORD
        );
        assert_eq!(base::IMPORT_VARIANTS.len(), 8);
        assert_eq!(writer::IMPORT_VARIANTS.len(), 9);
        assert!(!QUALIFIED_FLOAT_IMPORT_VARIANTS.contains(&IMPORT_VARIANT));
        assert!(!jxr_encode::qualified::IMPORT_VARIANTS.contains(&IMPORT_VARIANT));
    }
}
