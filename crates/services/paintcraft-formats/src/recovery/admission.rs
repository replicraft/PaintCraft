//! Recovery readers share the caller's attachment realm and latch callback failures.
use super::*;
use crate::pcnd::MetadataAdmission;
use std::cell::RefCell;
pub(super) fn native<T>(f: impl FnOnce(MetadataAdmission<'_>) -> T) -> T {
    let budget = paintcraft_raster::Admission::new(0).paint_metadata_budget();
    f(MetadataAdmission {
        bytes: paintcraft_document::source_metadata::BYTES_MAX,
        budget: &budget,
    })
}
pub(super) struct Context<'a> {
    metadata: MetadataAdmission<'a>,
    caller: &'a dyn Fn() -> Result<()>,
    failure: RefCell<Option<Error>>,
}
impl<'a> Context<'a> {
    pub(super) fn new(metadata: MetadataAdmission<'a>, caller: &'a dyn Fn() -> Result<()>) -> Self {
        Self {
            metadata,
            caller,
            failure: RefCell::new(None),
        }
    }
    pub(super) fn check(&self) -> Result<()> {
        if let Some(error) = self.failure.borrow().as_ref() {
            return Err(error.clone());
        }
        if let Err(error) = (self.caller)() {
            *self.failure.borrow_mut() = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }
    pub(super) fn abort(&self, error: &Error) -> bool {
        self.failure.borrow().is_some()
            || matches!(
                error.code,
                ErrorCode::ResourceLimit | ErrorCode::OperationCancelled
            )
    }
    pub(super) fn canonical(&self, error: Error) -> Error {
        self.failure.borrow().clone().unwrap_or(error)
    }
    pub(super) fn read(&self, bytes: &[u8]) -> Result<Document> {
        crate::pcnd::read_with_admission_checked(
            bytes,
            u64::MAX,
            paintcraft_core::types::policy::STAGED_PIXELS_MAX,
            paintcraft_document::LAYER_COUNT_MAX,
            crate::pcnd::ReadAdmission {
                metadata: self.metadata,
                coverage: crate::pcnd::CoverageAdmission {
                    bytes: 256 << 20,
                    budget: &paintcraft_core::types::resources::default_coverage_budget(),
                    limits: paintcraft_raster::coverage_mask::Limits::default(),
                },
            },
            &|| self.check(),
        )
    }
    pub(super) fn file(&self, path: &Path, limit: u64) -> Result<Vec<u8>> {
        use std::io::Read;
        self.check()?;
        let metadata =
            fs::symlink_metadata(path).map_err(|e| storage("cannot inspect recovery file", &e))?;
        if !metadata.is_file() || metadata.len() > limit {
            return Err(journal::invalid("invalid recovery file type or size"));
        }
        let mut file = File::open(path).map_err(|e| storage("cannot read recovery file", &e))?;
        if !file
            .metadata()
            .map_err(|e| storage("cannot inspect opened recovery file", &e))?
            .is_file()
        {
            return Err(journal::invalid("invalid opened recovery file type"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(metadata.len() as usize)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "recovery input allocation"))?;
        let mut block = [0; 65536];
        loop {
            self.check()?;
            let length = file
                .read(&mut block)
                .map_err(|e| storage("cannot read recovery file", &e))?;
            if length == 0 {
                break;
            }
            if (bytes.len() as u64)
                .checked_add(length as u64)
                .is_none_or(|n| n > limit)
            {
                return Err(journal::invalid("recovery file grew beyond its limit"));
            }
            bytes
                .try_reserve_exact(length)
                .map_err(|_| Error::new(ErrorCode::ResourceLimit, "recovery input growth"))?;
            bytes.extend_from_slice(&block[..length]);
        }
        self.check()?;
        Ok(bytes)
    }
}
