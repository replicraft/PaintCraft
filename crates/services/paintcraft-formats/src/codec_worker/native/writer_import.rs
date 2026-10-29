//! Closed writer-compatible import with the exact final helper contract.
#[cfg(test)]
use super::super::jxr_encode::qualified;
use super::*;
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};
pub(super) enum Helper {
    Legacy(codec_helper::VerifiedDecoderInstallation),
    Writer(codec_helper::VerifiedDecoderInstallation),
}
impl Helper {
    pub fn writer(version: &str, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let helper = super::writer_contracts::with_contracts(version, |contracts| {
            codec_helper::discover_decoder_compatible_set(&[], contracts, check)
        })?;
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
    /// Select the native short/writer-compatible family from one acquired source,
    /// then require exact frontend admission before discovery or child launch.
    pub fn prepare_jxr_family(
        self,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        admit: impl FnOnce(&crate::jxr::NativeImportAssessment) -> Result<()>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if self.codec != Codec::JxrRgb8 {
            return Err(unsupported(
                "JXR family selection requires explicit JXR RGB8",
            ));
        }
        self.prepare_selected(
            false,
            residency,
            lease,
            host_check,
            current_owner,
            |bytes, pixels, check| {
                let assessment = crate::jxr::assess_native_import(bytes, pixels, check)?;
                admit(&assessment)?;
                check()?;
                Ok(super::import::Selection::Jxr(assessment))
            },
            launch,
        )
    }
    /// Explicit writer-compatible mode. Caller supplies the admitted source/request/child
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
                Ok(super::import::Selection::Fixed)
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
    fn family_admission_uses_captured_structure_before_launch_and_rechecks_source() {
        let root = std::env::temp_dir().join(format!("paintcraft-jxr-family-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let input = root.join("source.jxr");
        let raw = include_bytes!("../../../tests/jxr-fixtures/white-rgb8-4096x1.jxr");
        let owner = Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 0,
            frontend: 3,
        };
        let budget = SharedBudget::new(HEAP_BYTES);
        let residency = SharedBudget::new(1 << 20);
        for mutate in [false, true] {
            std::fs::write(&input, raw).unwrap();
            let admitted = Cell::new(false);
            let job = Job {
                input: input.clone(),
                identity: SourceIdentity::existing_destination(&input)
                    .unwrap()
                    .unwrap(),
                length: raw.len() as u64,
                codec: Codec::JxrRgb8,
                owner,
                pixels: 1_048_576,
                deadline: Duration::from_secs(20),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: Some(hex(&Sha256::digest(raw))),
            };
            let error = job
                .prepare_jxr_family(
                    residency.clone(),
                    ResourceLease::new(budget.reserve(HEAP_BYTES).unwrap()),
                    &|| Ok(()),
                    &|| Ok(owner),
                    |assessment| {
                        admitted.set(true);
                        assert!(assessment.is_writer_compatible());
                        assert_eq!(assessment.assessment().source().width, 4096);
                        if mutate {
                            let mut changed = raw.to_vec();
                            changed[0] ^= 1;
                            std::fs::write(&input, changed).unwrap();
                            Ok(())
                        } else {
                            Err(unsupported("host denies selected variant"))
                        }
                    },
                    |_, _| panic!("no helper before admitted source verification"),
                )
                .unwrap_err();
            assert_eq!(
                error.code,
                if mutate {
                    ErrorCode::ConflictStaleBase
                } else {
                    ErrorCode::CapabilityUnsupported
                }
            );
            assert!(admitted.get());
            assert_eq!(residency.usage().reserved, 0);
            assert_eq!(budget.usage().reserved, 0);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
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
