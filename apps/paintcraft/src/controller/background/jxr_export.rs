//! JPEG XR shares the editing executor and retains its exact admitted result.
use super::*;
use paintcraft_formats::codec_worker::native::PreparedJxrEncode;

#[derive(Debug)]
pub(crate) struct PreparedJxrExport {
    pub(crate) encoded: PreparedJxrEncode,
    pub(crate) losses: Vec<String>,
}
#[derive(Debug)]
pub(crate) struct JxrExportPending(Pending);
impl JxrExportPending {
    pub(crate) fn poll(&self) -> Option<Result<PreparedJxrExport>> {
        self.0.poll().map(|result| {
            result.and_then(|completion| match completion {
                Completion::JxrExport(prepared) => Ok(*prepared),
                _ => Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "Unexpected JPEG XR export result",
                )),
            })
        })
    }
}
impl Service {
    /// Retry only executor-owned active contention; uncertain retirement is not
    /// reclamation, and external/impossible admission remains a typed refusal.
    pub(crate) fn try_submit_jxr_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<PreparedJxrExport> + Send + 'static,
    ) -> Result<Option<JxrExportPending>> {
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "JPEG XR export exceeds editing admission",
            ));
        }
        match self.submit_jxr_export(quote, task) {
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
    pub(crate) fn submit_jxr_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<PreparedJxrExport> + Send + 'static,
    ) -> Result<JxrExportPending> {
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "JPEG XR export exceeds editing admission",
            ));
        }
        self.submit(
            move |cancel| task(cancel).map(|value| Completion::JxrExport(Box::new(value))),
            "Export JPEG XR".into(),
            "",
            None,
        )
        .map(JxrExportPending)
    }
}
