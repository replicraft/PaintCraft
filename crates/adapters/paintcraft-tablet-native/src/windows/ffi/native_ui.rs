//! Exclusive source-side keyboard/IMM acquisition. No GUI target is resolved
//! here and no buffered winit WindowEvent is used as an origin witness.
use super::*;
use crate::native_ui::{
    ImeComponent, MAX_CALLBACK_RECORDS, NativeUi, State as UiState, UiFault, WindowsCharacter,
    WindowsCharacterKind, WindowsKey, validate_snapshot,
};
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Shell::DefSubclassProc;
mod ime;
mod keyboard;
#[cfg(test)]
mod tests;

pub(super) struct Capture {
    state: UiState,
    records: Vec<NativeUi>,
    result: Vec<u16>,
    preedit: Vec<u16>,
    attributes: Vec<u8>,
    preedit_clauses: Vec<u32>,
    result_clauses: Vec<u32>,
    // All buffers are destroyed before their admission is released.
    _account: BudgetAccount,
}
impl Capture {
    pub fn new(budget: &SharedBudget) -> Result<Self> {
        let mut account = budget.account(AllocationClass::PaintMetadata);
        let estimate = MAX_CALLBACK_RECORDS
            * (size_of::<NativeUi>()
                + 2 * size_of::<u16>()
                + size_of::<u8>()
                + 2 * size_of::<u32>());
        account.resize(estimate as u64)?;
        fn buffer<T: Clone>(value: T) -> Result<Vec<T>> {
            let mut out = Vec::new();
            out.try_reserve_exact(MAX_CALLBACK_RECORDS)
                .map_err(|_| error("Native UI storage refused"))?;
            out.resize(MAX_CALLBACK_RECORDS, value);
            Ok(out)
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(MAX_CALLBACK_RECORDS)
            .map_err(|_| error("Native UI record storage refused"))?;
        let result = buffer(0u16)?;
        let preedit = buffer(0u16)?;
        let attributes = buffer(0u8)?;
        let preedit_clauses = buffer(0u32)?;
        let result_clauses = buffer(0u32)?;
        let bytes = records.capacity() * size_of::<NativeUi>()
            + (result.capacity() + preedit.capacity()) * size_of::<u16>()
            + attributes.capacity() * size_of::<u8>()
            + (preedit_clauses.capacity() + result_clauses.capacity()) * size_of::<u32>();
        account.resize(bytes as u64)?;
        Ok(Self {
            state: UiState::default(),
            records,
            result,
            preedit,
            attributes,
            preedit_clauses,
            result_clauses,
            _account: account,
        })
    }
    fn push(&mut self, record: NativeUi) -> Result<()> {
        if self.records.len() == MAX_CALLBACK_RECORDS {
            return Err(error("Native UI callback record limit"));
        }
        self.records.push(record);
        Ok(())
    }
    fn acquire(
        &mut self,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<bool> {
        self.records.clear();
        match message {
            WM_IME_STARTCOMPOSITION => self.push(NativeUi::ImeBegin {
                composition: self.state.next_composition().map_err(fault)?,
            })?,
            WM_IME_ENDCOMPOSITION => self.push(NativeUi::ImeEnd {
                composition: self.state.composition().map_err(fault)?,
            })?,
            WM_IME_COMPOSITION => ime::acquire(self, hwnd, lparam.0 as u32)?,
            WM_IME_CHAR => {
                self.push(NativeUi::UnsupportedImeCharacter {
                    value: wparam
                        .0
                        .try_into()
                        .map_err(|_| error("IME character overflow"))?,
                    flags: lparam.0 as u32,
                })?;
                return Ok(true);
            }
            _ => self.push(keyboard::record(message, wparam, lparam)?)?,
        }
        Ok(false)
    }
}
fn fault(value: UiFault) -> Error {
    error(match value {
        UiFault::Capacity => "Native UI admission refused",
        UiFault::Order => "Native UI record order invalid",
        UiFault::Text => "Native UI text invalid",
        UiFault::Composition => "Native IME lifetime invalid",
        UiFault::Unsupported => "Native IME channel unsupported",
    })
}
pub(super) fn recognizes(message: u32) -> bool {
    matches!(
        message,
        WM_KEYDOWN
            | WM_KEYUP
            | WM_SYSKEYDOWN
            | WM_SYSKEYUP
            | WM_CHAR
            | WM_SYSCHAR
            | WM_DEADCHAR
            | WM_SYSDEADCHAR
            | WM_UNICHAR
            | WM_SETFOCUS
            | WM_KILLFOCUS
            | WM_INPUTLANGCHANGE
            | WM_IME_STARTCOMPOSITION
            | WM_IME_COMPOSITION
            | WM_IME_ENDCOMPOSITION
            | WM_IME_CHAR
    )
}
/// Called only by the scoped HWND subclass, before the selected winit parser.
/// Source START/END delegate OS cleanup directly; COMPOSITION has one reader.
pub(super) unsafe fn dispatch(
    inner: &Rc<Inner>,
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_UNICHAR && wparam.0 == UNICODE_NOCHAR as usize {
        return LRESULT(1);
    }
    if inner.processing.replace(true) {
        inner.reentered.set(true);
        return unsafe { delegate(hwnd, message, wparam, lparam) };
    }
    let emitter = inner.target.borrow().as_ref().map(|t| t.emitter.clone());
    let acquired = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut target = inner.target.borrow_mut().take()?;
        let window = target._window.clone();
        // A retired gate cannot acquire more IMM leases after a failed release
        // or source refusal. This is a short metadata check, before any SDK read.
        if target.emitter.gate.with_delivery(|health| health) == DeliveryHealth::Failed {
            if inner.active.get() {
                *inner.target.borrow_mut() = Some(target);
            }
            return Some(window);
        }
        target.storage.events.clear();
        let result = (|| {
            let target: &mut Target = &mut target;
            // Source priority and Alt/Win/history provenance precede GUI replay.
            target.fence();
            target.barrier = Barrier::observe()?;
            if message == WM_KILLFOCUS {
                target.mouse_recovery = true;
            }
            let boundary = if message == WM_KILLFOCUS {
                Boundary::FocusLost
            } else if matches!(
                message,
                WM_IME_STARTCOMPOSITION | WM_IME_COMPOSITION | WM_IME_ENDCOMPOSITION | WM_IME_CHAR
            ) {
                Boundary::ImeReceipt
            } else {
                Boundary::Keyboard
            };
            target.storage.events.push(CaptureEvent::Boundary {
                kind: boundary,
                time: None,
            });
            let unsupported = target.ui.acquire(hwnd, message, wparam, lparam)?;
            let idle = target.mouse_idle();
            let envelope = target.storage.events.len() + usize::from(idle.is_some());
            if !unsupported {
                let ui = &mut target.ui;
                ui.state
                    .validate_callback(&ui.records, envelope)
                    .map_err(fault)?;
            } else if envelope + target.ui.records.len() > MAX_CALLBACK_RECORDS {
                return Err(error("Native UI callback record limit"));
            }
            for record in &target.ui.records {
                target.storage.events.push(CaptureEvent::NativeUi(*record));
            }
            // This idle observation followed the actual key/IME snapshot. It
            // cannot release an older physical owner before that source edge.
            if let Some(idle) = idle {
                target.storage.events.push(idle);
            }
            super::super::batch::deliver(&target.emitter, &target.storage.events, false, || {
                !unsupported && !inner.reentered.replace(false) && inner.active.get()
            });
            Ok(())
        })();
        if result.is_err() {
            target.emitter.report.unclassified();
            target.emitter.source_failure();
        }
        target.storage.events.clear();
        target.ui.records.clear();
        if inner.active.get() {
            *inner.target.borrow_mut() = Some(target);
        }
        Some(window)
    }));
    inner.processing.set(false);
    if acquired.is_err()
        && let Some(emitter) = emitter
    {
        emitter.source_failure();
    }
    let _window = acquired.ok().flatten();
    unsafe { delegate(hwnd, message, wparam, lparam) }
}
unsafe fn delegate(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_IME_COMPOSITION | WM_IME_CHAR => LRESULT(0),
        WM_UNICHAR => LRESULT(1),
        WM_IME_STARTCOMPOSITION | WM_IME_ENDCOMPOSITION => unsafe {
            DefWindowProcW(hwnd, message, wparam, lparam)
        },
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}
