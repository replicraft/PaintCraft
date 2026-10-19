//! Native font discovery is asynchronous; text presentation stays frontend-owned.
use super::*;
use paintcraft_platform::fonts::FontCatalog;

#[derive(Debug)]
pub(crate) struct FontState {
    pub families: Vec<String>,
    pub search: String,
    changed: bool,
    pending: Option<std::sync::mpsc::Receiver<FontCatalog>>,
}

impl Default for FontState {
    fn default() -> Self {
        Self {
            families: vec!["Selawik".into()],
            search: String::new(),
            changed: false,
            pending: None,
        }
    }
}

impl App {
    pub(super) fn initialize_fonts(&mut self, ctx: &egui::Context) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let wake = ctx.clone();
        match std::thread::Builder::new()
            .name("paintcraft-font-catalog".into())
            .spawn(move || {
                let _ = sender.send(FontCatalog::system());
                wake.request_repaint();
            }) {
            Ok(_) => self.fonts.pending = Some(receiver),
            Err(e) => self
                .editor
                .set_status(format!("Installed fonts unavailable: {e}")),
        }
    }

    pub(super) fn poll_fonts(&mut self) {
        let result = self
            .fonts
            .pending
            .as_ref()
            .map(|pending| pending.try_recv());
        if let Some(Ok(catalog)) = result {
            self.fonts.pending = None;
            self.fonts.families = catalog.families;
            if !self.fonts.families.iter().any(|name| name == "Selawik") {
                self.fonts.families.push("Selawik".into());
            }
            self.fonts.families.sort_by_key(|name| name.to_lowercase());
        } else if matches!(
            result,
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected))
        ) {
            self.fonts.pending = None;
            self.editor
                .set_status("Installed font discovery stopped; using Selawik");
        }
    }

    pub(super) fn classic_text_options(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.text.clone();
        paintcraft_classic::text_options(
            ui,
            &mut self.settings.text,
            &self.fonts.families,
            &mut self.fonts.search,
            self.fonts.pending.is_some(),
        );
        self.fonts.changed |= before != self.settings.text;
        if self.fonts.changed
            && ui.ctx().current_pass_index() == 0
            && !ui.input(|input| input.pointer.any_down())
        {
            self.fonts.changed = false;
            self.persist_settings();
        }
    }
}
