//! Opt-in pen contact tokens. Broad Source cancellation remains a separate API.
use super::*;
use contact_tokens::{Bank, Cancellation, Key, Lease, Leases, Target};
use paintcraft_input::tablet::{Buttons, Identity, Provenance};
#[derive(Debug)]
struct Active {
    lease: Lease,
    position: Option<(f64, f64)>,
}
#[derive(Debug)]
struct Contacts {
    bank: Bank,
    ink: Option<Active>,
    ui: Option<Active>,
    pending: Leases,
    physical: Option<Identity>,
    frame: Option<u64>,
}
#[derive(Debug, Default)]
pub(super) struct Resident {
    value: Vec<Contacts>,
    _storage: Option<BudgetAccount>,
}
/// Negative presentation authority for one exact contact, not permission to
/// reset every GUI pointer or cancel an earlier prepared edit.
#[derive(Clone, Debug)]
pub struct NativePenCancellation {
    token: Cancellation,
    position: Option<(f64, f64)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePenTarget {
    Ink,
    Ui,
}
impl NativePenCancellation {
    pub fn matches_batch(&self, batch: &Batch) -> bool {
        batch
            .binding
            .native_contacts
            .as_ref()
            .is_some_and(|leases| leases.matches(&self.token))
    }
    pub fn matches_publication(&self, guard: &PublicationGuard) -> bool {
        guard
            .native_contacts
            .as_ref()
            .is_some_and(|leases| leases.matches(&self.token))
    }
    pub fn last_observed_position(&self) -> Option<(f64, f64)> {
        self.position
    }
}
impl Resident {
    pub(super) fn new(budget: &SharedBudget) -> Result<Self> {
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize(std::mem::size_of::<Contacts>() as u64)?;
        let mut value = Vec::new();
        value
            .try_reserve_exact(1)
            .map_err(|_| invalid(Invalid::Capacity))?;
        storage.resize((value.capacity() * std::mem::size_of::<Contacts>()) as u64)?;
        value.push(Contacts {
            bank: Bank::new(budget)?,
            ink: None,
            ui: None,
            pending: Leases::new(),
            physical: None,
            frame: None,
        });
        Ok(Self {
            value,
            _storage: Some(storage),
        })
    }
    pub(super) fn configured(&self) -> bool {
        !self.value.is_empty()
    }
    pub(super) fn context_matches(&self, leases: &Leases, owner: Owner) -> bool {
        self.value
            .first()
            .is_some_and(|v| v.bank.context_matches(leases, owner))
    }
    pub(super) fn acknowledge(&mut self, leases: &Leases, owner: Owner) {
        self.value
            .first_mut()
            .expect("validated contact configuration")
            .bank
            .acknowledge(leases, owner);
    }
    pub(super) fn follows(&self, leases: &Leases, sequence: u64) -> bool {
        self.value
            .first()
            .is_some_and(|v| v.bank.follows(leases, sequence))
    }
    pub(super) fn take(&mut self, frame: u64) -> Option<Leases> {
        self.value.first_mut().map(|v| {
            if v.frame == Some(frame) {
                return Leases::new();
            }
            v.frame = Some(frame);
            std::mem::take(&mut v.pending)
        })
    }
    pub(super) fn reconcile(&mut self, physically_owned: bool) {
        if !physically_owned && let Some(v) = self.value.first_mut() {
            v.bank.retire_active();
            v.pending.remove_retired();
            v.ink = None;
            v.ui = None;
            v.physical = None;
        }
    }
    pub(super) fn observe(
        &mut self,
        raw: RawSample,
        route: Route,
        owner: Owner,
    ) -> std::result::Result<(), Invalid> {
        let Some(v) = self.value.first_mut() else {
            return Ok(());
        };
        if raw.provenance == Provenance::Predicted || route == Route::Suppressed {
            return Ok(());
        }
        let held = raw.buttons.0 & (Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER) != 0
            || matches!(raw.phase, Phase::Down | Phase::Move);
        if raw.provenance == Provenance::Actual {
            v.physical = held.then_some(raw.identity);
        }
        let active = match route {
            Route::Canvas => &mut v.ink,
            Route::Ui | Route::System => &mut v.ui,
            Route::Suppressed => unreachable!(),
        };
        let begin = raw.provenance == Provenance::Actual
            && match route {
                Route::Canvas => raw.phase == Phase::Down,
                _ => held && active.is_none(),
            };
        if begin {
            if active.is_some() {
                return Err(Invalid::Lifecycle);
            }
            let lease = v.bank.begin(Key {
                installation: raw.time.epoch,
                identity: raw.identity,
                begin: raw.sequence,
                target: if route == Route::Canvas {
                    Target::Ink
                } else {
                    Target::Ui
                },
            })?;
            v.bank.bind_owner(&lease, owner);
            *active = Some(Active {
                lease,
                position: raw.logical_position,
            });
        }
        if let Some(contact) = active {
            if contact.lease.key.identity != raw.identity {
                return Err(Invalid::Owner);
            }
            v.pending.insert(&contact.lease)?;
            contact.position = raw.logical_position.or(contact.position);
            if raw.provenance == Provenance::Actual
                && (raw.phase == Phase::Up && route == Route::Canvas || !held)
            {
                v.bank.complete(&contact.lease);
                *active = None;
            }
        }
        Ok(())
    }
    pub(super) fn retire_active(
        &mut self,
    ) -> std::result::Result<[Option<NativePenCancellation>; 32], Invalid> {
        let v = self.value.first_mut().ok_or(Invalid::Owner)?;
        let result = v.bank.retire_active().map(|token| {
            token.map(|token| {
                let position = [&v.ink, &v.ui]
                    .into_iter()
                    .flatten()
                    .find(|active| token.matches(&active.lease))
                    .and_then(|active| active.position);
                NativePenCancellation { token, position }
            })
        });
        for active in [&mut v.ink, &mut v.ui] {
            if let Some(contact) = active.take() {
                v.pending.remove(&contact.lease);
            }
        }
        v.physical = None;
        Ok(result)
    }
    pub(super) fn active_identity(&self) -> Option<Identity> {
        let v = self.value.first()?;
        v.physical
    }
}
#[cfg(test)]
mod tests;
impl Source {
    /// Sealed active presenter lookup. Mutable frame payload cannot turn an
    /// unrelated hover or completed interval into current pointer ownership.
    pub fn native_pen_active_target(&self, guard: &PublicationGuard) -> Option<NativePenTarget> {
        if guard.source.as_ptr() != Arc::as_ptr(&self.0) {
            return None;
        }
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let leases = guard.native_contacts.as_ref()?;
        let resident = state.native_contacts.value.first()?;
        match resident.bank.active_target(leases)? {
            Target::Ink => Some(NativePenTarget::Ink),
            Target::Ui => Some(NativePenTarget::Ui),
        }
    }
    /// No public identity/ordinal selector: Source selects its own exact held
    /// lifetime, fences that physical tail and preserves completed authority.
    pub fn retire_active_native_pen(
        &self,
    ) -> std::result::Result<[Option<NativePenCancellation>; 32], Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let identity = state.native_contacts.active_identity();
        let retired = state.native_contacts.retire_active()?;
        if let Some(identity) = identity {
            state.native.deny(identity);
            state.receiver.retire_active_pen();
        }
        Ok(retired)
    }
}
