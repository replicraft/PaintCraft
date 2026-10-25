//! Desktop appearance integration.

use super::*;
use paintcraft_ui::i18n::{Language, set_language};

impl App {
    /// Load saved settings from `dir`. With no saved Classic variant the first-visit
    /// picker is shown, with the default (v11) selected.
    pub fn with_settings(mut self, dir: PathBuf) -> App {
        let (settings, loaded) = Settings::load(&dir);
        if let Loaded::Unusable { reason, moved_to } = loaded {
            let kept = moved_to.map_or_else(String::new, |p| format!(" (kept as {})", p.display()));
            self.editor.set_status(format!(
                "Settings were unusable ({reason}){kept}; using defaults"
            ));
        }
        if self.editor.path().is_none() && !self.editor.is_dirty() && self.document_count() == 1 {
            let size = match settings.experience {
                Experience::Classic => settings.canvas.new_size(),
                Experience::Studio => STUDIO_SIZE,
            };
            if self.editor.document().canvas != size {
                match Editor::new_document(size.0, size.1) {
                    Ok(mut editor) => {
                        editor.select_tool(self.editor.tool);
                        self.publish_editor(editor);
                    }
                    Err(e) => self
                        .editor
                        .set_status(format!("Cannot create preferred canvas: {e}")),
                }
            }
        }
        self.picking = (settings.experience == Experience::Classic
            && settings.classic_variant.is_none())
        .then_some(ClassicVariant::DEFAULT);
        settings.studio_layout.restore_visibility(&mut self.studio);
        self.settings = settings;
        let opened = self.recent.entries.clone();
        self.recent = recent::History::load(dir.clone());
        for path in opened.iter().rev() {
            self.recent.remember(path);
        }
        self.settings_dir = Some(dir);
        self
    }

    /// The Classic variant in effect (the default until one is chosen).
    pub fn variant(&self) -> ClassicVariant {
        self.settings
            .classic_variant
            .unwrap_or(ClassicVariant::DEFAULT)
    }

    pub fn experience(&self) -> Experience {
        self.settings.experience
    }

    /// Also available before an egui context exists, for native startup messages.
    pub fn language(&self) -> Language {
        self.settings.language
    }

    pub(super) fn choose_experience(&mut self, experience: Experience) {
        if self.settings.experience == experience {
            return;
        }
        let ctx = self.context.clone();
        self.resolve_tab_input(&ctx);
        if self.any_dirty_documents()
            || self
                .saving
                .as_ref()
                .is_some_and(|s| s.pending() || s.failed())
        {
            self.confirm = Some(Confirm::SwitchInterface(experience));
        } else {
            self.begin_interface_switch(experience, &ctx);
        }
    }

    pub(super) fn adopt_experience(&mut self, experience: Experience) {
        let size = match experience {
            Experience::Classic => self.settings.canvas.new_size(),
            Experience::Studio => STUDIO_SIZE,
        };
        let mut parked = self.other_workspace.take().unwrap_or_else(|| {
            Box::new(tabs::WorkspaceDocuments::new(
                size,
                self.recovery_dir.is_some(),
            ))
        });
        parked.swap(self);
        self.other_workspace = Some(parked);
        self.settings.experience = experience;
        self.picking = (experience == Experience::Classic
            && self.settings.classic_variant.is_none())
        .then_some(ClassicVariant::DEFAULT);
        self.persist_settings();
    }

    pub(super) fn persist_settings(&mut self) {
        if let Some(dir) = &self.settings_dir
            && let Err(e) = self.settings.save(dir)
        {
            self.editor
                .set_status(format!("Could not save preferences: {e}"));
        }
    }

    pub fn picker_open(&self) -> bool {
        self.picking.is_some()
    }

    pub(super) fn choose_variant(&mut self, v: ClassicVariant) {
        self.settings.classic_variant = Some(v);
        if let Some(dir) = &self.settings_dir
            && let Err(e) = self.settings.save(dir)
        {
            self.editor
                .set_status(format!("Could not save preferences: {e}"));
        }
        self.picking = None;
    }

