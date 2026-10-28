//! One atomic continuation transaction over opaque pen and native-touch bindings.
use super::*;
#[derive(Debug)]
pub enum Continuation<'a> {
    Pen(&'a mut Batch),
    NativeTouch(&'a mut NativeTouchBatch),
}
trait Bindings {
    fn len(&self) -> usize;
    fn binding(&self, index: usize) -> &Binding;
    fn binding_mut(&mut self, index: usize) -> &mut Binding;
}
impl Bindings for [&mut Batch] {
    fn len(&self) -> usize {
        <[&mut Batch]>::len(self)
    }
    fn binding(&self, i: usize) -> &Binding {
        &self[i].binding
    }
    fn binding_mut(&mut self, i: usize) -> &mut Binding {
        &mut self[i].binding
    }
}
impl Bindings for [Continuation<'_>] {
    fn len(&self) -> usize {
        <[Continuation<'_>]>::len(self)
    }
    fn binding(&self, i: usize) -> &Binding {
        match &self[i] {
            Continuation::Pen(b) => &b.binding,
            Continuation::NativeTouch(b) => &b.binding,
        }
    }
    fn binding_mut(&mut self, i: usize) -> &mut Binding {
        match &mut self[i] {
            Continuation::Pen(b) => &mut b.binding,
            Continuation::NativeTouch(b) => &mut b.binding,
        }
    }
}
impl Source {
    /// Continue one admitted pen batch after a trusted actual predecessor result.
    pub fn acknowledge_predecessor(
        &self,
        batch: &mut Batch,
        proof: Predecessor,
    ) -> std::result::Result<(), Invalid> {
        self.acknowledge_predecessors(&mut [batch], proof)
    }
    /// Existing pen-only callers use the exact same transaction as mixed hosts.
    pub fn acknowledge_predecessors(
        &self,
        batches: &mut [&mut Batch],
        proof: Predecessor,
    ) -> std::result::Result<(), Invalid> {
        self.acknowledge_bindings(batches, proof)
    }
    /// Host supplies bounded/admitted reference storage before entering Source.
    /// Proof must come from its exact earlier tagged typed publication/navigation
    /// result and clean service barrier, never a changed head ticket. No GUI,
    /// allocation, or partial rebase occurs inside this transaction.
    pub fn acknowledge_continuations(
        &self,
        batches: &mut [Continuation<'_>],
        proof: Predecessor,
    ) -> std::result::Result<(), Invalid> {
        self.acknowledge_bindings(batches, proof)
    }
    fn acknowledge_bindings<T: Bindings + ?Sized>(
        &self,
        batches: &mut T,
        proof: Predecessor,
    ) -> std::result::Result<(), Invalid> {
        if batches.len() == 0 {
            return Err(Invalid::Order);
        }
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if (0..batches.len())
            .any(|i| !self.current_binding(&state, batches.binding(i), proof.before))
        {
            return Err(Invalid::Owner);
        }
        let Some(mut view) = state.view else {
            return Err(Invalid::Owner);
        };
        if view.owner != proof.before
            || proof.before.session != proof.after.session
            || proof.before.interface != proof.after.interface
            || proof.before.publication.checked_add(1) != Some(proof.after.publication)
            || !proof.focused
            || proof.modal
            || proof.ime
            || proof.transform != view.transform
            || proof.canvas != view.canvas
            || proof.pixels_per_point != view.pixels_per_point
        {
            return Err(Invalid::Owner);
        }
        if (0..batches.len()).any(|i| {
            let b = batches.binding(i);
            b.acknowledged.is_some_and(|old| proof.sequence <= old)
                || !b.last_down.is_some_and(|down| down > proof.sequence)
        }) {
            return Err(Invalid::Order);
        }
        if (0..batches.len()).any(|i| {
            batches
                .binding(i)
                .native_touch
                .as_ref()
                .is_some_and(|lease| !state.native_touch.validate_continuation(lease, proof))
        }) {
            return Err(Invalid::Owner);
        }
        // Validate every private binding, source and lifetime before any mutation.
        view.owner = proof.after;
        state.view = Some(view);
        for i in 0..batches.len() {
            let b = batches.binding_mut(i);
            if let Some(lease) = &b.native_touch {
                state.native_touch.acknowledge(lease, proof.after);
            }
            b.owner = proof.after;
            if let Some(captured) = &mut b.view {
                captured.owner = proof.after;
            }
            b.acknowledged = Some(proof.sequence);
        }
        Ok(())
    }
}
