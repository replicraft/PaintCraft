//! Browser integration of shared Classic presentation and editor services.
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
    variant: ClassicVariant,
    pub(crate) zoom: f32,
    settings: bool,
    confirm_new: bool,
    pub(crate) message: String,
}

impl WebApp {
    pub fn new(ctx: &egui::Context) -> Self {
        paintcraft_ui::fonts::install(ctx);
        paintcraft_ui::theme::apply(ctx, ctx.theme());
        Self {
            controls: Controls {
                tool: Tool::Brush,
                options: ToolOptions::default(),
                primary: paintcraft_core::types::Rgba([0, 0, 0, 255]),
                secondary: WHITE,
            },
            editor: Editor::new(1000, 486).expect("default canvas is admitted"),
            canvas: Canvas::default(),
            workspace: Workspace::default(),
            variant: ClassicVariant::V11,
            zoom: 1.0,
            settings: false,
            confirm_new: false,
            message: "Ready — drawing stays in this tab; file saving is not connected yet".into(),
        }
    }

    pub(crate) fn blocked(&self) -> bool {
        self.settings || self.confirm_new
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

    fn action(&mut self, action: Action) {
        match action {
            Action::Settings | Action::Interface => {
                self.editor.cancel();
                self.settings = true;
            }
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
            Action::New => {
                self.editor.cancel();
                if self.editor.session.is_dirty() {
                    self.confirm_new = true;
                } else {
                    self.new_document();
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
        match Editor::new(1000, 486) {
            Ok(editor) => {
                self.editor = editor;
                self.canvas = Canvas::default();
                self.message = "New document".into();
            }
            Err(error) => self.message = error.to_string(),
        }
        self.confirm_new = false;
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
                self.action(action);
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.keyboard(&ctx);
        let context = self.context();
        egui::Panel::top("web-editor-banner").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("PaintCraft Web · Early editor");
                ui.label("Work stays in this tab. Files and text are not connected yet.");
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
                self.action(action);
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
                self.action(action);
            }
        });
        self.layers(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(paintcraft_ui::theme::workspace(ctx.theme())))
            .show(ui, |ui| {
                self.canvas(ui);
            });
        self.dialogs(&ctx);
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_new {
            egui::Modal::new(egui::Id::unique("web-new-unsaved")).show(ctx, |ui| {
                ui.heading("Discard unsaved changes?");
                ui.label("File saving is not connected yet. Creating a new canvas discards this document.");
                ui.horizontal(|ui| {
                    if ui.button("Discard and create new").clicked() { self.new_document(); }
                    if ui.button("Cancel").clicked() { self.confirm_new = false; }
                });
            });
        }
        if self.settings {
            egui::Window::new("Web settings").open(&mut self.settings).resizable(false).show(ctx, |ui| {
                let mut dark = ctx.theme() == egui::Theme::Dark;
                if ui.checkbox(&mut dark, "Dark mode").changed() {
                    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
                    ctx.set_theme(theme); paintcraft_ui::theme::apply(ctx,theme);
                }
                egui::ComboBox::from_label("Classic skin").selected_text(self.variant.name()).show_ui(ui, |ui| {
                    for variant in ClassicVariant::ALL { ui.selectable_value(&mut self.variant,variant,variant.name()); }
                });
                ui.label("Settings last until this page reloads.");
                ui.label("Browser limits: 4 megapixels, 8192 points per stroke; 64 MiB history budget (older entries may be trimmed).");
            });
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
