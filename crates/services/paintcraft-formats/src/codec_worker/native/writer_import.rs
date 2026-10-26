//! Closed private writer-compatible import mode over the ordinary decoder runner.
use super::super::jxr_encode::candidate;
use super::*;
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};
pub(super) enum Helper {
    Legacy(codec_helper::VerifiedHelper),
    Writer(codec_helper::VerifiedSchema3DecoderHelper),
}
impl Helper {
    pub fn writer(version: &str, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let record = candidate::build_record(version)?;
        let helper = codec_helper::discover_decoder(
            codec_helper::EncoderContract {
                decode: codec_helper::Contract {
                    application_version: version,
                    protocol_version: PROTOCOL_VERSION,
                    profile: candidate::NATIVE_PROFILE,
                    requested_heap_ceiling_bytes: HEAP_BYTES,
                    qualified_import_variants: candidate::IMPORT_VARIANTS,
                    build_marker: candidate::BUILD_RECORD_MARKER,
                    build_record: &record,
                },
                encode_protocol_version: super::super::jxr_encode::VERSION,
                qualified_export_variants: candidate::EXPORT_VARIANTS,
            },
            check,
        )?;
        Ok(Self::Writer(helper))
    }
    pub fn checked_executable(&self, check: &dyn Fn() -> Result<()>) -> Result<&Path> {
        match self {
            Self::Legacy(h) => h.checked_executable(check),
            Self::Writer(h) => h.checked_executable(check),
        }
    }
    pub fn sha256(&self) -> [u8; 32] {
        match self {
            Self::Legacy(h) => h.sha256(),
            Self::Writer(h) => h.sha256(),
        }
    }
}
impl Job {
    /// Private explicit mode. Caller supplies the admitted source/request/child
    /// lease and resident realm exactly as for existing native import preparation.
    pub fn prepare_jxr_writer(
        self,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if self.codec != Codec::JxrRgb8 {
            return Err(unsupported(
                "Writer-compatible import requires explicit JXR RGB8",
            ));
        }
        self.prepare_selected(
            true,
            residency,
            lease,
            host_check,
            current_owner,
            |bytes, pixels, check| {
                crate::jxr::assess_writer_import(bytes, pixels, check)?;
                Ok(None)
            },
            launch,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn writer_subset_and_cancellation_refuse_before_discovery_launch_or_resident_allocation() {
        let root = std::env::temp_dir().join(format!(
            "paintcraft-jxr-writer-preflight-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let input = root.join("source.jxr");
        let bytes = include_bytes!("../../../tests/jxr-fixtures/rgb8-dpi96x144.jxr");
        std::fs::write(&input, bytes).unwrap();
        let owner = Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 0,
            frontend: 3,
        };
        let residency = SharedBudget::new(1 << 20);
        let prep = SharedBudget::new(HEAP_BYTES);
        for cancel in [false, true] {
            let job = Job {
                input: input.clone(),
                identity: SourceIdentity::existing_destination(&input)
                    .unwrap()
                    .unwrap(),
                length: bytes.len() as u64,
                codec: Codec::JxrRgb8,
                owner,
                pixels: 1_048_576,
                deadline: Duration::from_secs(20),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: Some(hex(&Sha256::digest(bytes))),
            };
            let launched = Cell::new(false);
            let error = job
                .prepare_jxr_writer(
                    residency.clone(),
                    ResourceLease::new(prep.reserve(HEAP_BYTES).unwrap()),
                    &|| {
                        if cancel {
                            Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                        } else {
                            Ok(())
                        }
                    },
                    &|| Ok(owner),
                    |_, _| {
                        launched.set(true);
                        unreachable!()
                    },
                )
                .unwrap_err();
            assert_eq!(
                error.code,
                if cancel {
                    ErrorCode::OperationCancelled
                } else {
                    ErrorCode::CapabilityUnsupported
                }
            );
            assert!(!launched.get());
            assert_eq!(residency.usage().reserved, 0);
            assert_eq!(prep.usage().reserved, 0);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod installed;
#[cfg(test)]
pub(super) fn qualify(root: &Path) {
    installed::qualify(root);
}
