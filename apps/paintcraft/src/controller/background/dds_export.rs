//! DDS uses the admitted editing executor, with its lease retained by the result.
use super::*;
use crate::dds_export::worker::Prepared;
#[derive(Debug)]
pub(crate) struct DdsExportPending(Pending);
impl DdsExportPending {
    pub(crate) fn poll(&self) -> Option<Result<Prepared>> {
        self.0.poll().map(|result| {
            result.and_then(|completion| match completion {
                Completion::DdsExport(prepared) => Ok(*prepared),
                _ => Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "Unexpected DDS export result",
                )),
            })
        })
    }
}
impl Service {
    /// None means an executor-owned preparation is still live. A retained
    /// external budget lease alone is not worker liveness and must be reported.
    pub(crate) fn try_submit_dds_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<Prepared> + Send + 'static,
    ) -> Result<Option<DdsExportPending>> {
        // A request which can never fit must not masquerade as contention.
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "DDS export exceeds editing admission",
            ));
        }
        match self.submit_dds_export(quote, task) {
            Ok(pending) => Ok(Some(pending)),
            Err(error) if error.code == ErrorCode::ResourceLimit => {
                let mut executor = self.lock()?;
                executor.reap_abandoned();
                if executor.request_fits(PREPARATION_CHARGE) && executor.pending() > 0 {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
            Err(error) => Err(error),
        }
    }
    pub(crate) fn submit_dds_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<Prepared> + Send + 'static,
    ) -> Result<DdsExportPending> {
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "DDS export exceeds editing admission",
            ));
        }
        self.submit(
            move |cancel| task(cancel).map(|value| Completion::DdsExport(Box::new(value))),
            "Export DDS".into(),
            "",
            None,
        )
        .map(DdsExportPending)
    }
}
