//! One non-clonable Main prefix; sealing transfers only remaining base authority.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::export::layer_appearance::Admission7;
use std::cell::Cell;
#[derive(Debug)]
pub(in crate::app::filter_editing) struct Preparation7 {
    parent: super::parent::Parent7,
    selected: Cell<Option<Admission7>>,
    workspace: u64,
    base_limit: u64,
    noise_limit: u64,
    remaining: Cell<u64>,
    failed: Cell<Option<ErrorCode>>,
}
#[derive(Debug)]
pub(in crate::app::filter_editing) struct Sealed7 {
    pub admission: Admission7,
    pub main_spent: u64,
    _remote: Option<super::parent::Remote7>,
}
impl Preparation7 {
    #[cfg(test)]
    pub fn new(original: Admission7) -> Result<Self> {
        original.validate()?;
        Ok(Self {
            parent: super::parent::Parent7::Standalone,
            remaining: Cell::new(original.base_work),
            selected: Cell::new(Some(original)),
            workspace: original.workspace_bytes,
            base_limit: original.base_work,
            noise_limit: original.noise_work,
            failed: Cell::new(None),
        })
    }
    /// Small fixed control state can exist before the JS prelease. No numerical
    /// grant exists until the actual source/prospective scan selects mode once.
    #[cfg(test)]
    pub fn before_capture() -> Self {
        Self::from_parent(super::parent::Parent7::Standalone).unwrap()
    }
    pub(super) fn from_parent(parent: super::parent::Parent7) -> Result<Self> {
        let (workspace, base_limit, noise_limit) = parent.limits()?;
        Ok(Self {
            parent,
            selected: Cell::new(None),
            workspace,
            base_limit,
            noise_limit,
            remaining: Cell::new(base_limit),
            failed: Cell::new(None),
        })
    }
    pub fn select_mode(
        &self,
        mode: paintcraft_formats::export::layer_appearance::Mode7,
    ) -> Result<Admission7> {
        if self.selected.get().is_some() {
            self.failed.set(Some(ErrorCode::ConflictStaleBase));
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Source7 mode already selected",
            ));
        }
        self.spend(0)?;
        self.parent.validate_mode(mode)?;
        let admission = Admission7 {
            work_authority: self.parent.origin(),
            mode,
            workspace_bytes: self.workspace,
            base_work: self.base_limit,
            noise_work: if mode == paintcraft_formats::export::layer_appearance::Mode7::Noise {
                self.noise_limit
            } else {
                0
            },
        };
        admission.validate()?;
        self.selected.set(Some(admission));
        Ok(admission)
    }
    pub fn spend(&self, units: u64) -> Result<()> {
        if let Some(code) = self.failed.get() {
            return Err(Error::new(code, "Source7 Main preparation stopped"));
        }
        match self.remaining.get().checked_sub(units) {
            Some(next) => {
                if let Err(error) = self.parent.spend(units) {
                    self.failed.set(Some(error.code));
                    return Err(error);
                }
                self.remaining.set(next);
                Ok(())
            }
            None => {
                self.failed.set(Some(ErrorCode::ResourceLimit));
                Err(limit())
            }
        }
    }
    pub(super) fn validate_document(
        &self,
        doc: &paintcraft_document::Document,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        self.parent
            .validate_document(doc, check, &|n| self.spend(n))
    }
    #[cfg(test)]
    pub fn cancel(&self) {
        if self.failed.get().is_none() {
            self.failed.set(Some(ErrorCode::OperationCancelled));
        }
    }
    /// Precharge trusted final framing BEFORE computing the final admission SHA.
    /// This consumes the prefix, so neither BEGIN nor a retry can reset/reseal it.
    pub fn seal(self, final_framing_work: u64) -> Result<Sealed7> {
        self.spend(final_framing_work)?;
        let remaining = self.remaining.get();
        let original = self
            .selected
            .get()
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "Source7 mode not selected"))?;
        let admission = Admission7 {
            base_work: remaining,
            ..original
        };
        let remote = self.parent.claim(admission)?;
        Ok(Sealed7 {
            admission,
            main_spent: self.base_limit - remaining,
            _remote: remote,
        })
    }
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source7 Main preparation work exhausted",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_formats::export::layer_appearance::Mode7;
    fn admission() -> Admission7 {
        Admission7 {
            work_authority:
                paintcraft_formats::export::layer_appearance::Origin7::StandaloneViewJobs,
            mode: Mode7::Noise,
            workspace_bytes: 64 << 20,
            base_work: 100,
            noise_work: 1 << 35,
        }
    }
    #[test]
    fn prefix_and_worker_base_sum_to_original_while_noise_grant_is_unchanged() {
        let prefix = Preparation7::new(admission()).unwrap();
        prefix.spend(17).unwrap();
        prefix.spend(23).unwrap();
        let sealed = prefix.seal(11).unwrap();
        assert_eq!(sealed.main_spent, 51);
        assert_eq!(sealed.admission.base_work, 49);
        assert_eq!(sealed.admission.noise_work, 1 << 35);
        assert_eq!(sealed.main_spent + sealed.admission.base_work, 100);
    }
    #[test]
    fn exhaustion_and_cancellation_latch_without_reusable_partial_authority() {
        let prefix = Preparation7::new(admission()).unwrap();
        assert_eq!(
            prefix.spend(101).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert!(prefix.spend(0).is_err());
        assert!(prefix.seal(0).is_err());
        let prefix = Preparation7::new(admission()).unwrap();
        prefix.spend(7).unwrap();
        prefix.cancel();
        assert_eq!(
            prefix.seal(0).unwrap_err().code,
            ErrorCode::OperationCancelled
        );
        let prefix = Preparation7::new(admission()).unwrap();
        assert_eq!(prefix.seal(101).unwrap_err().code, ErrorCode::ResourceLimit);
    }
}
#[cfg(test)]
mod mode_tests {
    use super::*;
    use paintcraft_formats::export::layer_appearance::Mode7;
    #[test]
    fn mode_selection_keeps_prelease_scan_spent_and_cannot_rebind_or_reset() {
        let prefix = Preparation7::before_capture();
        prefix.spend(37).unwrap();
        let selected = prefix.select_mode(Mode7::Noise).unwrap();
        assert_eq!(selected.noise_work, Admission7::NOISE_MAX);
        let sealed = prefix.seal(13).unwrap();
        assert_eq!(sealed.main_spent, 50);
        assert_eq!(sealed.admission.base_work + 50, Admission7::BASE_MAX);
        let prefix = Preparation7::before_capture();
        prefix.select_mode(Mode7::Base).unwrap();
        assert_eq!(
            prefix.select_mode(Mode7::Noise).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert!(prefix.seal(0).is_err());
        assert!(Preparation7::before_capture().seal(0).is_err());
    }
}
