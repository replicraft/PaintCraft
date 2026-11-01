//! Source storage survives bounded execution jobs. Captured parents never renew.
use super::*;
use std::cell::Cell;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Initial,
    Active,
    Success,
    Failed,
}
#[derive(Debug)]
pub(super) struct ExecutionAuthority {
    default: bool,
    setup: u64,
    pending: Cell<u64>,
    intent: Option<[u8; 32]>,
    outcome: Outcome,
}
impl ExecutionAuthority {
    pub fn new(default: bool, job: &OpenJob) -> Result<Self> {
        Ok(Self {
            default,
            setup: if default { job.used()?.base } else { 0 },
            pending: Cell::new(0),
            intent: None,
            outcome: Outcome::Initial,
        })
    }
    /// Failed/stale metadata attempts remain debt; a generation never clears it.
    pub fn spend(&self, job: &OpenJob, n: u64) -> Result<()> {
        if !self.default {
            return job.spend_control(n);
        }
        let limit = job
            .limits()
            .base_work
            .checked_sub(self.setup)
            .ok_or_else(|| resource("Source7 setup exceeds job"))?;
        match self.pending.get().checked_add(n).filter(|v| *v <= limit) {
            Some(next) => {
                self.pending.set(next);
                Ok(())
            }
            None => {
                self.pending.set(limit);
                Err(resource("Source7 metadata debt exhausted"))
            }
        }
    }
    fn renew(&self, intent: [u8; 32]) -> bool {
        self.default
            && (self.outcome == Outcome::Success
                || (self.outcome == Outcome::Failed
                    && self.intent.is_some_and(|old| old != intent)))
    }
    pub fn check(&self, job: &OpenJob, intent: [u8; 32], work: WorkComponents) -> Result<()> {
        if !self.default {
            return job.check_available(work);
        }
        let used = if self.renew(intent) {
            WorkComponents {
                base: self.setup,
                noise: 0,
            }
        } else {
            job.used()?
        };
        let total = used
            .checked_add(WorkComponents {
                base: self.pending.get(),
                noise: 0,
            })?
            .checked_add(work)?;
        let limits = job.limits();
        if total.base > limits.base_work || total.noise > limits.noise_work {
            return Err(resource("Source7 execution job work admission"));
        }
        Ok(())
    }
    /// Called ONLY after source/operation/candidate/union/geometry validation.
    pub fn accept(&mut self, job: &mut OpenJob, intent: [u8; 32]) -> Result<()> {
        let renew = self.renew(intent);
        let result = (|| {
            // Decide from the prior outcome; a failure is recorded even before mint.
            self.check(job, intent, WorkComponents::default())?;
            if renew {
                let mode = if matches!(job, OpenJob::Noise(_)) {
                    Mode::Noise
                } else {
                    Mode::Base
                };
                let (next, default) =
                    OpenJob::capture(mode, job.limits(), job.workspace().scope().clone())?;
                if !default {
                    return Err(resource(
                        "Source7 default handover cannot borrow another caller",
                    ));
                }
                next.spend_control(
                    self.setup
                        .checked_add(self.pending.get())
                        .ok_or_else(|| resource("Source7 setup plus debt"))?,
                )?;
                *job = next;
                self.pending.set(0);
            } else {
                self.flush(job)?;
            }
            Ok(())
        })();
        self.intent = Some(intent);
        self.outcome = if result.is_ok() {
            Outcome::Active
        } else {
            Outcome::Failed
        };
        result
    }
    pub fn flush(&self, job: &OpenJob) -> Result<()> {
        if self.default {
            job.spend_control(self.pending.get())?;
            self.pending.set(0);
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn pending_work(&self) -> u64 {
        self.pending.get()
    }
    pub fn success(&mut self) {
        self.outcome = Outcome::Success;
    }
    pub fn fail(&mut self) {
        self.outcome = Outcome::Failed;
    }
    pub fn retire(&mut self) {
        if self.outcome == Outcome::Active {
            self.fail();
        }
    }
}
pub(super) fn intent(
    value: &Value,
    operation: &Value,
    request: DisplayRequest,
    copy: Option<[u8; 32]>,
) -> Result<[u8; 32]> {
    // No generation/draft/generated alias IDs/raw view spellings in this identity.
    let mut hash = Sha256::new();
    hash.update(b"paintcraft.source7.execution-intent/v1\0");
    // Typed normalized operation and target reuse the bounded control scratch.
    for part in [field(value, "target")?, operation] {
        let bytes = paintcraft_core::types::json::canonical(part);
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    let region = request.region();
    for coordinate in [region.x0, region.y0, region.x1, region.y1] {
        hash.update(coordinate.to_be_bytes());
    }
    hash.update([request.level()]);
    hash.update([u8::from(copy.is_some())]);
    if let Some(digest) = copy {
        hash.update(digest);
    }
    Ok(hash.finalize().into())
}

/// Forecast/control view; never creates or installs an execution owner.
pub(super) struct QuoteJob<'a> {
    pub job: &'a OpenJob,
    pub authority: &'a ExecutionAuthority,
    pub intent: [u8; 32],
}
impl QuoteJob<'_> {
    pub fn workspace(&self) -> &paintcraft_render::spatial::WorkspaceAdmission {
        self.job.workspace()
    }
    pub fn spend_control(&self, n: u64) -> Result<()> {
        self.authority.spend(self.job, n)
    }
    pub fn check_available(&self, work: WorkComponents) -> Result<()> {
        self.authority.check(self.job, self.intent, work)
    }
    pub fn validate_document(&self, doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
        self.job.validate_quote_document(doc, check)
    }
}
