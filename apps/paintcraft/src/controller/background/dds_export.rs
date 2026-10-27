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
