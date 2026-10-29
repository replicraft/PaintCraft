//! TIFF phases use the existing one-job service; ready values own no job lease.
use super::*;

pub(crate) enum TiffCompletion {
    Inventory(Arc<crate::importing::tiff::Frozen>),
    Decoded(Box<crate::importing::tiff::Prepared>),
}
impl std::fmt::Debug for TiffCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Inventory(_) => "TiffInventory",
            Self::Decoded(_) => "TiffDecoded",
        })
    }
}
#[derive(Debug)]
pub(crate) struct TiffPending(Pending);
impl TiffPending {
    pub(crate) fn poll(&self) -> Option<Result<TiffCompletion>> {
        self.0.poll().map(|result| {
            result.and_then(|completion| match completion {
                Completion::TiffImport(value) => Ok(value),
                _ => Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "Unexpected TIFF completion",
                )),
            })
        })
    }
}
impl Service {
    pub(crate) fn submit_tiff(
        &self,
        task: impl FnOnce(Cancellation) -> Result<TiffCompletion> + Send + 'static,
    ) -> Result<TiffPending> {
        self.submit(
            move |cancel| task(cancel).map(Completion::TiffImport),
            "Import image".into(),
            "",
            None,
        )
        .map(TiffPending)
    }
}
