//! The PaintCraft desktop UI. All decisions live in `paintcraft-ui`; this file
//! turns egui input into editor calls and editor state into pixels.
//!
//! Input is read once per frame, in the first layout pass only, so a frame that
//! egui lays out twice cannot press, draw or release twice. Buttons and
//! shortcuts push tokenized actions into the editor's outbox, which runs each
//! activation once.

use std::path::PathBuf;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use paintcraft_core::render::render_flat;
use paintcraft_types::Rgba;
use paintcraft_ui::editor::{Action, Editor};
use paintcraft_ui::view::View;
use uuid::Uuid;

/// Button and shortcut codes mixed into activation tokens.
mod code {
    pub const ADD_LAYER: u64 = 2;
    pub const UNDO: u64 = 3;
    pub const REDO: u64 = 4;
    pub const SAVE: u64 = 5;
    pub const VISIBLE: u64 = 6;
}

const DEFAULT_SIZE: (u32, u32) = (320, 320);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Confirm {
    DiscardForNew,
    ReplaceFile(PathBuf),
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("editor", &self.editor)
            .field("zoom", &self.zoom)
            .finish_non_exhaustive()
    }
}

pub struct App {
    editor: Editor,
    texture: Option<egui::TextureHandle>,
    texture_key: Option<(Uuid, u64)>,
    /// Straight RGBA8 flat render of what the texture shows.
    rendered: Vec<u8>,
    zoom: u32,
    path_text: String,
    /// The file this document was opened from or last saved to.
    bound: Option<PathBuf>,
    confirm: Option<Confirm>,
    canvas_rect: Option<Rect>,
    hover_doc: Option<(i64, i64)>,
    exit_after_frames: Option<u64>,
}

fn token(ctx: &egui::Context, code: u64) -> u64 {
    // Stable across layout passes of one frame, different on the next frame, and
    // never in the upper half reserved for gesture tokens (splitmix64 finalizer).
    let mut z = ctx
        .cumulative_frame_nr()
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ code;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) & !(1 << 63)
}

