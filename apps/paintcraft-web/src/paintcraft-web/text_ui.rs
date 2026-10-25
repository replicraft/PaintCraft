//! Canvas text uses shared raster masks; the host owns input and draft lifetime.
use crate::app::WebApp;
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use paintcraft_core::types::text::TextOptions;
use paintcraft_tools::{Preview, Tool};
use paintcraft_ui::actions::ImageExport;
use paintcraft_ui::t;
#[derive(Debug)]
pub(crate) struct TextUi {
    pub style: TextOptions,
    pub(crate) search: String,
    pub(crate) families: Vec<String>,
    pub(crate) font_generation: u64,
    pub preedit: String,
    pub widget_owned_input: bool,
    pub selected_all: bool,
}
impl Default for TextUi {
    fn default() -> Self {
        Self {
            style: TextOptions {
                family: "Arial".into(),
                ..TextOptions::default()
            },
            search: String::new(),
            font_generation: 0,
            families: paintcraft_ui::fonts::BUNDLED_FAMILIES
                .iter()
                .map(|name| (*name).into())
                .collect(),
            preedit: String::new(),
            widget_owned_input: false,
            selected_all: false,
        }
    }
}

impl WebApp {
    pub(crate) fn sync_text(&mut self) {
        self.editor.tools.color = self.controls.primary;
        self.editor.tools.secondary = self.controls.secondary;
        self.editor.tools.set_text_style(self.text.style.clone());
    }
    pub(crate) fn finish_text(&mut self) -> bool {
        self.sync_text();
        if !self.text.preedit.is_empty() {
            self.message = "Finish text composition before placing or downloading".into();
            return false;
        }
        match self.editor.place_text() {
            Ok(()) => {
                self.text.selected_all = false;
                true
            }
            Err(error) => {
                self.message = format!("Text not placed: {error}; the draft is still editable");
                false
            }
        }
    }
    pub(crate) fn download(&mut self, ctx: &egui::Context, png: bool) {
        self.download_as(ctx, png.then_some(ImageExport::Png));
    }
    pub(crate) fn download_image(&mut self, ctx: &egui::Context, format: ImageExport) {
        self.download_as(ctx, Some(format));
    }
    fn download_as(&mut self, ctx: &egui::Context, format: Option<ImageExport>) {
        if !self.floating_file_ready() {
            return;
        }
        if self.editor.tools.is_constructing_shape() {
            self.message = paintcraft_tools::FINISH_SHAPE_BEFORE_FILE.into();
            return;
        }
        self.editor.tools.cancel_gesture();
        self.sync_text();
        if !self.text.preedit.is_empty() {
            self.message = "Finish text composition before downloading".into();
            return;
        }
        match self.editor.copy_snapshot() {
            Ok(snapshot) => {
                let name = self.download_name(format);
                self.files.download(
                    ctx,
                    &snapshot,
                    crate::files::ExportOptions {
                        format,
                        jpeg_quality: self.export_quality,
                        frontend: self.format_profile(),
                    },
                    &name,
                    self.editor.session.history_ticket().owner,
                );
            }
            Err(error) => {
                self.message = format!("Download not prepared: {error}; text remains editable")
            }
        }
    }
    pub(crate) fn unsaved(&self) -> bool {
        self.paste.is_placing() || self.editor.is_dirty() || !self.text.preedit.is_empty()
    }

