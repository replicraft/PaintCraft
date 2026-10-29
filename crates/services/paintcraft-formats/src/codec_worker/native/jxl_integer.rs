//! Exact candidate decoder discovery. Older contracts never authorize tag 9.
use super::*;
use crate::codec_worker::jxl_integer::contracts::{base, writer};
pub(super) fn discover(
    version: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<codec_helper::VerifiedDecoderInstallation> {
    let application = std::env::current_exe()
        .map_err(|_| Error::new(ErrorCode::StorageFailed, "Cannot locate codec application"))?;
    for_application(&application, version, check)
}
fn for_application(
    application: &Path,
    version: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<codec_helper::VerifiedDecoderInstallation> {
    let base_record = base::build_record(version)?;
    let writer_record = writer::build_record(version)?;
    codec_helper::for_application_decoder_compatible(
        application,
        &[codec_helper::Contract {
            application_version: version,
            protocol_version: PROTOCOL_VERSION,
            profile: base::NATIVE_PROFILE,
            requested_heap_ceiling_bytes: HEAP_BYTES,
            qualified_import_variants: base::IMPORT_VARIANTS,
            build_marker: base::BUILD_RECORD_MARKER,
            build_record: &base_record,
        }],
        codec_helper::EncoderContract {
            decode: codec_helper::Contract {
                application_version: version,
                protocol_version: PROTOCOL_VERSION,
                profile: writer::NATIVE_PROFILE,
                requested_heap_ceiling_bytes: HEAP_BYTES,
                qualified_import_variants: writer::IMPORT_VARIANTS,
                build_marker: writer::BUILD_RECORD_MARKER,
                build_record: &writer_record,
            },
            encode_protocol_version: crate::codec_worker::jxr_encode::VERSION,
            qualified_export_variants: crate::codec_worker::jxr_encode::qualified::EXPORT_VARIANTS,
        },
        check,
    )
}

#[cfg(test)]
mod tests;
