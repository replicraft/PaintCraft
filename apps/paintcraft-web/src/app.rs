//! Browser integration of shared Classic presentation and editor services.
#[cfg(test)]
#[path = "crop_tests.rs"]
mod crop_tests;
mod forms;
use crate::{
    canvas::Canvas,
    editor::{Editor, WHITE},
};
use eframe::egui;
use paintcraft_classic::{ClassicVariant, Controls, Workspace};
use paintcraft_tools::{Action as Edit, Tool, ToolOptions};
use paintcraft_ui::actions::{Action, Context, Frontend, ViewAction};

#[derive(Debug)]
pub struct WebApp {
    pub(crate) controls: Controls,
    pub(crate) editor: Editor,
    pub(crate) canvas: Canvas,
    pub(crate) workspace: Workspace,
    pub(crate) variant: ClassicVariant,
    name: String,
    pub(crate) zoom: f32,
    settings: bool,
    preferences: crate::preferences::Preferences,
    export_confirm: bool,
    confirm_replace: Option<Action>,
    pub(crate) files: crate::files::Files,
    pub(crate) text: crate::text_ui::TextUi,
    pending_import: Option<crate::files::Loaded>,
    form: Option<forms::Form>,
    pub(crate) message: String,
}

impl WebApp {
    pub fn new(ctx: &egui::Context) -> Self {
        let (preferences, warning) = crate::preferences::Preferences::load();
        if let Some(theme) = preferences.theme {
            ctx.set_theme(theme);
        }
        let (width, height) = preferences.canvas.new_size();
        paintcraft_ui::i18n::set_language(ctx, preferences.language);
        paintcraft_ui::fonts::install(ctx);
        paintcraft_ui::theme::apply(ctx, ctx.theme());
        Self {
            controls: Controls {
                tool: Tool::Brush,
                options: ToolOptions::default(),
                primary: paintcraft_core::types::Rgba([0, 0, 0, 255]),
                secondary: WHITE,
            },
            editor: Editor::new(width, height).expect("validated startup canvas is admitted"),
            canvas: Canvas::default(),
            workspace: Workspace {
                custom_colors: preferences.custom_colors,
                ..Workspace::default()
            },
            variant: preferences.variant,
            name: "Untitled".into(),
            zoom: 1.0,
            settings: false,
            text: crate::text_ui::TextUi {
                style: paintcraft_classic::TextOptions {
                    family: preferences.default_font.clone(),
                    ..Default::default()
                },
                ..Default::default()
            },
            preferences,
            export_confirm: false,
            confirm_replace: None,
            files: crate::files::Files::default(),
            pending_import: None,
            form: None,
            message: warning.unwrap_or_else(|| {
                "Ready - Save downloads a project copy; work stays in this tab".into()
            }),
        }
    }

    pub(crate) fn blocked(&self) -> bool {
        self.settings
            || self.export_confirm
            || self.confirm_replace.is_some()
            || self.files.busy
            || self.pending_import.is_some()
            || self.form.is_some()
    }

    fn context(&self) -> Context {
        let doc = self.editor.session.document();
        let index = self
            .editor
            .target
            .and_then(|id| doc.roots.iter().position(|root| *root == id));
        Context {
            frontend: Frontend::Classic,
            document_open: true,
            can_undo: self.editor.session.undo_depth() > 0 || self.editor.has_text(),
            can_redo: self.editor.session.redo_depth() > 0,
            selection_active: self.editor.session.selection()
                != paintcraft_editor::execute::Selection::Inactive,
            tool: self.controls.tool,
            blocked: self.blocked(),
            layer_index: index,
            layer_count: doc.roots.len(),
            layer_visible: self
                .editor
                .target
                .and_then(|id| doc.layer(&id))
                .is_some_and(|l| l.visible),
            zoom: self.zoom,
            pixel_grid: false,
            can_fit_selection: false,
            can_crop_selection: matches!(self.editor.session.selection(),paintcraft_editor::execute::Selection::Rect(rect) if rect.intersect(&doc.canvas_rect()).is_some()),
            status_bar: true,
            rulers: false,
            fullscreen: false,
            auto_hide_toolbar: false,
            thumbnail: false,
        }
    }

    pub(crate) fn edit(&mut self, action: Edit) {
        let remember_size = matches!(
            action,
            Edit::CropToSelection
                | Edit::ResizeCanvas { .. }
                | Edit::ResizeCanvasAnchored { .. }
                | Edit::TransformImage { .. }
        );
        let picked = matches!(action, Edit::PickColor(_));
        let before_size = self.editor.session.document().canvas;
        match self.editor.apply(action) {
            Ok(()) => {
                if picked {
                    self.controls.primary = self.editor.tools.color;
                    self.controls.secondary = self.editor.tools.secondary;
                    self.controls.tool = self.editor.tools.tool;
                }
                let size = self.editor.session.document().canvas;
                if remember_size && size != before_size {
                    self.remember_canvas_size(size);
                }
                self.message = if self.unsaved() {
                    "Unsaved changes - kept only in this tab".into()
                } else {
                    "Ready".into()
                }
            }
            Err(error) => self.message = format!("Edit failed: {error}"),
        }
    }

