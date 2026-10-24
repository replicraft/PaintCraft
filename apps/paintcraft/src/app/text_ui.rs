//! Native font discovery is asynchronous; text presentation stays frontend-owned.
use super::*;
use paintcraft_platform::fonts::FontCatalog;

#[derive(Debug)]
pub(crate) struct FontState {
    pub families: Vec<String>,
    pub search: String,
    changed: bool,
    pending: Option<std::sync::mpsc::Receiver<(FontCatalog, text::TextRenderer)>>,
    ready: Option<(FontCatalog, text::TextRenderer)>,
}

impl Default for FontState {
    fn default() -> Self {
        Self {
            families: vec!["Selawik".into()],
            search: String::new(),
            changed: false,
            pending: None,
            ready: None,
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
                let renderer = text::system();
                let catalog = FontCatalog {
                    families: text::families(&renderer),
                };
                let _ = sender.send((catalog, renderer));
                wake.request_repaint();
            }) {
            Ok(_) => self.fonts.pending = Some(receiver),
            Err(e) => self
                .editor
                .set_status(format!("Installed fonts unavailable: {e}")),
        }
    }

    pub(super) fn poll_fonts(&mut self) {
        if self.fonts.ready.is_none() {
            let result = self
                .fonts
                .pending
                .as_ref()
                .map(|pending| pending.try_recv());
            match result {
                Some(Ok(result)) => {
                    self.fonts.ready = Some(result);
                    self.fonts.pending = None;
                }
                Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                    self.fonts.pending = None;
                    self.editor
                        .set_status("Installed font discovery stopped; using Selawik");
                }
                _ => {}
            }
        }
        if self.fonts.ready.is_none() {
            return;
        }
        let mut current = match self.text_renderer.try_lock() {
            Ok(current) => current,
            Err(std::sync::TryLockError::WouldBlock) => {
                self.context
                    .request_repaint_after(Duration::from_millis(16));
                return;
            }
            Err(std::sync::TryLockError::Poisoned(error)) => {
                // A newly discovered renderer replaces the failed derived font cache.
                self.text_renderer.clear_poison();
                error.into_inner()
            }
        };
        let (catalog, renderer) = self.fonts.ready.take().expect("completed font discovery");
        *current = renderer;
        self.texture_key = None;
        self.preview_revision += 1;
        self.preview_start_revision = self.preview_revision;
        self.fonts.families = catalog.families;
        if !self.fonts.families.iter().any(|name| name == "Selawik") {
            self.fonts.families.push("Selawik".into());
        }
        self.fonts.families.sort_by_key(|name| name.to_lowercase());
        // Arial ships on Windows and macOS, but may be absent elsewhere.
        // Keep explicit user preferences; only resolve an unavailable default.
        if self.settings.text.family == "Arial"
            && !self.fonts.families.iter().any(|name| name == "Arial")
        {
            self.settings.text.family = "Selawik".into();
        }
    }

    pub(super) fn classic_text_options(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.text.clone();
        paintcraft_classic::text_options(
            ui,
            &mut self.settings.text,
            &self.fonts.families,
            &mut self.fonts.search,
            self.fonts.pending.is_some() || self.fonts.ready.is_some(),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_discovery_waits_without_losing_the_result_when_a_worker_owns_the_font_cache() {
        let mut app = App::new(None);
        let fonts = app.text_renderer.clone();
        let guard = fonts.lock().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        sender
            .send((
                FontCatalog {
                    families: vec!["Example".into()],
                },
                text::bundled(),
            ))
            .unwrap();
        app.fonts.pending = Some(receiver);
        let revision = app.preview_revision;
        app.poll_fonts();
        assert!(app.fonts.ready.is_some());
        assert!(app.fonts.pending.is_none());
        assert_eq!(app.preview_revision, revision);
        drop(guard);
        app.poll_fonts();
        assert!(app.fonts.ready.is_none());
        assert_eq!(app.fonts.families, vec!["Example", "Selawik"]);
        assert_eq!(app.preview_revision, revision + 1);
        app.poll_fonts();
        assert_eq!(app.preview_revision, revision + 1);
    }
    #[test]
    fn installed_arial_is_the_default_without_overwriting_custom_font_preferences() {
        for (selected, installed, expected) in [
            ("Arial", vec!["Arial".into()], "Arial"),
            ("Arial", vec!["Example".into()], "Selawik"),
            ("Example", vec!["Arial".into(), "Example".into()], "Example"),
        ] {
            let mut app = App::new(None);
            app.settings.text.family = selected.into();
            app.fonts.ready = Some((
                FontCatalog {
                    families: installed,
                },
                text::bundled(),
            ));
            app.poll_fonts();
            assert_eq!(app.settings.text.family, expected);
        }
    }
}
