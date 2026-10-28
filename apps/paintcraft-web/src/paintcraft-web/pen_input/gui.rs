use super::*;
impl WebApp {
    pub(super) fn append_pen_gui(
        &mut self,
        raw: &mut egui::RawInput,
        batch: &Batch,
        ctx: &egui::Context,
    ) -> bool {
        let room =
            2 * (batch.frame.ui.len() + batch.frame.samples.len() + self.pen.mouse_events.len());
        if raw.events.try_reserve_exact(room).is_err() {
            self.retire_pen_host(ctx);
            return false;
        }
        let (mut ui, mut mouse, mut pen) = (0, 0, 0);
        let mut current = ctx.input(|i| i.modifiers);
        for event in &raw.events {
            if let Event::ModifiersChanged(m) = event {
                current = *m;
            }
        }
        loop {
            // None is an explicit cancellation boundary. Actual source zero is
            // Some(0), and the browser's one acquisition counter starts at one.
            let next = [
                batch.frame.ui.get(ui).map(|r| (r.sequence, 0)),
                self.pen.mouse_events.get(mouse).map(|r| (Some(r.order), 1)),
                batch.frame.samples.get(pen).map(|r| (Some(r.sequence), 2)),
            ]
            .into_iter()
            .flatten()
            .min();
            match next {
                Some((_, 0)) => {
                    let record = batch.frame.ui[ui];
                    if record.sequence.is_some() {
                        let m = match record.event {
                            UiEvent::Move { modifiers, .. } | UiEvent::Button { modifiers, .. } => {
                                Some(modifiers)
                            }
                            UiEvent::Gone => None,
                        };
                        if let Some(m) = m {
                            set_modifiers(raw, &mut current, modifiers(m));
                        }
                    }
                    raw.events.push(ui_event(batch.frame.ui[ui].event));
                    ui += 1;
                }
                Some((_, 1)) => {
                    let r = &self.pen.mouse_events[mouse];
                    set_modifiers(raw, &mut current, r.modifiers);
                    raw.events.push(r.event.clone());
                    mouse += 1;
                }
                Some((_, 2)) => {
                    let sample = batch.frame.samples[pen];
                    set_modifiers(raw, &mut current, modifiers(sample.modifiers));
                    if self.pen.gui_button.is_some()
                        || (sample.phase == Phase::Down
                            && !self.editor.tools.pressure_eligible(sample))
                    {
                        if let Some(event) = legacy_event(sample, &mut self.pen.gui_button) {
                            raw.events.push(event);
                        }
                    } else if let Some(
                        Event::PointerMoved(pos) | Event::PointerButton { pos, .. },
                    ) = legacy_event(sample, &mut None)
                    {
                        raw.events.push(Event::PointerMoved(pos));
                    }
                    pen += 1;
                }
                _ => break,
            }
        }
        true
    }
}
fn set_modifiers(raw: &mut egui::RawInput, current: &mut egui::Modifiers, next: egui::Modifiers) {
    if *current != next {
        raw.events.push(Event::ModifiersChanged(next));
        *current = next;
    }
}
