use super::Fault;

/// Opaque registry identity, not a current-focus lookup or publication authority.
/// The widget prototype owns minting and incarnation replacement semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub widget: eframe::egui::Id,
    pub context: u128,
    pub incarnation: u64,
    pub session: u128,
    pub interface: u8,
    pub modal_generation: u64,
    pub window_generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Pending,
    Bound(Target),
    Revoked,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Binding {
    installation: u64,
    composition: u64,
    state: State,
    ended: bool,
}
impl Binding {
    pub fn new(installation: u64) -> Result<Self, Fault> {
        if installation == 0 || installation == u64::MAX {
            return Err(Fault::Order);
        }
        Ok(Self {
            installation,
            composition: 0,
            state: State::Revoked,
            ended: false,
        })
    }
    /// Called at the ordered semantic Begin edge, after earlier actions finish.
    /// Monotonic ids make all prior ids tombstones without unbounded storage.
    pub fn begin(&mut self, installation: u64, composition: u64) -> Result<(), Fault> {
        if installation != self.installation
            || composition == u64::MAX
            || composition <= self.composition
        {
            return Err(Fault::Composition);
        }
        self.composition = composition;
        self.state = State::Pending;
        self.ended = false;
        Ok(())
    }
    /// A coordinator may bind once, at Begin, never when Result arrives.
    /// Authentication/Scope checks remain the coordinator's responsibility.
    pub fn bind(&mut self, target: Target) -> Result<(), Fault> {
        if self.state != State::Pending {
            return Err(Fault::Target);
        }
        self.state = State::Bound(target);
        Ok(())
    }
    pub fn result(
        &self,
        installation: u64,
        composition: u64,
        current: Target,
    ) -> Result<Target, Fault> {
        if installation != self.installation || composition != self.composition {
            return Err(Fault::Composition);
        }
        match self.state {
            State::Bound(target) if target == current => Ok(target),
            _ => Err(Fault::Target),
        }
    }
    pub fn end(&mut self, installation: u64, composition: u64) -> Result<(), Fault> {
        if installation != self.installation || composition != self.composition {
            return Err(Fault::Composition);
        }
        self.ended = true;
        // Actual END is not a speculative result or policy revocation. A late
        // actual Result can still use this same immutable, unrevoked binding.
        Ok(())
    }
    pub fn revoke(&mut self) {
        self.state = State::Revoked;
    }
    pub fn preedit(
        &self,
        installation: u64,
        composition: u64,
        current: Target,
    ) -> Result<Target, Fault> {
        if self.ended {
            return Err(Fault::Composition);
        }
        self.result(installation, composition, current)
    }
}