    /// Presentation presets. These are look-and-feel starting points, not captured
    /// reproductions of the original applications.
    pub(super) fn apply_style(&mut self, ctx: &egui::Context) {
        let v = self.variant();
        let experience = self.experience();
        let appearance = match &self.confirm {
            Some(Confirm::Settings(draft)) => draft.values.appearance,
            _ => self.settings.appearance,
        };
        let language = match &self.confirm {
            Some(Confirm::Settings(draft)) => draft.values.language,
            _ => self.settings.language,
        };
        set_language(ctx, language);
        let theme = appearance.resolve(ctx.system_theme());
        if self.applied_variant == Some((experience, v, appearance, theme)) {
            return;
        }
        if self.applied_variant.is_none() {
            paintcraft_ui::fonts::install_for_locale(ctx, language.id());
        }
        ctx.options_mut(|options| {
            options.zoom_with_keyboard = false;
            options.input_options.zoom_modifier = egui::Modifiers::CTRL | egui::Modifiers::COMMAND;
        });
        paintcraft_ui::theme::apply(ctx, theme);
        match experience {
            Experience::Classic => paintcraft_classic::apply_style_for_theme(ctx, v, theme),
            Experience::Studio => paintcraft_studio::apply_style_for_theme(ctx, theme),
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match appearance {
            settings::Appearance::System => egui::viewport::SystemTheme::SystemDefault,
            settings::Appearance::Light => egui::viewport::SystemTheme::Light,
            settings::Appearance::Dark => egui::viewport::SystemTheme::Dark,
        }));
        self.applied_variant = Some((experience, v, appearance, theme));
    }

    /// First-visit picker, also reachable later from the toolbar.
    pub(super) fn picker_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(mut current) = self.picking else {
            return;
        };
        let choice = paintcraft_classic::skin_picker::show(
            ui.ctx(),
            &mut current,
            self.settings.classic_variant.is_none(),
        );
        if ui.ctx().current_pass_index() != 0 {
            return;
        }
        match choice {
            paintcraft_classic::skin_picker::Choice::Apply => self.choose_variant(current),
            paintcraft_classic::skin_picker::Choice::Cancel => self.picking = None,
            paintcraft_classic::skin_picker::Choice::Pending => self.picking = Some(current),
        }
    }
}

#[cfg(test)]
mod studio_layout_tests {
    use super::*;

    fn frame(app: &mut App, context: &egui::Context) {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 1000.0),
                )),
                ..Default::default()
            },
            |ui| app.ui(ui),
        );
        output.textures_delta.clear();
    }

    #[test]
    fn studio_panel_changes_survive_restart_without_editing_documents_or_classic_preferences() {
        let directory =
            std::env::temp_dir().join(format!("paintcraft-panel-restart-{}", Uuid::new_v4()));
        let settings = Settings {
            experience: Experience::Studio,
            classic_variant: Some(ClassicVariant::V10),
            classic_rulers: true,
            classic_status_bar: false,
            ..Default::default()
        };
        settings.save(&directory).unwrap();
        let mut app = App::new(None).with_settings(directory.clone());
        let original = app.editor.document_snapshot();
        let original_ticket = app.editor.history_view().ticket;
        let context = egui::Context::default();
        for _ in 0..3 {
            frame(&mut app, &context);
        }
        let positions = app.settings.studio_layout.clone();
        app.studio.tools = false;
        app.studio.layers = false;
        app.studio.history = false;
        frame(&mut app, &context);
        assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view().ticket, original_ticket);
        let saved = Settings::load(&directory).0;
        assert!(!saved.studio_layout.panels[0].visible);
        assert!(!saved.studio_layout.panels[2].visible);
        assert!(!saved.studio_layout.panels[3].visible);
        assert_eq!(
            saved.studio_layout.panels[0].position,
            positions.panels[0].position
        );
        assert_eq!(saved.classic_variant, Some(ClassicVariant::V10));
        assert!(saved.classic_rulers);
        assert!(!saved.classic_status_bar);
        let mut restarted = App::new(None).with_settings(directory.clone());
        assert!(!restarted.studio.tools);
        assert!(!restarted.studio.layers);
        assert!(!restarted.studio.history);
        assert!(restarted.studio.colors);
        let restarted_context = egui::Context::default();
        for _ in 0..3 {
            frame(&mut restarted, &restarted_context);
        }
        assert_eq!(restarted.settings.studio_layout, saved.studio_layout);
        restarted.settings.experience = Experience::Classic;
        restarted.studio.tools = true;
        frame(&mut restarted, &restarted_context);
        assert_eq!(
            Settings::load(&directory).0.studio_layout,
            saved.studio_layout,
            "Classic frames cannot overwrite Studio preferences"
        );
        drop(restarted);
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
