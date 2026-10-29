//! The two exact public writer-capable inventories; decoder-only V9 is absent.
use super::*;
use crate::codec_worker::{jxl_integer::contracts::writer, jxr_encode::qualified};
pub(super) fn with_contracts<T>(
    version: &str,
    consume: impl FnOnce(&[codec_helper::EncoderContract<'_>]) -> Result<T>,
) -> Result<T> {
    let v8 = qualified::build_record(version)?;
    let v10 = writer::build_record(version)?;
    let contracts = [
        codec_helper::EncoderContract {
            decode: codec_helper::Contract {
                application_version: version,
                protocol_version: PROTOCOL_VERSION,
                profile: qualified::NATIVE_PROFILE,
                requested_heap_ceiling_bytes: HEAP_BYTES,
                qualified_import_variants: qualified::IMPORT_VARIANTS,
                build_marker: qualified::BUILD_RECORD_MARKER,
                build_record: &v8,
            },
            encode_protocol_version: crate::codec_worker::jxr_encode::VERSION,
            qualified_export_variants: qualified::EXPORT_VARIANTS,
        },
        codec_helper::EncoderContract {
            decode: codec_helper::Contract {
                application_version: version,
                protocol_version: PROTOCOL_VERSION,
                profile: writer::NATIVE_PROFILE,
                requested_heap_ceiling_bytes: HEAP_BYTES,
                qualified_import_variants: writer::IMPORT_VARIANTS,
                build_marker: writer::BUILD_RECORD_MARKER,
                build_record: &v10,
            },
            encode_protocol_version: crate::codec_worker::jxr_encode::VERSION,
            qualified_export_variants: qualified::EXPORT_VARIANTS,
        },
    ];
    consume(&contracts)
}
