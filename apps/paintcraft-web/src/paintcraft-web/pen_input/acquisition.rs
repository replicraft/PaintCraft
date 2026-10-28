//! One bounded acquisition counter for actual pen/coalesced and mouse records.
//! Mouse events are owned directly at DOM intake, never guessed from GUI counts.
use super::*;
use paintcraft_input::tablet::Invalid;
use std::collections::VecDeque;
#[derive(Clone, Debug)]
pub(crate) struct MouseRecord {
    pub order: u64,
    pub event: Event,
    pub modifiers: egui::Modifiers,
}
#[derive(Debug)]
pub(crate) struct Acquisition {
    #[cfg(any(test, target_arch = "wasm32"))]
    sequence: u64,
    buttons: u8,
    dirty: bool,
    denied_tail: bool,
    records: VecDeque<MouseRecord>,
}
impl Acquisition {
    pub fn new() -> Result<Self, Invalid> {
        let mut records = VecDeque::new();
        records
            .try_reserve_exact(CAPACITY)
            .map_err(|_| Invalid::Capacity)?;
        Ok(Self {
            #[cfg(any(test, target_arch = "wasm32"))]
            sequence: 0,
            buttons: 0,
            dirty: false,
            denied_tail: false,
            records,
        })
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    fn next(&mut self) -> Result<u64, Invalid> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .filter(|n| *n != u64::MAX)
            .ok_or(Invalid::Order)?;
        Ok(self.sequence)
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    pub fn pen(&mut self, sample: &mut paintcraft_input::tablet::RawSample) -> Result<(), Invalid> {
        sample.sequence = self.next()?;
        self.dirty = true;
        Ok(())
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    pub fn capture_mouse(&mut self, source: &Source, buttons: u8) -> bool {
        if self.denied_tail {
            self.buttons = buttons;
            if buttons == 0 {
                self.denied_tail = false;
            }
            return false;
        }
        let accepted = if buttons != 0 || self.buttons != 0 {
            source.mouse_capture(buttons != 0)
        } else {
            let accepted = source.mouse_capture(true);
            if accepted {
                source.mouse_capture(false);
            }
            accepted
        };
        // Physical held state is independent of who owns GUI delivery. A denied
        // contact may not become a new press merely because the pen later lifts.
        self.buttons = buttons;
        if !accepted && buttons != 0 {
            self.denied_tail = true;
        }
        accepted
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    pub fn mouse(
        &mut self,
        source: &Source,
        event: Event,
        buttons: u8,
        modifiers: egui::Modifiers,
    ) -> Result<bool, Invalid> {
        if !matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. }) {
            return Err(Invalid::Lifecycle);
        }
        if !self.capture_mouse(source, buttons) {
            return Ok(false);
        }
        if self.records.len() == CAPACITY {
            self.reset(source);
            return Err(Invalid::Capacity);
        }
        let order = self.next()?;
        self.records.push_back(MouseRecord {
            order,
            event,
            modifiers,
        });
        self.dirty = true;
        Ok(true)
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    pub fn button_changes(
        &mut self,
        source: &Source,
        previous: u8,
        buttons: u8,
        pos: egui::Pos2,
        modifiers: egui::Modifiers,
    ) -> Result<bool, Invalid> {
        if !self.capture_mouse(source, buttons) {
            return Ok(false);
        }
        let changed = (previous ^ buttons) & 0x1f;
        let count = changed.count_ones() as usize;
        if self.records.len() + count > CAPACITY {
            self.reset(source);
            return Err(Invalid::Capacity);
        }
        if count == 0 {
            return Ok(true);
        }
        let order = self.next()?;
        for (bit, button) in [
            egui::PointerButton::Primary,
            egui::PointerButton::Secondary,
            egui::PointerButton::Middle,
            egui::PointerButton::Extra1,
            egui::PointerButton::Extra2,
        ]
        .into_iter()
        .enumerate()
        {
            if changed & (1 << bit) != 0 {
                self.records.push_back(MouseRecord {
                    order,
                    modifiers,
                    event: Event::PointerButton {
                        pos,
                        button,
                        pressed: buttons & (1 << bit) != 0,
                        modifiers,
                    },
                });
            }
        }
        self.dirty = true;
        Ok(true)
    }
    pub fn declared_bytes() -> usize {
        CAPACITY * std::mem::size_of::<MouseRecord>()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn needs_flush(&self) -> bool {
        self.dirty
    }
    pub fn buttons(&self) -> u8 {
        self.buttons
    }
    pub fn reset(&mut self, source: &Source) {
        self.denied_tail |= self.buttons != 0;
        self.records.clear();
        self.dirty = false;
        source.mouse_capture(false);
    }
    pub fn drain(&mut self, out: &mut Vec<MouseRecord>) {
        out.clear();
        out.extend(self.records.drain(..));
        self.dirty = false;
    }
}