    pub(crate) fn text_chrome(&mut self, ui: &mut egui::Ui) {
        if self.frontend == paintcraft_ui::actions::Frontend::Classic
            && !self.workspace.toolbar.visible()
        {
            return;
        }
        if self.controls.tool != Tool::Text && !self.editor.tools.is_editing_text() {
            return;
        }
        let enabled = !self.blocked();
        egui::Panel::top("web-text-options").show(ui, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                if self.variant == paintcraft_classic::ClassicVariant::V11 {
                    paintcraft_classic::text_options(
                        ui,
                        &mut self.text.style,
                        &self.text.families,
                        &mut self.text.search,
                        false,
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!(
                        ui.ctx(),
                        "Enter for a new line · Cmd/Ctrl+Enter to place · Escape to cancel"
                    ));
                    if ui
                        .add_enabled(
                            self.editor.tools.is_editing_text() && self.text.preedit.is_empty(),
                            egui::Button::new(t!(ui.ctx(), "Place text")),
                        )
                        .clicked()
                    {
                        self.finish_text();
                    }
                    if ui
                        .add_enabled(
                            self.editor.tools.is_editing_text(),
                            egui::Button::new(t!(ui.ctx(), "Cancel text")),
                        )
                        .clicked()
                    {
                        self.editor.tools.cancel_text();
                        self.text.preedit.clear();
                    }
                });
            });
        });
        self.sync_text();
    }

    pub(crate) fn text_event(&mut self, ctx: &egui::Context, event: egui::Event) {
        if self.blocked()
            || self.text.widget_owned_input
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
        {
            return;
        }
        if !self.editor.tools.is_editing_text() {
            self.text.preedit.clear();
            self.text.selected_all = false;
            return;
        }
        match event {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                self.text.preedit = text.chars().take(4096).collect()
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                self.replace_selected_text();
                self.text.preedit.clear();
                self.editor.tools.type_text(&text);
            }
            egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
                before_chars,
                after_chars: _,
            }) => {
                for _ in 0..before_chars.min(4096) {
                    self.editor.tools.backspace();
                }
            }
            egui::Event::Text(text) | egui::Event::Paste(text) if self.text.preedit.is_empty() => {
                self.replace_selected_text();
                if self.editor.tools.type_text(&text) > 0 {
                    self.message="Some text was omitted: the block allows 4096 characters and printable text".into();
                }
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                repeat: false,
                ..
            } if modifiers.command
                && matches!(
                    key,
                    egui::Key::A | egui::Key::B | egui::Key::I | egui::Key::U
                ) =>
            {
                match key {
                    egui::Key::A => self.text.selected_all = true,
                    egui::Key::B => self.text.style.bold = !self.text.style.bold,
                    egui::Key::I => self.text.style.italic = !self.text.style.italic,
                    egui::Key::U => self.text.style.underline = !self.text.style.underline,
                    _ => {}
                }
                self.sync_text();
            }
            egui::Event::Key {
                key: egui::Key::Delete,
                pressed: true,
                ..
            } if self.text.selected_all => self.replace_selected_text(),
            egui::Event::Key {
                key: egui::Key::Backspace,
                pressed: true,
                ..
            } if self.text.preedit.is_empty() => {
                if self.text.selected_all {
                    self.replace_selected_text();
                } else {
                    self.editor.tools.backspace();
                }
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                modifiers,
                ..
            } if self.text.preedit.is_empty() => {
                if modifiers.command {
                    self.finish_text();
                } else {
                    self.editor.tools.type_text("\n");
                }
            }
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                self.editor.tools.cancel_text();
                self.text.preedit.clear();
            }
            _ => {}
        }
        ctx.request_repaint();
    }

    fn replace_selected_text(&mut self) {
        if self.text.selected_all {
            self.editor.tools.clear_text();
            self.text.selected_all = false;
        }
    }

    pub(crate) fn text_overlay(&mut self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect) {
        let Some(Preview::Text {
            style: Some(style),
            at,
            text,
            ..
        }) = self.editor.tools.preview()
        else {
            return;
        };
        let Ok(mask) = self.editor.text_mask(&text, &style, at) else {
            return;
        };
        let (x, y, height) = mask.caret;
        let point = rect.min + Vec2::new(x as f32, y as f32) * self.zoom;
        let cursor =
            Rect::from_min_size(point, Vec2::new(1.0, (height as f32 * self.zoom).max(1.0)));
        let painter = painter.with_clip_rect(painter.clip_rect().intersect(rect));
        if !self.blocked() {
            if self.text.selected_all
                && let Some(bounds) = mask.bounds
            {
                painter.rect_filled(
                    Rect::from_min_max(
                        rect.min + Vec2::new(bounds.x0 as f32, bounds.y0 as f32) * self.zoom,
                        rect.min + Vec2::new(bounds.x1 as f32, bounds.y1 as f32) * self.zoom,
                    ),
                    0,
                    ui.visuals().selection.bg_fill.gamma_multiply(0.25),
                );
            }
            painter.line_segment(
                [cursor.left_top(), cursor.left_bottom()],
                egui::Stroke::new(1.0, ui.visuals().selection.stroke.color),
            );
            if !self.text.preedit.is_empty() {
                painter.text(
                    point,
                    egui::Align2::LEFT_TOP,
                    &self.text.preedit,
                    egui::FontId::proportional(f32::from(style.points) * 96.0 / 72.0 * self.zoom),
                    Color32::from_rgb(0, 120, 215),
                );
            }
            if !ui.ctx().egui_wants_keyboard_input() && !egui::Popup::is_any_open(ui.ctx()) {
                ui.ctx().output_mut(|out| {
                    out.ime = Some(egui::output::IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: rect.intersect(painter.clip_rect()),
                        cursor_rect: Rect::from_min_max(
                            Pos2::new(
                                cursor.min.x.clamp(rect.left(), rect.right()),
                                cursor.min.y.clamp(rect.top(), rect.bottom()),
                            ),
                            Pos2::new(
                                cursor.max.x.clamp(rect.left(), rect.right()),
                                cursor.max.y.clamp(rect.top(), rect.bottom()),
                            ),
                        ),
                        should_interrupt_composition: false,
                    })
                });
            }
        }
    }
}
