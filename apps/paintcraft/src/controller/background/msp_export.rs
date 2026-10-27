//! MSP uses the admitted editing executor, with its lease retained by the result.
use super::*;
use crate::msp_export::worker::Prepared;
#[derive(Debug)]
pub(crate) struct MspExportPending(Pending);
impl MspExportPending {
    pub(crate) fn poll(&self) -> Option<Result<Prepared>> {
        self.0.poll().map(|result| {
            result.and_then(|completion| match completion {
                Completion::MspExport(prepared) => Ok(*prepared),
                _ => Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "Unexpected MSP export result",
                )),
            })
        })
    }
}
impl Service {
    /// None means an executor-owned preparation is still live. A retained
    /// external budget lease alone is not worker liveness and must be reported.
    pub(crate) fn try_submit_msp_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation, u64) -> Result<Prepared> + Send + 'static,
    ) -> Result<Option<MspExportPending>> {
        // A request which can never fit must not masquerade as contention.
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "MSP export exceeds editing admission",
            ));
        }
        match self.submit_msp_export(quote, task) {
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
    pub(crate) fn submit_msp_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation, u64) -> Result<Prepared> + Send + 'static,
    ) -> Result<MspExportPending> {
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "MSP export exceeds editing admission",
            ));
        }
        self.submit(
            move |cancel| {
                task(cancel, PREPARATION_CHARGE).map(|value| Completion::MspExport(Box::new(value)))
            },
            "Export MSP".into(),
            "",
            None,
        )
        .map(MspExportPending)
    }
}
