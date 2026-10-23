//! Keyboard browsing stays staged while the search field retains focus.
use egui::{Id, Key, Modifiers, Ui};

#[derive(Clone, Default)]
pub(super) struct Navigation {
    pub candidate: Option<String>,
}

impl Navigation {
    pub fn index(&self, filtered: &[&String]) -> Option<usize> {
        filtered
            .iter()
            .position(|name| Some(name.as_str()) == self.candidate.as_deref())
    }

    pub fn reset(&mut self, filtered: &[&String], preferred: Option<&str>) {
        self.candidate = preferred
            .and_then(|preferred| filtered.iter().find(|name| name.as_str() == preferred))
            .or_else(|| filtered.first())
            .map(|name| (*name).clone());
    }

    pub fn move_by(&mut self, filtered: &[&String], key: Key, page: usize) {
        if filtered.is_empty() {
            self.candidate = None;
            return;
        }
        let current = self.index(filtered).unwrap_or(0);
        let next = match key {
            Key::ArrowDown => current.saturating_add(1),
            Key::ArrowUp => current.saturating_sub(1),
            Key::PageDown => current.saturating_add(page),
            Key::PageUp => current.saturating_sub(page),
            _ => current,
        }
        .min(filtered.len() - 1);
        self.candidate = Some(filtered[next].clone());
    }
}

pub(super) fn take_key(ui: &mut Ui, search_id: Id) -> Option<Key> {
    if ui.ctx().current_pass_index() != 0 {
        return None;
    }
    let ime_id = search_id.with("navigation-ime");
    if !ui.memory(|memory| memory.has_focus(search_id)) {
        ui.data_mut(|data| data.insert_temp(ime_id, false));
        return None;
    }
    let mut composing = ui.data(|data| data.get_temp::<bool>(ime_id).unwrap_or(false));
    let was_composing = composing;
    let ime_event = ui.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(_)))
    });
    ui.input(|input| {
        for event in &input.events {
            match event {
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    composing = !text.is_empty()
                }
                egui::Event::Ime(egui::ImeEvent::Commit(_)) => composing = false,
                _ => {}
            }
        }
    });
    ui.data_mut(|data| data.insert_temp(ime_id, composing));
    if composing || was_composing || ime_event {
        // Platform IME handles these keys; its preedit/commit events still reach
        // TextEdit. Do not move its composition cursor or accept the font.
        ui.input_mut(|input| {
            for key in [
                Key::ArrowDown,
                Key::ArrowUp,
                Key::PageDown,
                Key::PageUp,
                Key::Enter,
            ] {
                input.consume_key(Modifiers::NONE, key);
            }
        });
        return None;
    }
    ui.input_mut(|input| {
        [
            Key::ArrowDown,
            Key::ArrowUp,
            Key::PageDown,
            Key::PageUp,
            Key::Enter,
        ]
        .into_iter()
        .find(|key| input.consume_key(Modifiers::NONE, *key))
    })
}
