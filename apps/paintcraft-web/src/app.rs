//! First web slice: the real Classic presentation over an explicitly blank preview.
use eframe::egui::{self, Color32, Vec2};
use paintcraft_classic::{ClassicVariant, Controls, Workspace};
use paintcraft_core::types::Rgba;
use paintcraft_tools::{Tool, ToolOptions};
use paintcraft_ui::actions::{Action, Context, Frontend, ViewAction};

const PREVIEW: &str = "UI preview — drawing, documents and files are not connected yet.";

#[derive(Debug)]
pub struct WebApp {
    controls: Controls,
    workspace: Workspace,
    variant: ClassicVariant,
    zoom: f32,
    settings: bool,
    message: String,
}

impl WebApp {
    pub fn new(ctx: &egui::Context) -> Self {
        paintcraft_ui::fonts::install(ctx);
        paintcraft_ui::theme::apply(ctx, ctx.theme());
        Self {
            controls: Controls {
                tool: Tool::Brush,
                options: ToolOptions::default(),
                primary: Rgba([0, 0, 0, 255]),
                secondary: Rgba([255; 4]),
            },
            workspace: Workspace::default(),
            variant: ClassicVariant::V11,
            zoom: 1.0,
            settings: false,
            message: PREVIEW.into(),
        }
    }

    fn context(&self) -> Context {
        Context {
            frontend: Frontend::Classic,
            // Preview controls are interactive, but every unsupported intent is
            // acknowledged explicitly. There is no document or saved-state claim.
            document_open: true,
            can_undo: false,
            can_redo: false,
            selection_active: false,
            tool: self.controls.tool,
            blocked: self.settings,
            layer_index: None,
            layer_count: 0,
            layer_visible: false,
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

    fn action(&mut self, action: Action) {
        match action {
            Action::Settings | Action::Interface => self.settings = true,
            Action::SelectTool(tool) => self.controls.tool = tool,
            Action::View(view) => match view {
                ViewAction::SetZoom(percent) => self.zoom = percent as f32 / 10_000.0,
                ViewAction::ZoomIn => self.zoom *= 1.25,
                ViewAction::ZoomOut => self.zoom /= 1.25,
                ViewAction::ActualSize => self.zoom = 1.0,
                ViewAction::Double => self.zoom = 2.0,
                ViewAction::Quadruple => self.zoom = 4.0,
                ViewAction::Octuple => self.zoom = 8.0,
                _ => self.unsupported(action),
            },
            _ => self.unsupported(action),
        }
        self.zoom = self.zoom.clamp(0.125, 10.0);
    }

    fn unsupported(&mut self, action: Action) {
        self.message = format!(
            "{} is not connected in this browser UI preview.",
            action.descriptor().label
        );
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let context = self.context();
        egui::Panel::top("web-preview-banner").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("PaintCraft Web · UI preview");
                ui.label(PREVIEW);
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
                    canvas: Some((1000, 486)),
                    pointer: None,
                    selection: None,
                },
                context,
                egui::Id::unique("web-zoom"),
            );
            for action in actions {
                self.action(action);
            }
        });
        if self.workspace.layers {
            egui::Panel::right("preview-layers").show(ui, |ui| {
                ui.heading("Layers");
                ui.label("Document layers are not connected in the web preview yet.");
            });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(paintcraft_ui::theme::workspace(ctx.theme())))
            .show(ui, |ui| {
                paintcraft_classic::canvas::canvas(
                    ui,
                    egui::Id::unique("web-preview-canvas"),
                    Vec2::new(1000.0, 486.0) * self.zoom,
                    0.0,
                    |_ui, response, painter| {
                        painter.rect_filled(response.rect, 0, Color32::WHITE);
                        if response.clicked() {
                            self.message =
                                "Drawing is not connected in this UI preview yet.".into();
                        }
                    },
                );
            });
        if self.settings {
            egui::Window::new("Web preview settings")
                .open(&mut self.settings)
                .resizable(false)
                .show(&ctx, |ui| {
                    let mut dark = ctx.theme() == egui::Theme::Dark;
                    if ui.checkbox(&mut dark, "Dark mode").changed() {
                        let theme = if dark {
                            egui::Theme::Dark
                        } else {
                            egui::Theme::Light
                        };
                        ctx.set_theme(theme);
                        paintcraft_ui::theme::apply(&ctx, theme);
                    }
                    egui::ComboBox::from_label("Classic skin")
                        .selected_text(self.variant.name())
                        .show_ui(ui, |ui| {
                            for variant in ClassicVariant::ALL {
                                ui.selectable_value(&mut self.variant, variant, variant.name());
                            }
                        });
                    ui.label("Preview settings last until this page reloads.");
                });
        }
    }
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