    fn action(&mut self, ctx: &egui::Context, action: Action) {
        if matches!(
            action,
            Action::New | Action::Open | Action::Save | Action::SaveAs | Action::Export
        ) && self.editor.tools.is_constructing_shape()
        {
            self.message = paintcraft_tools::FINISH_SHAPE_BEFORE_FILE.into();
            return;
        }
        match action {
            Action::Crop => self.edit(Edit::CropToSelection),
            Action::EraseSelection => self.edit(Edit::EraseSelection {
                color: paintcraft_tools::commands::canvas_color(self.editor.session.document()),
            }),
            Action::TransformImage(transform) => self.edit(Edit::TransformImage {
                transform,
                clear: paintcraft_tools::commands::canvas_color(self.editor.session.document()),
            }),
            Action::Settings | Action::Interface => {
                self.editor.tools.cancel_gesture();
                self.settings = true;
            }
            Action::CanvasSize | Action::ImageProperties | Action::CanvasDefaults => {
                self.begin_canvas(action)
            }
            Action::EditColors => self.begin_colors(false, None),
            Action::EditSecondaryColors => self.begin_colors(true, None),
            Action::EditCustomColor(slot) => self.begin_colors(false, Some(slot)),
            Action::SelectTool(tool) => {
                if !self.finish_text() {
                    self.controls.tool = Tool::Text;
                    return;
                }
                self.editor.tools.cancel_gesture();
                self.controls.tool = tool;
                self.editor.tools.choose_tool(tool);
            }
            Action::Undo => {
                self.editor.tools.cancel_gesture();
                self.edit(Edit::Undo);
            }
            Action::Redo => {
                self.editor.tools.cancel_gesture();
                self.edit(Edit::Redo);
            }
            Action::ClearCanvas => self.edit(Edit::ClearCanvas {
                color: paintcraft_tools::commands::canvas_color(self.editor.session.document()),
            }),
            Action::Deselect => self.edit(Edit::Select { corners: None }),
            Action::SelectAll => {
                let (w, h) = self.editor.session.document().canvas;
                self.edit(Edit::Select {
                    corners: Some(((0, 0), (i64::from(w) - 1, i64::from(h) - 1))),
                });
            }
            Action::Export => {
                self.editor.tools.cancel_gesture();
                self.export_confirm = true;
            }
            Action::Save | Action::SaveAs => {
                self.editor.tools.cancel_gesture();
                self.download(ctx, false);
            }
            Action::New | Action::Open => {
                self.editor.tools.cancel_gesture();
                if self.unsaved() {
                    self.confirm_replace = Some(action);
                } else {
                    self.replace(ctx, action);
                }
            }
            Action::View(view) => match view {
                ViewAction::SetZoom(percent) => self.zoom = percent as f32 / 10_000.0,
                ViewAction::ZoomIn => self.zoom *= 1.25,
                ViewAction::ZoomOut => self.zoom /= 1.25,
                ViewAction::ActualSize => self.zoom = 1.0,
                ViewAction::Double => self.zoom = 2.0,
                ViewAction::Quadruple => self.zoom = 4.0,
                ViewAction::Octuple => self.zoom = 8.0,
                ViewAction::FitWindow => self.fit_canvas(),
                _ => self.unsupported(action),
            },
            _ => self.unsupported(action),
        }
        self.zoom = self.zoom.clamp(0.125, 10.0);
        ctx.request_repaint();
    }

    fn new_document(&mut self) {
        let (width, height) = self.preferences.canvas.new_size();
        match Editor::new(width, height) {
            Ok(editor) => {
                self.editor = editor;
                self.canvas = Canvas::default();
                self.text.preedit.clear();
                self.name = "Untitled".into();
                self.update_title();
                self.message = "New document".into();
                self.remember_canvas_size((width, height));
            }
            Err(error) => self.message = error.to_string(),
        }
        self.confirm_replace = None;
    }

    fn replace(&mut self, ctx: &egui::Context, action: Action) {
        if action == Action::Open {
            self.files.open(ctx);
        } else {
            self.new_document();
        }
    }

    fn install(&mut self, loaded: crate::files::Loaded) {
        match Editor::from_document(loaded.document) {
            Ok(editor) => {
                self.editor = editor;
                self.canvas = Canvas::default();
                self.text.preedit.clear();
                self.name = loaded.name;
                self.update_title();
                self.message = format!("Opened {}", self.name);
            }
            Err(error) => self.message = format!("Open failed: {error}"),
        }
    }

