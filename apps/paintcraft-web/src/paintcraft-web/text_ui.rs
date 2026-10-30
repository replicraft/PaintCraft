//! Canvas text uses shared raster masks; the host owns input and draft lifetime.
use crate::app::WebApp;
mod composition;
pub(crate) use composition::Key as CompositionKey;
use eframe::egui::{self, Pos2, Rect, Vec2};
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
    composition: composition::State,
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
            composition: Default::default(),
            widget_owned_input: false,
            selected_all: false,
        }
    }
}

impl TextUi {
    pub(crate) fn with_style(style: TextOptions) -> Self {
        Self {
            style,
            ..Default::default()
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
        if self.text.is_composing() {
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
        if self.text.is_composing() {
            self.message = "Finish text composition before downloading".into();
            return;
        }
        let mut source_metadata = paintcraft_formats::export::metadata_policy::Intent::Refuse;
        let snapshot =
            if let Some(frozen) = self.export_frozen.as_ref().filter(|_| format.is_some()) {
                if !frozen.current(self) {
                    self.message = "The document changed; choose the image again.".into();
                    return;
                }
                if self.export_omit_source_metadata {
                    source_metadata = paintcraft_formats::export::metadata_policy::Intent::Omit;
                }
                Ok(frozen.document.clone())
            } else {
                self.editor.copy_snapshot()
            };
        match snapshot {
            Ok(snapshot) => {
                let filter_bake = if let Some(format) = format {
                    let target = paintcraft_formats::export::metadata_policy::Target::new(
                        paintcraft_formats::registry::Family::from_extension(format.extension())
                            .expect("registered export family"),
                        paintcraft_formats::registry::Operation::ExportFlat,
                    );
                    match paintcraft_formats::export::filter_bake::Policy::capture(
                        &snapshot,
                        target,
                        if self.export_bake_filters && self.export_frozen.is_some() {
                            paintcraft_formats::export::filter_bake::Intent::Bake
                        } else {
                            paintcraft_formats::export::filter_bake::Intent::Refuse
                        },
                        &|| Ok(()),
                    ) {
                        Ok(policy) => policy,
                        Err(error) => {
                            self.message = error.to_string();
                            return;
                        }
                    }
                } else {
                    Default::default()
                };
                let name = self.download_name(format);
                self.files.download(
                    ctx,
                    &snapshot,
                    crate::files::ExportOptions {
                        format,
                        jpeg_quality: self.export_quality,
                        jpeg_matte: (format == Some(ImageExport::Jpeg)).then(|| {
                            paintcraft_formats::image::JpegMatte::new(self.export_jpeg_matte)
                        }),
                        pcx_encoding: self.export_pcx_encoding,
                        frontend: self.format_profile(),
                        source_metadata,
                        filter_bake,
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
        self.paste.is_placing() || self.editor.is_dirty() || self.text.is_composing()
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
                            self.editor.tools.is_editing_text() && !self.text.is_composing(),
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
                        self.text.reset_composition();
                    }
                });
            });
        });
        self.sync_text();
    }

    pub(crate) fn text_event(&mut self, ctx: &egui::Context, event: egui::Event) {
        self.guard_composition(ctx);
        if composition::is_disabled(&event) || matches!(event, egui::Event::WindowFocused(false)) {
            self.composition_event(ctx, &event);
            return;
        }
        if self.blocked()
            || self.text.widget_owned_input
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
        {
            return;
        }
        if !self.editor.tools.is_editing_text() {
            self.text.reset_composition();
            self.text.selected_all = false;
            return;
        }
        if self.composition_event(ctx, &event) {
            return;
        }
        if self.text.is_composing()
            && let egui::Event::Key {
                key, pressed: true, ..
            } = event
        {
            if key == egui::Key::Escape {
                self.text.reset_composition();
            }
            ctx.request_repaint();
            return;
        }
        match event {
            egui::Event::Text(text) | egui::Event::Paste(text) if !self.text.is_composing() => {
                let delete = if self.text.selected_all {
                    self.editor
                        .tools
                        .text_content()
                        .unwrap_or_default()
                        .chars()
                        .count()
                } else {
                    0
                };
                if let Err(error) = self.replace_text_exact(delete, &text) {
                    self.message = error.to_string();
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
            } if !self.text.is_composing() => {
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
            } if !self.text.is_composing() => {
                if modifiers.command {
                    self.finish_text();
                } else {
                    let delete = if self.text.selected_all {
                        self.editor
                            .tools
                            .text_content()
                            .unwrap_or_default()
                            .chars()
                            .count()
                    } else {
                        0
                    };
                    if let Err(error) = self.replace_text_exact(delete, "\n") {
                        self.message = error.to_string();
                    }
                }
            }
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                if self.text.is_composing() {
                    self.text.reset_composition();
                } else {
                    self.editor.tools.cancel_text();
                }
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
        }) = self.canvas_text_preview().ok().flatten()
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
                        should_interrupt_composition: self.text.interrupt_composition(),
                    })
                });
            }
        }
    }
}
