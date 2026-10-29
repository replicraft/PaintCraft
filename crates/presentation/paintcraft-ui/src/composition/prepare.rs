//! Resolve an already-owned IME prefix before any widget sees the raw batch.
//! egui's whole-batch clicked-elsewhere logic can otherwise drop a preceding
//! Commit, or let a newly focused widget consume that old owner's result.
use super::*;
impl Field {
    /// The host must call this before ALL widgets, after refreshing current
    /// document/modal ownership. It never chooses a target from current focus.
    pub fn prepare_boundary(&mut self, ctx: &egui::Context, owner: Owner, draft: &mut String) {
        let (Some(active), Some(id)) = (&self.active, self.mounted_id) else {
            return;
        };
        let current = ctx.cumulative_frame_nr();
        let window_lost = ctx.input(|i| {
            i.events
                .iter()
                .any(|event| matches!(event, Event::WindowFocused(false)))
        });
        let valid = active.owner == owner
            && self
                .last_frame
                .is_some_and(|frame| current == frame || frame.checked_add(1) == Some(current))
            && (window_lost || ctx.memory(|m| m.has_focus(id) || m.had_focus_last_frame(id)))
            && ctx.input(|i| {
                i.focused
                    || i.events
                        .iter()
                        .any(|event| matches!(event, Event::WindowFocused(false)))
            });
        if !valid {
            self.retire(ctx);
            return;
        }
        let boundary = ctx.input(|i| {
            i.events.iter().position(|event| match event {
                Event::PointerButton { pressed: true, .. } | Event::MiddleClickPaste { .. } => true,
                Event::Touch {
                    phase: egui::TouchPhase::Start,
                    ..
                } => true,
                Event::Key {
                    key: egui::Key::Tab | egui::Key::Enter | egui::Key::Escape,
                    pressed: true,
                    ..
                } => true,
                Event::WindowFocused(false) => true,
                Event::AccessKitActionRequest(request) => {
                    request.action == egui::accesskit::Action::Focus
                        && request.target_node != id.accesskit_id()
                }
                _ => false,
            })
        });
        let Some(cut) = boundary else {
            return;
        };
        // Opaque ordinary text/selection edits cannot be replayed by the IME
        // reducer. Refuse their IME part rather than infer an editable result.
        let opaque = ctx.input(|i| {
            i.events[..cut].iter().any(|event| match event {
                Event::Ime(_) | Event::ModifiersChanged(_) | Event::PointerMoved(_) => false,
                Event::Key { key, modifiers, .. } => match key {
                    egui::Key::H | egui::Key::K | egui::Key::U | egui::Key::W => modifiers.ctrl,
                    egui::Key::Z | egui::Key::Y => {
                        modifiers.command || modifiers.mac_cmd || modifiers.ctrl
                    }
                    egui::Key::Backspace | egui::Key::Delete => true,
                    _ => false,
                },
                _ => true,
            })
        });
        let admitted = if opaque {
            false
        } else {
            let active = self.active.as_mut().expect("validated active binding");
            let next = ctx.input(|i| {
                let mut next = self.bound;
                next.observe(active.working.capacity(), &i.events[..cut])
                    .map(|()| next)
            });
            next.and_then(|next| active.snapshot.admit_observed(&mut self.bound, next))
                .is_ok()
        };
        if admitted {
            let active = self.active.as_mut().expect("validated active binding");
            let mut reduced = Reduced {
                text: active.working.clone(),
                cursor: TextEditState::load(ctx, id)
                    .and_then(|s| s.cursor.char_range())
                    .unwrap_or(active.baseline_cursor),
                composing: active.composing,
                result: None,
            };
            // Borrow only for the pure reducer; no budget callbacks or GUI work
            // occurs while the Context input lock is held.
            ctx.input(|i| {
                for event in &i.events[..cut] {
                    if let Event::Ime(event) = event {
                        reduced.apply(event);
                    }
                }
            });
            if let Some((result, cursor)) = reduced.result {
                if active
                    .snapshot
                    .checkpoint(
                        &mut self.bound,
                        (active.baseline_cursor, &active.baseline),
                        (cursor, &result),
                    )
                    .is_ok()
                {
                    draft.clone_from(&result);
                    active.snapshot.set_cursor(cursor);
                    self.completed_owner = Some(owner);
                } else {
                    self.notice = true;
                }
            }
        } else {
            self.notice = true;
        }
        let finish_enter = ctx.input(|i| {
            matches!(i.events.get(cut), Some(Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                modifiers,
                ..
            }) if modifiers.matches_logically(egui::Modifiers::NONE))
        });
        // Binding and cut were established before widgets. Consume every old
        // owner's IME record in that prefix, including provisional-only tails;
        // post-boundary records remain for the later focused widget.
        ctx.input_mut(|i| {
            let mut index = 0;
            i.events.retain(|event| {
                let keep = index >= cut || !matches!(event, Event::Ime(_));
                index += 1;
                keep
            });
        });
        self.retire(ctx);
        self.retired_frame = Some(current);
        // Preserve Enter-to-finish after resolving the earlier owned result;
        // the retired widget does not replay this batch's text events itself.
        if finish_enter {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
    }
}