    fn persist_preferences(&mut self) {
        if let Err(error) = self.preferences.save() {
            self.message = format!("{error}; choices apply only to this tab");
        }
    }

    fn remember_canvas_size(&mut self, size: (u32, u32)) {
        if self.preferences.canvas.last != Some(size) {
            self.preferences.canvas.last = Some(size);
            self.persist_preferences();
        }
    }

    fn update_title(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            document.set_title(&format!("{} - PaintCraft", self.name));
        }
    }

    fn poll_files(&mut self) {
        use crate::files::Outcome;
        match self.files.poll() {
            Some(Ok(Outcome::Loaded(loaded))) if loaded.losses.is_empty() => self.install(loaded),
            Some(Ok(Outcome::Loaded(loaded))) => self.pending_import = Some(loaded),
            Some(Ok(Outcome::Cancelled)) => {
                self.message = "File dialog cancelled; current document retained".into()
            }
            Some(Ok(Outcome::DownloadClosed)) => {
                self.message =
                    "Download dialog closed - verify your copy; this tab remains unsaved".into()
            }
            Some(Err(error)) => self.message = format!("File operation failed: {error}"),
            None => {}
        }
    }

    fn unsupported(&mut self, action: Action) {
        self.message = format!(
            "{} is not connected in the browser yet.",
            action.descriptor().label
        );
    }

    pub(crate) fn keyboard_event(&mut self, ctx: &egui::Context, event: &egui::Event) -> bool {
        if self.blocked()
            || self.text.widget_owned_input
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
            || !self.text.preedit.is_empty()
        {
            return false;
        }
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            repeat: false,
            ..
        } = event
        {
            if self.editor.tools.is_editing_text()
                && modifiers.command
                && matches!(
                    key,
                    egui::Key::A | egui::Key::B | egui::Key::I | egui::Key::U
                )
            {
                return false;
            }
            if let Some(action) = Action::from_shortcut_for(Frontend::Classic, *key, *modifiers)
                && action.enabled(self.context())
            {
                self.action(ctx, action);
                return true;
            }
        }
        false
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.current_pass_index() == 0 {
            self.poll_files();
        }
        self.text.widget_owned_input =
            ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(&ctx);
        let context = self.context();
        egui::Panel::top("web-editor-banner").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(paintcraft_ui::i18n::tr(ui.ctx(), "PaintCraft Web · Early editor"));
                ui.label(paintcraft_ui::i18n::tr(ui.ctx(), "Save downloads a project copy. Export downloads PNG. Text stays editable until placed; downloads include the draft."));
            });
        });
        egui::Panel::top("classic-chrome").show(ui, |ui| {
            let actions = paintcraft_classic::chrome(
                ui,
                self.variant,
                &mut self.controls,
                &mut self.workspace,
                context,
            );
            for action in actions {
                self.action(&ctx, action);
            }
        });
        let status_rect = egui::Panel::bottom("classic-status")
            .show(ui, |ui| {
                let size = egui::vec2(
                    ui.available_width(),
                    paintcraft_classic::status::height(ui, true),
                );
                ui.allocate_space(size).1
            })
            .inner;
        self.text_chrome(ui);
        self.layers(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(paintcraft_ui::theme::workspace(ctx.theme())))
            .show(ui, |ui| {
                self.canvas(ui);
            });
        ui.scope_builder(egui::UiBuilder::new().max_rect(status_rect), |ui| {
            let selection = self
                .editor
                .tools
                .selection_preview_size(self.editor.session.document().canvas_rect())
                .or_else(|| match self.editor.session.selection() {
                    paintcraft_editor::execute::Selection::Rect(rect) => {
                        Some((rect.width(), rect.height()))
                    }
                    paintcraft_editor::execute::Selection::Empty => Some((0, 0)),
                    paintcraft_editor::execute::Selection::Inactive => None,
                });
            let actions = paintcraft_classic::status::contents(
                ui,
                paintcraft_classic::status::Status {
                    message: &self.message,
                    recovery: None,
                    canvas: Some(self.editor.session.document().canvas),
                    pointer: self.canvas.pointer,
                    selection,
                },
                self.context(),
                egui::Id::unique("web-zoom"),
            );
            for action in actions {
                self.action(&ctx, action);
            }
        });
        self.dialogs(&ctx);
        self.show_form(&ctx);
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_replace.is_some() {
            egui::Modal::new(egui::Id::unique("web-new-unsaved")).show(ctx, |ui| {
                ui.heading(paintcraft_ui::i18n::tr(ui.ctx(), "Discard unsaved changes?"));
                ui.label(paintcraft_ui::i18n::tr(ui.ctx(), "Download a project copy to keep your work. Browser downloads cannot confirm a disk save; discard explicitly when ready."));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(paintcraft_ui::i18n::tr(ui.ctx(), "Download project")).clicked() {
                        self.download(ctx,false);
                    }
                    if ui.button(paintcraft_ui::i18n::tr(ui.ctx(), "Discard and continue")).clicked()
                        && let Some(action) = self.confirm_replace.take()
                    { self.replace(ctx,action); }
                    if ui.button(paintcraft_ui::i18n::tr(ui.ctx(), "Cancel")).clicked() { self.confirm_replace = None; }
                });
            });
        }
        if self.pending_import.is_some() {
            let mut accept = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::unique("web-import-losses")).show(ctx, |ui| {
                ui.heading(paintcraft_ui::i18n::tr(
                    ui.ctx(),
                    "Open with these import limitations?",
                ));
                for loss in &self.pending_import.as_ref().expect("pending import").losses {
                    ui.label(loss);
                }
                ui.horizontal(|ui| {
                    accept = ui
                        .button(paintcraft_ui::i18n::tr(ui.ctx(), "Open image"))
                        .clicked();
                    cancel = ui
                        .button(paintcraft_ui::i18n::tr(ui.ctx(), "Cancel"))
                        .clicked();
                });
            });
            if accept && let Some(loaded) = self.pending_import.take() {
                self.install(loaded);
            }
            if cancel {
                self.pending_import = None;
            }
        }
        if self.export_confirm {
            egui::Modal::new(egui::Id::unique("web-export-png")).show(ctx, |ui| {
                ui.heading(paintcraft_ui::i18n::tr(ui.ctx(), "Export PNG copy?"));
                ui.label(paintcraft_ui::i18n::tr(ui.ctx(), "PNG flattens visible layers and omits hidden layer data. Save a PaintCraft project to preserve editable layers."));
                ui.horizontal(|ui| {
                    if ui.button(paintcraft_ui::i18n::tr(ui.ctx(), "Download PNG")).clicked() {
                        self.download(ctx,true);
                        self.export_confirm = false;
                    }
                    if ui.button(paintcraft_ui::i18n::tr(ui.ctx(), "Cancel")).clicked() { self.export_confirm = false; }
                });
            });
        }
        if self.settings {
            let previous_variant = self.variant;
            let previous_language = self.preferences.language;
            let mut theme_changed = false;
            let previous_font = self.preferences.default_font.clone();
            egui::Window::new(paintcraft_ui::i18n::tr(ctx, "Web settings")).open(&mut self.settings).resizable(false).show(ctx, |ui| {
                paintcraft_ui::i18n::picker(ui, &mut self.preferences.language);
                let mut dark = ctx.theme() == egui::Theme::Dark;
                if ui.checkbox(&mut dark, paintcraft_ui::i18n::tr(ui.ctx(), "Dark mode")).changed() {
                    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
                    ctx.set_theme(theme); paintcraft_ui::theme::apply(ctx,theme);
                    self.preferences.theme = Some(theme); theme_changed = true;
                }
                egui::ComboBox::from_label(paintcraft_ui::i18n::tr(ui.ctx(), "Classic skin")).selected_text(self.variant.name()).show_ui(ui, |ui| {
                    for variant in ClassicVariant::ALL { ui.selectable_value(&mut self.variant,variant,variant.name()); }
                });
                ui.separator();
                ui.heading(paintcraft_ui::i18n::tr(ui.ctx(), "Text preferences"));
                ui.label(paintcraft_ui::i18n::tr(ui.ctx(), "Default font"));
                paintcraft_classic::font_family(ui, &mut self.preferences.default_font, &self.text.families, &mut self.text.search, false);
                ui.label(paintcraft_ui::i18n::tr(ui.ctx(), "Preferences are stored in this browser when storage is available. Document pixels are not stored."));
                ui.label(paintcraft_ui::i18n::tr(ui.ctx(), "Browser limits: 4 megapixels, 8192 points per stroke; 64 MiB history budget (older entries may be trimmed)."));
            });
            if previous_language != self.preferences.language {
                paintcraft_ui::i18n::set_language(ctx, self.preferences.language);
            }
            if previous_font != self.preferences.default_font {
                self.text
                    .style
                    .family
                    .clone_from(&self.preferences.default_font);
                self.sync_text();
            }
            if theme_changed
                || previous_variant != self.variant
                || previous_font != self.preferences.default_font
                || previous_language != self.preferences.language
            {
                self.preferences.variant = self.variant;
                self.persist_preferences();
            }
        }
    }
}

impl eframe::App for WebApp {
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
