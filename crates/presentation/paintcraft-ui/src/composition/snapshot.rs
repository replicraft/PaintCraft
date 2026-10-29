//! Owned rollback, not a cloned TextEditState alias of its mutable undo Arc.
use super::history::Bound;
use egui::{text::CCursorRange, text_edit::TextEditState};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};

type Undoer = egui::util::undoer::Undoer<(CCursorRange, String)>;
pub(super) struct Snapshot {
    state: TextEditState,
    history: Option<Undoer>,
    bound: Bound,
    // Storage/undo state drops before the account. Existing framework state is
    // included in the peak declaration; this does not claim process RSS.
    storage: BudgetAccount,
}
impl Snapshot {
    /// Caller must prove Bound has owned this exact state from first mount.
    pub fn take(state: &mut TextEditState, bound: Bound, budget: &SharedBudget) -> Result<Self> {
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize(quote(bound)?)?;
        // This is the only deep clone, after the admission above. clear_undoer
        // mutates the shared Arc; the OWNED history below is its rollback.
        let history = state.undoer();
        let saved = state.clone();
        state.clear_undoer();
        Ok(Self {
            state: saved,
            history: Some(history),
            bound,
            storage,
        })
    }
    pub fn admit_observed(&mut self, lifetime: &mut Bound, next: Bound) -> Result<()> {
        if *lifetime != self.bound {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "composition history accounting changed",
            ));
        }
        self.storage.resize(quote(next)?)?;
        *lifetime = next;
        self.bound = next;
        Ok(())
    }
    #[cfg(test)]
    pub fn admit_batch(
        &mut self,
        lifetime: &mut Bound,
        text_bytes: usize,
        events: &[egui::Event],
    ) -> Result<()> {
        let mut next = *lifetime;
        next.observe(text_bytes, events)?;
        self.admit_observed(lifetime, next)
    }
    pub fn checkpoint(
        &mut self,
        lifetime: &mut Bound,
        before: (CCursorRange, &str),
        after: (CCursorRange, &str),
    ) -> Result<()> {
        if *lifetime != self.bound {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "composition history accounting changed",
            ));
        }
        let mut next = *lifetime;
        next.observe(before.1.len().max(after.1.len()), &[])?;
        next.checkpoints(2)?;
        self.storage.resize(quote(next)?)?;
        *lifetime = next;
        let history = self
            .history
            .as_mut()
            .expect("owned snapshot not restored yet");
        history.add_undo(&(before.0, before.1.to_owned()));
        let result = (after.0, after.1.to_owned());
        history.feed_state(0.0, &result);
        history.add_undo(&result);
        self.bound = next;
        Ok(())
    }
    pub fn set_cursor(&mut self, range: CCursorRange) {
        self.state.cursor.set_char_range(Some(range));
    }
    pub fn restore(mut self) -> TextEditState {
        self.restore_history();
        self.state.clone()
    }
    fn restore_history(&mut self) {
        if let Some(history) = self.history.take() {
            self.state.set_undoer(history);
        }
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        // A failed/abandoned attempt must not leave the live Arc cleared.
        self.restore_history();
    }
}

fn quote(bound: Bound) -> Result<u64> {
    // Base/working Strings, shadow reducer/result, caller draft growth, and
    // replacement temporaries can coexist. Charge ten text bounds in addition
    // to the history/feed scratch, including geometric String growth slack.
    bound
        .history_bytes()?
        .checked_add(bound.text_bytes().checked_mul(10).ok_or_else(|| {
            Error::new(ErrorCode::ResourceLimit, "composition text bound overflow")
        })?)
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "composition quote overflow"))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn range(at: usize) -> CCursorRange {
        CCursorRange::one(egui::text::CCursor::new(at))
    }
    #[test]
    fn admission_failure_does_not_clear_existing_history() {
        let mut state = TextEditState::default();
        let mut history = state.undoer();
        history.add_undo(&(range(0), "before".into()));
        state.set_undoer(history);
        let mut bound = Bound::default();
        bound.observe(6, &[]).unwrap();
        assert!(Snapshot::take(&mut state, bound, &SharedBudget::new(1)).is_err());
        assert_eq!(
            state.undoer().undo(&(range(5), "after".into())).unwrap().1,
            "before"
        );
    }
    #[test]
    fn rollback_restores_owned_history_even_though_state_clones_share_the_arc() {
        let mut state = TextEditState::default();
        let mut history = state.undoer();
        history.add_undo(&(range(0), "baseline".into()));
        state.set_undoer(history);
        let alias = state.clone();
        let mut bound = Bound::default();
        bound.observe(32, &[]).unwrap();
        let budget = SharedBudget::new(1 << 20);
        let saved = Snapshot::take(&mut state, bound, &budget).unwrap();
        assert!(!alias.undoer().has_undo(&(range(0), "baseline".into())));
        let mut provisional = state.undoer();
        provisional.add_undo(&(range(1), "provisional".into()));
        state.set_undoer(provisional);
        drop(saved);
        let mut restored = alias.undoer();
        assert_eq!(
            restored.undo(&(range(5), "after".into())).unwrap().1,
            "baseline"
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn result_checkpoint_excludes_provisional_history_and_retains_real_baseline() {
        let mut state = TextEditState::default();
        let mut bound = Bound::default();
        bound.observe(32, &[]).unwrap();
        let budget = SharedBudget::new(1 << 20);
        let mut saved = Snapshot::take(&mut state, bound, &budget).unwrap();
        saved
            .admit_batch(
                &mut bound,
                32,
                &[egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "provisional".into(),
                    active_range_chars: None,
                })],
            )
            .unwrap();
        let mut provisional = state.undoer();
        provisional.add_undo(&(range(1), "provisional".into()));
        state.set_undoer(provisional);
        saved
            .checkpoint(&mut bound, (range(0), "baseline"), (range(6), "result"))
            .unwrap();
        assert_eq!(bound.snapshots().unwrap(), 103);
        let restored = saved.restore();
        let mut history = restored.undoer();
        assert_eq!(
            history.undo(&(range(6), "result".into())).unwrap().1,
            "baseline"
        );
        assert!(!history.has_undo(&(range(0), "baseline".into())));
        assert_eq!(budget.usage().reserved, 0);
    }
}
