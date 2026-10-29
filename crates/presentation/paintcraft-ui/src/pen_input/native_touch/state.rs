use super::*;
use paintcraft_input::tablet::{DeviceKind, receiver::CaptureStatus};
const CONTACTS: usize = 32;
const CLOCKS: usize = 64;
// Conservative retained allowance per preallocated Arc validity cell. Allocator
// metadata/process RSS are outside this logical admission, as for Source itself.
const TOKEN_ALLOWANCE: usize =
    std::mem::size_of::<Cell>() + 2 * std::mem::size_of::<usize>() + std::mem::align_of::<Cell>()
        - 1;
pub(crate) const BANK_BYTES: usize = CONTACTS * TOKEN_ALLOWANCE;
#[derive(Clone, Copy, Debug)]
struct ClockRecord {
    identity: Identity,
    time: Time,
    ordinal: u64,
}
#[derive(Clone, Copy, Debug)]
struct Contact {
    identity: Identity,
    held: bool,
    denied: bool,
    primary: bool,
    slot: Option<usize>,
}
#[derive(Debug)]
struct Slot {
    valid: Arc<Cell>,
    key: Option<Key>,
    target: NativeTouchTarget,
    primary: bool,
    context: Option<CaptureContext>,
    position: Option<(f64, f64)>,
}
#[derive(Debug)]
pub(crate) struct Metadata {
    contacts: [Option<Contact>; CONTACTS],
    clocks: [Option<ClockRecord>; CLOCKS],
    slots: [Slot; CONTACTS],
    installation: Option<u64>,
    last_ordinal: Option<u64>,
    pub(super) frames: BudgetScope,
}
// Fixed-size handoff carries its admitted capture context inline; a Box here
// would allocate inside native delivery. The larger public batch is charged.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(super) enum Change {
    Suppressed,
    Admitted {
        lease: Lease,
        position: Option<(f64, f64)>,
        target: NativeTouchTarget,
        primary: bool,
        context: CaptureContext,
    },
    Cancelled(NativeTouchCancellation),
}
pub(crate) fn storage_bytes() -> usize {
    std::mem::size_of::<Metadata>()
}
impl Metadata {
    pub(crate) fn new(frames: BudgetScope, bank: BudgetScope) -> Result<Self> {
        let mut error = None;
        // Fixed stack construction: never allocate a cell unless its individual
        // declaration succeeded inside the already-reserved bank scope.
        let cells: [Option<Slot>; CONTACTS] = std::array::from_fn(|_| {
            let mut storage = bank.account();
            if let Err(e) = storage.resize(TOKEN_ALLOWANCE as u64) {
                error = Some(e);
                return None;
            }
            Some(Slot {
                valid: Arc::new(Cell {
                    alive: AtomicBool::new(false),
                    _storage: storage,
                }),
                key: None,
                target: NativeTouchTarget::Ui,
                primary: false,
                context: None,
                position: None,
            })
        });
        if let Some(error) = error {
            return Err(error);
        }
        Ok(Self {
            contacts: [None; CONTACTS],
            clocks: [None; CLOCKS],
            slots: cells.map(|cell| cell.expect("all token admissions succeeded")),
            installation: None,
            last_ordinal: None,
            frames,
        })
    }
    pub(crate) fn active(&self) -> bool {
        self.contacts.iter().flatten().any(|c| {
            c.held
                && !c.denied
                && c.slot
                    .is_some_and(|i| self.slots[i].valid.alive.load(Ordering::Acquire))
        })
    }
    pub(crate) fn fence_all(&mut self) {
        for slot in &self.slots {
            slot.valid.alive.store(false, Ordering::Release);
        }
        for c in self.contacts.iter_mut().flatten() {
            c.denied |= c.held;
        }
    }
    pub(crate) fn fence_canvas(&mut self) {
        for slot in &self.slots {
            if slot.target == NativeTouchTarget::Canvas {
                slot.valid.alive.store(false, Ordering::Release);
            }
        }
        for c in self.contacts.iter_mut().flatten() {
            if c.slot
                .is_some_and(|i| self.slots[i].target == NativeTouchTarget::Canvas)
            {
                c.denied |= c.held;
            }
        }
    }
    pub(super) fn deny(&mut self, identity: Identity) {
        for c in self
            .contacts
            .iter_mut()
            .flatten()
            .filter(|c| c.identity == identity)
        {
            c.denied |= c.held;
            if let Some(i) = c.slot {
                self.slots[i].valid.alive.store(false, Ordering::Release);
            }
        }
    }
    fn cancel_slot(
        &mut self,
        i: usize,
        cause: Option<NativeFingerMetadata>,
    ) -> Option<NativeTouchCancellation> {
        let slot = &self.slots[i];
        if !slot.valid.alive.swap(false, Ordering::AcqRel) {
            return None;
        }
        Some(NativeTouchCancellation {
            lease: Lease {
                key: slot.key?,
                valid: slot.valid.clone(),
            },
            cause,
            position: slot.position,
            target: slot.target,
            primary: slot.primary,
        })
    }
    pub(super) fn retire_all(&mut self) -> [Option<NativeTouchCancellation>; CONTACTS] {
        let result = std::array::from_fn(|i| self.cancel_slot(i, None));
        self.fence_all();
        result
    }
    pub(super) fn retire_active(&mut self) -> [Option<NativeTouchCancellation>; CONTACTS] {
        let result = std::array::from_fn(|i| {
            let active = self
                .contacts
                .iter()
                .flatten()
                .any(|contact| contact.held && !contact.denied && contact.slot == Some(i));
            active.then(|| self.cancel_slot(i, None)).flatten()
        });
        for contact in self.contacts.iter_mut().flatten() {
            // A completed slot may still have opaque guards outside Source.
            // Only this currently held contact becomes a denied physical tail.
            if contact.held && contact.slot.is_some_and(|i| result[i].is_some()) {
                contact.denied = true;
            }
        }
        result
    }
    pub(super) fn retire_identity(
        &mut self,
        identity: Identity,
        installation: u64,
        ordinal: u64,
        time: Time,
    ) -> std::result::Result<(), Invalid> {
        if self
            .contacts
            .iter()
            .flatten()
            .any(|c| c.identity == identity && (c.held || c.denied))
        {
            return Err(Invalid::Owner);
        }
        let i = self
            .clocks
            .iter()
            .position(|v| v.is_some_and(|v| v.identity == identity))
            .ok_or(Invalid::Owner)?;
        self.validate_clock(NativeFingerMetadata {
            installation,
            ordinal,
            identity,
            phase: Phase::Leave,
            time,
            provenance: Provenance::Actual,
            confidence: FingerConfidence::Unspecified,
            in_contact: false,
            primary: false,
        })?;
        self.clocks[i] = None;
        self.last_ordinal = Some(ordinal);
        Ok(())
    }
    fn validate_clock(&self, m: NativeFingerMetadata) -> std::result::Result<(), Invalid> {
        if m.identity.kind != DeviceKind::Finger
            || m.installation == 0
            || m.installation == u64::MAX
            || m.time.epoch != m.installation
            || m.ordinal == u64::MAX
            || self.installation.is_some_and(|i| i != m.installation)
        {
            return Err(Invalid::Owner);
        }
        if self.last_ordinal.is_some_and(|old| m.ordinal <= old) {
            return Err(Invalid::Order);
        }
        if self.clocks.iter().flatten().any(|v| {
            v.identity == m.identity
                && (v.time.clock != m.time.clock
                    || v.time.epoch != m.time.epoch
                    || v.time.micros > m.time.micros
                    || v.ordinal >= m.ordinal)
        }) {
            return Err(Invalid::Clock);
        }
        Ok(())
    }
    fn observe_clock(&mut self, m: NativeFingerMetadata) -> std::result::Result<(), Invalid> {
        self.validate_clock(m)?;
        let i = self
            .clocks
            .iter()
            .position(|v| v.is_some_and(|v| v.identity == m.identity))
            .or_else(|| self.clocks.iter().position(Option::is_none))
            .ok_or(Invalid::Capacity)?;
        self.clocks[i] = Some(ClockRecord {
            identity: m.identity,
            time: m.time,
            ordinal: m.ordinal,
        });
        self.installation = Some(m.installation);
        self.last_ordinal = Some(m.ordinal);
        Ok(())
    }
    fn remember(
        &mut self,
        index: Option<usize>,
        contact: Contact,
        terminal: bool,
    ) -> std::result::Result<(), Invalid> {
        if terminal {
            if let Some(i) = index {
                self.contacts[i] = None;
            }
        } else if contact.held || contact.denied || contact.slot.is_some() {
            let i = index
                .or_else(|| self.contacts.iter().position(Option::is_none))
                .ok_or(Invalid::Capacity)?;
            self.contacts[i] = Some(contact);
        }
        Ok(())
    }
    fn context_current(slot: &Slot, current: CaptureContext) -> bool {
        let Some(old) = slot.context else {
            return false;
        };
        old.cancel == current.cancel
            && old.window == current.window
            && old.view.owner.session == current.view.owner.session
            && old.view.owner.interface == current.view.owner.interface
            && current.view.focused
            && old.view.pixels_per_point == current.view.pixels_per_point
            && (slot.target == NativeTouchTarget::Ui
                || (old.canvas == current.canvas
                    && old.view.owner == current.view.owner
                    && !current.view.blocked
                    && !current.view.ime
                    && old.view.transform == current.view.transform
                    && old.view.canvas == current.view.canvas))
    }
    fn continuation_slot(&self, lease: &Lease) -> Option<usize> {
        self.slots.iter().position(|s| {
            s.key == Some(lease.key)
                && Arc::ptr_eq(&s.valid, &lease.valid)
                && s.valid.alive.load(Ordering::Acquire)
        })
    }
    pub(crate) fn validate_continuation(&self, lease: &Lease, proof: Predecessor) -> bool {
        let Some(slot) = self.continuation_slot(lease).map(|i| &self.slots[i]) else {
            return false;
        };
        let Some(context) = slot.context else {
            return false;
        };
        context.view.owner.session == proof.before.session
            && context.view.owner.interface == proof.before.interface
            && (slot.target == NativeTouchTarget::Ui || context.view.owner == proof.before)
    }
    pub(crate) fn acknowledge(&mut self, lease: &Lease, after: Owner) {
        let i = self
            .continuation_slot(lease)
            .expect("validated native touch continuation");
        self.slots[i]
            .context
            .as_mut()
            .expect("admitted touch context")
            .view
            .owner = after;
    }
    pub(super) fn observe(
        &mut self,
        m: NativeFingerMetadata,
        admission: NativeFingerAdmission,
        context: Option<CaptureContext>,
        capture: CaptureStatus,
        convert: impl FnOnce(f64) -> std::result::Result<Option<(f64, f64)>, Invalid>,
    ) -> std::result::Result<Change, Invalid> {
        let view = context.map(|c| c.view);
        if m.provenance == Provenance::Predicted {
            return Ok(Change::Suppressed);
        }
        if m.provenance == Provenance::Boundary {
            return Err(Invalid::Order);
        }
        let index = self
            .contacts
            .iter()
            .position(|c| c.is_some_and(|c| c.identity == m.identity));
        let previous = index.and_then(|i| self.contacts[i]);
        if m.provenance == Provenance::Coalesced {
            if !matches!(m.phase, Phase::Move | Phase::Hover)
                || (m.phase == Phase::Move && !m.in_contact)
                || (m.phase == Phase::Hover && m.in_contact)
                || (previous.is_none() && m.in_contact)
                || previous.is_some_and(|c| {
                    c.held != m.in_contact
                        || c.primary != m.primary
                        || (c
                            .slot
                            .is_some_and(|i| self.slots[i].valid.alive.load(Ordering::Acquire))
                            && m.confidence != FingerConfidence::High)
                })
            {
                return Err(Invalid::Order);
            }
            self.observe_clock(m)?;
            // Retain only geometry under an already actual-admitted lifetime.
            // History never updates held/primary/confidence/contact authority.
            if admission == NativeFingerAdmission::Candidate
                && let (Some(v), Some(contact)) = (view.filter(|v| v.focused), previous)
                && !capture.failed
                && !contact.denied
                && let Some(i) = contact
                    .slot
                    .filter(|i| self.slots[*i].valid.alive.load(Ordering::Acquire))
                && context.is_some_and(|current| Self::context_current(&self.slots[i], current))
            {
                let position =
                    checked_position(convert(v.pixels_per_point)?)?.ok_or(Invalid::Coordinates)?;
                let slot = &mut self.slots[i];
                slot.position = Some(position);
                return Ok(Change::Admitted {
                    lease: Lease {
                        key: slot.key.ok_or(Invalid::Owner)?,
                        valid: slot.valid.clone(),
                    },
                    position: Some(position),
                    target: slot.target,
                    primary: slot.primary,
                    context: slot.context.ok_or(Invalid::Owner)?,
                });
            }
            return Ok(Change::Suppressed);
        }
        if (matches!(m.phase, Phase::Down | Phase::Move) && !m.in_contact)
            || (matches!(m.phase, Phase::Up | Phase::Hover) && m.in_contact)
        {
            return Err(Invalid::Order);
        }
        self.observe_clock(m)?;
        let terminal = !m.in_contact
            && matches!(
                m.phase,
                Phase::Up | Phase::Hover | Phase::Leave | Phase::Cancel
            );
        let mut contact = previous.unwrap_or(Contact {
            identity: m.identity,
            held: false,
            denied: false,
            primary: m.primary,
            slot: None,
        });
        let live_slot = contact
            .slot
            .filter(|i| self.slots[*i].valid.alive.load(Ordering::Acquire));
        let usable = view.is_some_and(|v| v.focused) && !capture.failed;
        let context_current = live_slot.is_none_or(|i| {
            context.is_some_and(|current| Self::context_current(&self.slots[i], current))
        });
        let restriction =
            admission != NativeFingerAdmission::Candidate || !usable || !context_current;
        let withdrawal = live_slot.is_some()
            && (matches!(m.phase, Phase::Cancel | Phase::Leave)
                || (m.in_contact
                    && (m.confidence != FingerConfidence::High || m.primary != contact.primary))
                || (terminal && m.phase != Phase::Up));
        let competing = capture.mouse || capture.pen;
        let mut change = Change::Suppressed;
        if restriction || withdrawal {
            contact.denied |= m.in_contact;
            if let Some(i) = live_slot
                && let Some(cancel) = self.cancel_slot(i, Some(m))
            {
                change = Change::Cancelled(cancel);
            }
        } else if m.phase == Phase::Down && !contact.denied {
            if live_slot.is_some() {
                return Err(Invalid::Order);
            }
            if competing || m.confidence != FingerConfidence::High {
                contact.denied = true;
            } else {
                let v = view.ok_or(Invalid::Coordinates)?;
                let position =
                    checked_position(convert(v.pixels_per_point)?)?.ok_or(Invalid::Coordinates)?;
                let target = if v.permissions(Some(position)).canvas_hit && !v.blocked && !v.ime {
                    NativeTouchTarget::Canvas
                } else {
                    NativeTouchTarget::Ui
                };
                let i = self
                    .slots
                    .iter()
                    .enumerate()
                    .position(|(i, s)| {
                        Arc::strong_count(&s.valid) == 1
                            && !self.contacts.iter().flatten().any(|c| c.slot == Some(i))
                    })
                    .ok_or(Invalid::Capacity)?;
                let key = Key {
                    installation: m.installation,
                    identity: m.identity,
                    down: m.ordinal,
                };
                let slot = &mut self.slots[i];
                slot.key = Some(key);
                slot.target = target;
                slot.primary = m.primary;
                slot.context = context;
                slot.position = Some(position);
                slot.valid.alive.store(true, Ordering::Release);
                contact.slot = Some(i);
                change = Change::Admitted {
                    lease: Lease {
                        key,
                        valid: slot.valid.clone(),
                    },
                    position: Some(position),
                    target,
                    primary: slot.primary,
                    context: slot.context.ok_or(Invalid::Owner)?,
                };
            }
        } else if let Some(i) = live_slot
            && !contact.denied
        {
            let v = view.ok_or(Invalid::Coordinates)?;
            let position = checked_position(convert(v.pixels_per_point)?)?;
            if position.is_none() {
                if terminal {
                    if let Some(cancel) = self.cancel_slot(i, Some(m)) {
                        change = Change::Cancelled(cancel);
                    }
                } else {
                    return Err(Invalid::Coordinates);
                }
            } else {
                let slot = &mut self.slots[i];
                slot.position = position;
                change = Change::Admitted {
                    lease: Lease {
                        key: slot.key.ok_or(Invalid::Owner)?,
                        valid: slot.valid.clone(),
                    },
                    position,
                    target: slot.target,
                    primary: slot.primary,
                    context: slot.context.ok_or(Invalid::Owner)?,
                };
            }
        } else if m.in_contact && m.phase != Phase::Enter {
            contact.denied = true;
        }
        contact.held = m.in_contact;
        contact.primary = m.primary;
        self.remember(index, contact, terminal)?;
        Ok(change)
    }
}
fn checked_position(
    position: Option<(f64, f64)>,
) -> std::result::Result<Option<(f64, f64)>, Invalid> {
    if position.is_some_and(|(x, y)| {
        !x.is_finite() || !y.is_finite() || !(x as f32).is_finite() || !(y as f32).is_finite()
    }) {
        return Err(Invalid::Coordinates);
    }
    Ok(position)
}
