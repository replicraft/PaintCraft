//! Bounds apply only to a widget owned from its first fresh TextEditState.
//! See the pinned Undoer redo-cycle control: default max_undos is not a hard cap.
use egui::{Event, ImeEvent, Key};
use paintcraft_core::types::{Error, ErrorCode, Result};

/// Lifetime accounting, never reset while the corresponding widget state lives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Bound {
    bytes: u64,
    shortcuts: u64,
    checkpoints: u64,
}
impl Bound {
    pub fn observe(&mut self, text_bytes: usize, events: &[Event]) -> Result<()> {
        let mut next = *self;
        next.bytes = next.bytes.max(text_bytes as u64);
        for event in events {
            let insertion = match event {
                Event::Text(text) | Event::Paste(text) | Event::MiddleClickPaste { text, .. } => {
                    text.len()
                }
                Event::Ime(ImeEvent::Preedit { text, .. } | ImeEvent::Commit(text)) => text.len(),
                Event::AccessKitActionRequest(request) => match &request.data {
                    Some(egui::accesskit::ActionData::Value(text)) => text.len(),
                    _ => 0,
                },
                _ => 0,
            };
            // A history restore earlier in the same batch can restore the old
            // highwater before another insertion. Using current buffer len alone
            // would therefore underprice this frame.
            next.bytes = checked(next.bytes.checked_add(insertion as u64))?;
            if matches!(
                event,
                Event::Key {
                    key: Key::Z | Key::Y,
                    pressed: true,
                    ..
                }
            ) {
                // Count all Z/Y presses, even without the undo modifier. This is
                // an upper count independent of OS logical modifier matching.
                next.shortcuts = checked(next.shortcuts.checked_add(1))?;
            }
        }
        *self = next;
        Ok(())
    }
    pub fn checkpoints(&mut self, count: u64) -> Result<()> {
        self.checkpoints = checked(self.checkpoints.checked_add(count))?;
        Ok(())
    }
    pub fn snapshots(self) -> Result<u64> {
        checked(
            101_u64
                .checked_add(self.shortcuts)
                .and_then(|n| n.checked_add(self.checkpoints)),
        )
    }
    /// Logical requested bytes for live history plus its owned backup. Selected
    /// Rust clones String/Vec/VecDeque at source length, not source capacity.
    /// Include both containers' small initial capacity allowances. This excludes
    /// allocator metadata/RSS and the caller's actual base/working String caps.
    pub fn text_bytes(self) -> u64 {
        self.bytes
    }
    pub fn history_bytes(self) -> Result<u64> {
        let n = self.snapshots()?;
        let strings = checked(n.checked_mul(self.bytes).and_then(|n| n.checked_mul(2)))?;
        let slots = checked(n.checked_mul(3).and_then(|n| n.checked_add(8)))?;
        let containers = checked(
            slots.checked_mul(std::mem::size_of::<(egui::text::CCursorRange, String)>() as u64),
        )?;
        let inline = 2 * std::mem::size_of::<
            egui::util::undoer::Undoer<(egui::text::CCursorRange, String)>,
        >() as u64;
        // feed_state/undo/redo arguments and flux replacement can temporarily
        // coexist with the history and backup being counted above.
        checked(
            strings
                .checked_add(containers)
                .and_then(|n| n.checked_add(inline))
                .and_then(|n| {
                    self.bytes
                        .checked_mul(4)
                        .and_then(|scratch| n.checked_add(scratch))
                }),
        )
    }
}
fn checked(value: Option<u64>) -> Result<u64> {
    value.ok_or_else(|| {
        Error::new(
            ErrorCode::ResourceLimit,
            "composition history bound overflow",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn includes_sameframe_restore_then_insertion_and_redo_growth() {
        let mut bound = Bound::default();
        bound.observe(1024, &[]).unwrap();
        let key = Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        bound
            .observe(1, &[key.clone(), key, Event::Text("added".into())])
            .unwrap();
        assert_eq!(bound.bytes, 1029);
        assert_eq!(bound.snapshots().unwrap(), 103);
        let before = bound.history_bytes().unwrap();
        bound.checkpoints(2).unwrap();
        assert_eq!(bound.snapshots().unwrap(), 105);
        assert!(bound.history_bytes().unwrap() > before);
    }
    #[test]
    fn overflow_keeps_prior_observation_and_never_wraps() {
        let mut bound = Bound {
            bytes: u64::MAX,
            ..Default::default()
        };
        assert!(bound.observe(0, &[Event::Text("x".into())]).is_err());
        assert_eq!(bound.bytes, u64::MAX);
        assert_eq!(bound.shortcuts, 0);
        assert!(bound.history_bytes().is_err());
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;
    #[test]
    fn middle_click_paste_prices_insertions_even_before_focus() {
        let mut bound = Bound::default();
        bound.observe(4096, &[]).unwrap();
        bound
            .observe(
                1,
                &[Event::MiddleClickPaste {
                    pos: egui::Pos2::ZERO,
                    text: "pasted".into(),
                }],
            )
            .unwrap();
        assert_eq!(bound.text_bytes(), 4102);
    }
}