impl App {
    pub fn new(path: Option<PathBuf>) -> App {
        let (editor, bound) = match path.as_deref() {
            Some(p) if p.exists() => match Editor::open(p) {
                Ok(e) => (e, Some(p.to_path_buf())),
                Err(e) => {
                    let mut fresh = Editor::new_document(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
                        .expect("default size is valid");
                    fresh.set_path(p.to_path_buf());
                    eprintln!("cannot open {}: {e}", p.display());
                    (fresh, None)
                }
            },
            _ => (
                Editor::new_document(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
                    .expect("default size is valid"),
                None,
            ),
        };
        let path_text = path.map_or_else(
            || "untitled.pcnd".to_owned(),
            |p| p.to_string_lossy().into_owned(),
        );
        App {
            editor,
            texture: None,
            texture_key: None,
            rendered: Vec::new(),
            zoom: 1,
            path_text,
            bound,
            confirm: None,
            canvas_rect: None,
            hover_doc: None,
            exit_after_frames: None,
        }
    }

    /// Close the window after this many frames (smoke runs).
    pub fn exit_after(mut self, frames: u64) -> App {
        self.exit_after_frames = Some(frames);
        self
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// Where the canvas was drawn last frame, in screen points.
    pub fn canvas_rect(&self) -> Option<Rect> {
        self.canvas_rect
    }

    /// The flat render currently shown (straight RGBA8, row-major).
    pub fn rendered(&self) -> &[u8] {
        &self.rendered
    }

    fn sync_texture(&mut self, ctx: &egui::Context) {
        let key = self.editor.render_key();
        if self.texture_key == Some(key) {
            return;
        }
        let doc = self.editor.document();
        match render_flat(doc) {
            Ok(px) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [doc.canvas.0 as usize, doc.canvas.1 as usize],
                    &px,
                );
                match &mut self.texture {
                    Some(t) => t.set(image, egui::TextureOptions::NEAREST),
                    None => {
                        self.texture =
                            Some(ctx.load_texture("canvas", image, egui::TextureOptions::NEAREST))
                    }
                }
                self.rendered = px;
            }
            Err(e) => {
                self.texture = None;
                self.rendered.clear();
                eprintln!("render failed: {e}");
            }
        }
        self.texture_key = Some(key);
    }

    fn request(&mut self, ctx: &egui::Context, code: u64, action: Action) {
        self.editor.request(token(ctx, code), action);
    }

    /// Keyboard shortcuts and window events; first pass only. Each key press is
    /// judged with the modifiers it arrived with, not the pointer-time state.
    fn handle_global_input(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => match key {
                    egui::Key::Z if modifiers.command && modifiers.shift => {
                        self.request(&ctx, code::REDO, Action::Redo)
                    }
                    egui::Key::Z if modifiers.command => {
                        self.request(&ctx, code::UNDO, Action::Undo)
                    }
                    egui::Key::Y if modifiers.command => {
                        self.request(&ctx, code::REDO, Action::Redo)
                    }
                    egui::Key::S if modifiers.command => self.save_requested(&ctx),
                    egui::Key::Escape => self.editor.cancel_gesture(),
                    _ => {}
                },
                // Focus loss ends an unfinished gesture without publishing it.
                egui::Event::WindowFocused(false) => self.editor.cancel_gesture(),
                _ => {}
            }
        }
    }

    fn save_requested(&mut self, ctx: &egui::Context) {
        let path = PathBuf::from(self.path_text.trim());
        if path.as_os_str().is_empty() {
            return;
        }
        if path.exists() && self.bound.as_deref() != Some(path.as_path()) {
            self.confirm = Some(Confirm::ReplaceFile(path));
            return;
        }
        self.do_save(ctx, path);
    }

    fn do_save(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.editor.set_path(path.clone());
        self.request(ctx, code::SAVE, Action::Save);
        self.bound = Some(path);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                if self.editor.is_dirty() {
                    self.confirm = Some(Confirm::DiscardForNew);
                } else {
                    self.new_document();
                }
            }
            if ui.button("Add layer").clicked() {
                self.request(&ctx, code::ADD_LAYER, Action::AddLayer);
            }
            if ui.button("Undo").clicked() {
                self.request(&ctx, code::UNDO, Action::Undo);
            }
            if ui.button("Redo").clicked() {
                self.request(&ctx, code::REDO, Action::Redo);
            }
            if ui.button("Save").clicked() {
                self.save_requested(&ctx);
            }
            let file_label = ui.label("File");
            ui.add(egui::TextEdit::singleline(&mut self.path_text))
                .labelled_by(file_label.id);
            ui.separator();
            let c = self.editor.color.0;
            let mut color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
            let swatch = ui.color_edit_button_srgba(&mut color);
            swatch.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::ColorWell, true, "Pencil color")
            });
            if swatch.changed() {
                let [r, g, b, a] = color.to_srgba_unmultiplied();
                self.editor.color = Rgba([r, g, b, a]);
            }
            ui.label("Zoom");
            for z in [1u32, 2, 4, 8] {
                ui.selectable_value(&mut self.zoom, z, format!("{z}×"));
            }
        });
    }

    fn new_document(&mut self) {
        if let Ok(e) = Editor::new_document(DEFAULT_SIZE.0, DEFAULT_SIZE.1) {
            self.editor = e;
            self.bound = None;
            self.texture_key = None;
        }
    }

    fn layer_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.heading("Layers");
        let target = self.editor.target();
        for row in self.editor.layers() {
            ui.horizontal(|ui| {
                let mut visible = row.visible;
                let toggle = ui.checkbox(&mut visible, "").on_hover_text("Visible");
                toggle.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::CheckBox,
                        true,
                        visible,
                        format!("Show {}", row.name),
                    )
                });
                if toggle.changed() {
                    self.editor.request(
                        token(&ctx, code::VISIBLE ^ row.id.as_u128() as u64),
                        Action::SetVisible {
                            layer: row.id,
                            visible,
                        },
                    );
                }
                if ui
                    .selectable_label(target == Some(row.id), &row.name)
                    .clicked()
                {
                    self.editor.set_target(row.id);
                }
            });
        }
        ui.separator();
        ui.label(
            match target.and_then(|t| self.editor.document().layers.get(&t)) {
                Some(l) => format!("Drawing on: {}", l.name),
                None => "Drawing on: (no layer)".to_owned(),
            },
        );
    }

    fn confirm_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
        let (title, text) = match &confirm {
            Confirm::DiscardForNew => (
                "Discard changes?",
                "The current document has unsaved changes.".to_owned(),
            ),
            Confirm::ReplaceFile(p) => (
                "Replace file?",
                format!(
                    "{} already exists and is not this document's file.",
                    p.display()
                ),
            ),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button("Continue").clicked() {
                        match confirm {
                            Confirm::DiscardForNew => self.new_document(),
                            Confirm::ReplaceFile(p) => self.do_save(&ctx, p),
                        }
                        self.confirm = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm = None;
                    }
                });
            });
    }

    fn canvas(&mut self, ui: &mut egui::Ui, first_pass: bool) {
        let ppp = ui.ctx().pixels_per_point();
        let (w, h) = self.editor.document().canvas;
        let size = View::new((0.0, 0.0), self.zoom, ppp).canvas_size(w, h);
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(Vec2::new(size.0, size.1), Sense::click_and_drag());
                let rect = response.rect;
                self.canvas_rect = Some(rect);
                let view = View::new((rect.min.x, rect.min.y), self.zoom, ppp);
                self.hover_doc = ui
                    .input(|i| i.pointer.hover_pos())
                    .filter(|p| rect.contains(*p))
                    .and_then(|p| view.to_doc((p.x, p.y)));

                // Transparency checkerboard, then the composite. Display only: the document is untouched.
                painter.rect_filled(rect, 0.0, Color32::from_gray(200));
                let cell = 8.0 / ppp;
                let visible = painter.clip_rect().intersect(rect);
                let (mut y, mut row) = (
                    rect.min.y + ((visible.min.y - rect.min.y) / cell).floor() * cell,
                    ((visible.min.y - rect.min.y) / cell).floor() as i64,
                );
                while y < visible.max.y {
                    let (mut x, mut col) = (
                        rect.min.x + ((visible.min.x - rect.min.x) / cell).floor() * cell,
                        ((visible.min.x - rect.min.x) / cell).floor() as i64,
                    );
                    while x < visible.max.x {
                        if (row + col) % 2 == 0 {
                            painter.rect_filled(
                                Rect::from_min_size(Pos2::new(x, y), Vec2::splat(cell))
                                    .intersect(rect),
                                0.0,
                                Color32::from_gray(235),
                            );
                        }
                        x += cell;
                        col += 1;
                    }
                    y += cell;
                    row += 1;
                }
                if let Some(t) = &self.texture {
                    painter.image(
                        t.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                if first_pass {
                    let events = ui.input(|i| i.events.clone());
                    for event in events {
                        match event {
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                ..
                            } => {
                                if (response.hovered() || rect.contains(pos))
                                    && let Some(d) = view.to_doc((pos.x, pos.y))
                                {
                                    self.editor.pointer_down(d);
                                }
                            }
                            egui::Event::PointerMoved(pos) => {
                                if let Some(d) = view.to_doc((pos.x, pos.y)) {
                                    self.editor.pointer_move(d);
                                }
                            }
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: false,
                                ..
                            } => {
                                if let Some(d) = view.to_doc((pos.x, pos.y)) {
                                    self.editor.pointer_move(d);
                                }
                                self.editor.pointer_up();
                            }
                            _ => {}
                        }
                    }
                }

                // Live overlay of the unpublished stroke.
                let pts: Vec<Pos2> = self
                    .editor
                    .gesture_points()
                    .iter()
                    .map(|p| {
                        let s = view.to_screen(*p);
                        let half = view.points_per_pixel() / 2.0;
                        Pos2::new(s.0 + half, s.1 + half)
                    })
                    .collect();
                if pts.len() > 1 {
                    let c = self.editor.color.0;
                    painter.add(egui::Shape::line(
                        pts,
                        egui::Stroke::new(
                            view.points_per_pixel().max(1.0 / ppp),
                            Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
                        ),
                    ));
                }
            });
    }

    /// Lay out one pass of the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let first_pass = ctx.current_pass_index() == 0;
        if first_pass {
            self.handle_global_input(ui);
        }
        self.sync_texture(&ctx);
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{}{}",
                    self.editor.status(),
                    if self.editor.is_dirty() {
                        "  •  unsaved changes"
                    } else {
                        ""
                    }
                ));
                if let Some((x, y)) = self.hover_doc {
                    ui.separator();
                    ui.label(format!("{x}, {y}"));
                }
            });
        });
        egui::Panel::left("layers").show(ui, |ui| self.layer_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui, first_pass));
        self.confirm_dialog(ui);
        if self.editor.run_pending() > 0 {
            ctx.request_repaint();
            self.sync_texture(&ctx);
        }
        if let Some(n) = self.exit_after_frames {
            // egui repaints only on events; a smoke run must keep frames coming.
            ctx.request_repaint();
            if ctx.cumulative_frame_nr() >= n {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        App::ui(self, ui);
    }
}
