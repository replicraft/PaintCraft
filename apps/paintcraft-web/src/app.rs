//! Browser integration of shared Classic presentation and editor services.
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
    files: crate::files::Files,
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
            preferences,
            export_confirm: false,
            confirm_replace: None,
            files: crate::files::Files::default(),
            pending_import: None,
            form: None,
            message: warning.unwrap_or_else(|| {
                "Ready — Save downloads a project copy; work stays in this tab".into()
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
            can_undo: self.editor.session.undo_depth() > 0,
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
            status_bar: true,
            rulers: false,
            fullscreen: false,
            auto_hide_toolbar: false,
            thumbnail: false,
        }
    }

    pub(crate) fn edit(&mut self, action: Edit) {
        match self.editor.apply(action) {
            Ok(()) => {
                self.message = if self.editor.session.is_dirty() {
                    "Unsaved changes — kept only in this tab".into()
                } else {
                    "Ready".into()
                }
            }
            Err(error) => self.message = format!("Edit failed: {error}"),
        }
    }

    fn action(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Settings | Action::Interface => {
                self.editor.cancel();
                self.settings = true;
            }
            Action::CanvasSize | Action::ImageProperties | Action::CanvasDefaults => {
                self.begin_canvas(action)
            }
            Action::EditColors => self.begin_colors(false, None),
            Action::EditSecondaryColors => self.begin_colors(true, None),
            Action::EditCustomColor(slot) => self.begin_colors(false, Some(slot)),
            Action::SelectTool(Tool::Text) => self.unsupported(action),
            Action::SelectTool(tool) => {
                self.editor.cancel();
                self.controls.tool = tool;
            }
            Action::Undo => self.edit(Edit::Undo),
            Action::Redo => self.edit(Edit::Redo),
            Action::ClearCanvas => self.edit(Edit::ClearCanvas { color: WHITE }),
            Action::Deselect => self.edit(Edit::Select { corners: None }),
            Action::SelectAll => {
                let (w, h) = self.editor.session.document().canvas;
                self.edit(Edit::Select {
                    corners: Some(((0, 0), (i64::from(w) - 1, i64::from(h) - 1))),
                });
            }
            Action::Export => {
                self.editor.cancel();
                self.export_confirm = true;
            }
            Action::Save | Action::SaveAs => {
                self.editor.cancel();
                self.files
                    .download(ctx, self.editor.session.document(), false);
            }
            Action::New | Action::Open => {
                self.editor.cancel();
                if self.editor.session.is_dirty() {
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
                ViewAction::FitWindow => {
                    let (w, h) = self.editor.session.document().canvas;
                    let size = (self.canvas.viewport - egui::Vec2::splat(32.0))
                        .max(egui::Vec2::splat(1.0));
                    self.zoom = (size.x / w as f32).min(size.y / h as f32);
                }
                _ => self.unsupported(action),
            },
            _ => self.unsupported(action),
        }
        self.zoom = self.zoom.clamp(0.125, 10.0);
    }

    fn new_document(&mut self) {
        let (width, height) = self.preferences.canvas.new_size();
        match Editor::new(width, height) {
            Ok(editor) => {
                self.editor = editor;
                self.canvas = Canvas::default();
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
                    "Download dialog closed — verify your copy; this tab remains unsaved".into()
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

    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.current_pass_index() != 0 || self.blocked() || ctx.egui_wants_keyboard_input() {
            return;
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                repeat: false,
                ..
            } = event
                && let Some(action) = Action::from_shortcut_for(Frontend::Classic, key, modifiers)
                && action.enabled(self.context())
            {
                self.action(ctx, action);
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.current_pass_index() == 0 {
            self.poll_files();
        }
        self.keyboard(&ctx);
        let context = self.context();
        egui::Panel::top("web-editor-banner").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("PaintCraft Web · Early editor");
                ui.label("Save downloads a project copy. Export downloads PNG. Editable text is not connected yet.");
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
        egui::Panel::bottom("classic-status").show(ui, |ui| {
            let actions = paintcraft_classic::status::contents(
                ui,
                paintcraft_classic::status::Status {
                    message: &self.message,
                    recovery: None,
                    canvas: Some(self.editor.session.document().canvas),
                    pointer: self.canvas.pointer,
                    selection: None,
                },
                self.context(),
                egui::Id::unique("web-zoom"),
            );
            for action in actions {
                self.action(&ctx, action);
            }
        });
        self.layers(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(paintcraft_ui::theme::workspace(ctx.theme())))
            .show(ui, |ui| {
                self.canvas(ui);
            });
        self.dialogs(&ctx);
        self.show_form(&ctx);
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_replace.is_some() {
            egui::Modal::new(egui::Id::unique("web-new-unsaved")).show(ctx, |ui| {
                ui.heading("Discard unsaved changes?");
                ui.label("Download a project copy to keep your work. Browser downloads cannot confirm a disk save; discard explicitly when ready.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Download project").clicked() {
                        self.files.download(ctx,self.editor.session.document(),false);
                    }
                    if ui.button("Discard and continue").clicked()
                        && let Some(action) = self.confirm_replace.take()
                    { self.replace(ctx,action); }
                    if ui.button("Cancel").clicked() { self.confirm_replace = None; }
                });
            });
        }
        if self.pending_import.is_some() {
            let mut accept = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::unique("web-import-losses")).show(ctx, |ui| {
                ui.heading("Open with these import limitations?");
                for loss in &self.pending_import.as_ref().expect("pending import").losses {
                    ui.label(loss);
                }
                ui.horizontal(|ui| {
                    accept = ui.button("Open image").clicked();
                    cancel = ui.button("Cancel").clicked();
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
                ui.heading("Export PNG copy?");
                ui.label("PNG flattens visible layers and omits hidden layer data. Save a PaintCraft project to preserve editable layers.");
                ui.horizontal(|ui| {
                    if ui.button("Download PNG").clicked() {
                        self.files.download(ctx,self.editor.session.document(),true);
                        self.export_confirm = false;
                    }
                    if ui.button("Cancel").clicked() { self.export_confirm = false; }
                });
            });
        }
        if self.settings {
            let previous_variant = self.variant;
            let mut theme_changed = false;
            egui::Window::new("Web settings").open(&mut self.settings).resizable(false).show(ctx, |ui| {
                let mut dark = ctx.theme() == egui::Theme::Dark;
                if ui.checkbox(&mut dark, "Dark mode").changed() {
                    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
                    ctx.set_theme(theme); paintcraft_ui::theme::apply(ctx,theme);
                    self.preferences.theme = Some(theme); theme_changed = true;
                }
                egui::ComboBox::from_label("Classic skin").selected_text(self.variant.name()).show_ui(ui, |ui| {
                    for variant in ClassicVariant::ALL { ui.selectable_value(&mut self.variant,variant,variant.name()); }
                });
                ui.label("Preferences are stored in this browser when storage is available. Document pixels are not stored.");
                ui.label("Browser limits: 4 megapixels, 8192 points per stroke; 64 MiB history budget (older entries may be trimmed).");
            });
            if theme_changed || previous_variant != self.variant {
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
