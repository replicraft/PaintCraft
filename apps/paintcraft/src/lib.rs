//! The PaintCraft desktop UI. All decisions live in `paintcraft-ui`; this file
//! turns egui input into editor calls and editor state into pixels.
//!
//! Input is read once per frame, in the first layout pass only, so a frame that
//! egui lays out twice cannot press, draw or release twice. Buttons and
//! shortcuts push tokenized actions into the editor's outbox, which runs each
//! activation once.

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use paintcraft_core::document::{hex, import_image};
use paintcraft_core::execute::Selection;
use paintcraft_core::render::render_flat;
use paintcraft_formats::export::{self, KNOWN_LOSSES};
use paintcraft_formats::image::{self, ImageFormat};
use paintcraft_formats::recovery::{Found, Recovery};
use paintcraft_formats::save;
use paintcraft_types::Rgba;
use paintcraft_ui::editor::{Action, Editor, Preview, ShapeStyle, Tool};
use paintcraft_ui::settings::{ClassicVariant, Loaded, Settings};
use paintcraft_ui::view::View;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Button and shortcut codes mixed into activation tokens.
mod code {
    pub const ADD_LAYER: u64 = 2;
    pub const UNDO: u64 = 3;
    pub const REDO: u64 = 4;
    pub const SAVE: u64 = 5;
    pub const VISIBLE: u64 = 6;
    pub const SELECT_ALL: u64 = 7;
    pub const DESELECT: u64 = 8;
    pub const RENAME: u64 = 9;
    pub const OPACITY: u64 = 10;
    pub const DELETE: u64 = 11;
    pub const MOVE: u64 = 12;
}

const DEFAULT_SIZE: (u32, u32) = (320, 320);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Confirm {
    DiscardForNew,
    DiscardForOpen(PathBuf),
    ReplaceFile(PathBuf),
    QuitUnsaved,
    /// Export to a flat image, naming what it would lose and whether it replaces a file.
    Export {
        path: PathBuf,
        format: ImageFormat,
        losses: Vec<String>,
        exists: bool,
    },
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
    recovery_dir: Option<PathBuf>,
    recoverable: Vec<Found>,
    autosave_every: Duration,
    last_autosave: f64,
    allow_close: bool,
    settings_dir: Option<PathBuf>,
    settings: Settings,
    /// The variant highlighted in the first-visit picker.
    picking: Option<ClassicVariant>,
    applied_variant: Option<ClassicVariant>,
    /// Edit buffers for the selected layer's name and opacity, committed on release.
    name_edit: Option<(Uuid, String)>,
    opacity_edit: Option<(Uuid, u8)>,
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
        let path_text = path.as_deref().map_or_else(
            || "untitled.pcnd".to_owned(),
            |p| p.to_string_lossy().into_owned(),
        );
        let mut app = App {
            editor: Editor::new_document(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
                .expect("default size is valid"),
            texture: None,
            texture_key: None,
            rendered: Vec::new(),
            zoom: 1,
            path_text,
            bound: None,
            confirm: None,
            canvas_rect: None,
            hover_doc: None,
            exit_after_frames: None,
            recovery_dir: None,
            recoverable: Vec::new(),
            autosave_every: Duration::from_secs(5),
            last_autosave: 0.0,
            allow_close: false,
            settings_dir: None,
            settings: Settings::default(),
            picking: None,
            applied_variant: None,
            name_edit: None,
            opacity_edit: None,
        };
        if let Some(p) = path.filter(|p| p.exists())
            && let Err(e) = app.open_path(&p)
        {
            eprintln!("cannot open {}: {e}", p.display());
            app.editor.set_path(p);
        }
        app
    }

    /// Protect work with crash-recovery checkpoints in `dir`, and offer whatever an
    /// earlier crashed session left behind.
    pub fn with_recovery(mut self, dir: PathBuf) -> App {
        self.recoverable = Recovery::scan(&dir);
        self.recovery_dir = Some(dir);
        self.attach_recovery();
        self
    }

    fn attach_recovery(&mut self) {
        if let Some(dir) = &self.recovery_dir {
            match Recovery::start(dir) {
                Ok(r) => self.editor.attach_recovery(r),
                Err(e) => self
                    .editor
                    .set_status(format!("Crash recovery unavailable: {e}")),
            }
        }
    }

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
        self.picking = settings
            .classic_variant
            .is_none()
            .then_some(ClassicVariant::DEFAULT);
        self.settings = settings;
        self.settings_dir = Some(dir);
        self
    }

    /// The Classic variant in effect (the default until one is chosen).
    pub fn variant(&self) -> ClassicVariant {
        self.settings
            .classic_variant
            .unwrap_or(ClassicVariant::DEFAULT)
    }

    pub fn picker_open(&self) -> bool {
        self.picking.is_some()
    }

    fn choose_variant(&mut self, v: ClassicVariant) {
        self.settings.classic_variant = Some(v);
        if let Some(dir) = &self.settings_dir
            && let Err(e) = self.settings.save(dir)
        {
            self.editor
                .set_status(format!("Could not save the interface choice: {e}"));
        }
        self.picking = None;
    }

    /// Presentation presets. These are look-and-feel starting points, not captured
    /// reproductions of the original applications.
    fn apply_style(&mut self, ctx: &egui::Context) {
        let v = self.variant();
        if self.applied_variant == Some(v) {
            return;
        }
        let (rounding, panel, accent) = match v {
            ClassicVariant::V7 => (
                0u8,
                Color32::from_rgb(233, 238, 247),
                Color32::from_rgb(63, 119, 186),
            ),
            ClassicVariant::V10 => (
                0,
                Color32::from_rgb(243, 243, 243),
                Color32::from_rgb(0, 120, 215),
            ),
            ClassicVariant::V11 => (
                6,
                Color32::from_rgb(249, 249, 249),
                Color32::from_rgb(0, 95, 184),
            ),
        };
        let mut vis = egui::Visuals::light();
        vis.panel_fill = panel;
        vis.window_fill = panel;
        vis.selection.bg_fill = accent;
        let r = egui::CornerRadius::same(rounding);
        for w in [
            &mut vis.widgets.noninteractive,
            &mut vis.widgets.inactive,
            &mut vis.widgets.hovered,
            &mut vis.widgets.active,
            &mut vis.widgets.open,
        ] {
            w.corner_radius = r;
        }
        vis.window_corner_radius = r;
        ctx.set_visuals(vis);
        self.applied_variant = Some(v);
    }

    /// First-visit picker, also reachable later from the toolbar.
    fn picker_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(mut current) = self.picking else {
            return;
        };
        let ctx = ui.ctx().clone();
        let mut done = false;
        egui::Window::new("Choose your Paint")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label("Pick the look you know. You can change it later from Interface.");
                ui.horizontal(|ui| {
                    for v in ClassicVariant::ALL {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(150.0, 110.0), Sense::click());
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::RadioButton,
                                true,
                                current == v,
                                v.name(),
                            )
                        });
                        if response.clicked() {
                            current = v;
                        }
                        draw_variant_thumbnail(ui.painter(), rect, v, current == v);
                    }
                });
                ui.horizontal(|ui| {
                    for v in ClassicVariant::ALL {
                        ui.add_sized([150.0, 18.0], egui::Label::new(v.name()));
                    }
                });
                if ui.button("Continue").clicked() {
                    done = true;
                }
            });
        if done {
            self.choose_variant(current);
        } else {
            self.picking = Some(current);
        }
    }

    pub fn autosave_every(mut self, every: Duration) -> App {
        self.autosave_every = every;
        self
    }

    pub fn set_path_text(&mut self, text: &str) {
        self.path_text = text.to_owned();
    }

    /// Swap in a new editor; its predecessor's work was already saved or deliberately discarded.
    fn install(&mut self, editor: Editor) {
        self.editor.discard_checkpoint();
        self.editor = editor;
        self.texture_key = None;
        self.attach_recovery();
    }

    /// Open a `.pcnd` document or import a PNG, BMP or JPEG picture.
    fn open_path(&mut self, path: &Path) -> paintcraft_types::Result<()> {
        let bytes = std::fs::read(path).map_err(|e| {
            paintcraft_types::Error::new(
                paintcraft_types::ErrorCode::StorageFailed,
                format!("cannot read {}: {}", path.display(), e.kind()),
            )
        })?;
        let (editor, bound, note) = if image::ImageFormat::sniff(&bytes).is_some() {
            let img = image::decode(&bytes)?;
            let doc = import_image(
                img.width,
                img.height,
                &img.rgba,
                &hex(&Sha256::digest(&bytes)),
            )?;
            let note = if img.losses.is_empty() {
                format!("Imported {}", path.display())
            } else {
                format!(
                    "Imported {} (not kept: {})",
                    path.display(),
                    img.losses.join(", ")
                )
            };
            // A picture is a new document; it is not bound to the image file.
            (Editor::with_document(doc, None), None, note)
        } else {
            let editor = Editor::open(path)?;
            (
                editor,
                Some(path.to_path_buf()),
                format!("Opened {}", path.display()),
            )
        };
        self.install(editor);
        self.bound = bound;
        self.editor.set_status(note);
        Ok(())
    }

    fn open_requested(&mut self) {
        let path = PathBuf::from(self.path_text.trim());
        if self.editor.is_dirty() {
            self.confirm = Some(Confirm::DiscardForOpen(path));
        } else if let Err(e) = self.open_path(&path) {
            self.editor.set_status(format!("Cannot open: {e}"));
        }
    }

    /// Plan an export to the format named by the file extension; losses and
    /// replacement are confirmed before anything is written.
    fn export_requested(&mut self) {
        let path = PathBuf::from(self.path_text.trim());
        let Some(format) = ImageFormat::from_extension(&path) else {
            self.editor
                .set_status("Name the file .png, .bmp or .jpg to export");
            return;
        };
        let all: Vec<String> = KNOWN_LOSSES.iter().map(|l| (*l).to_owned()).collect();
        match export::export_flat(self.editor.document(), format, 90, &all) {
            Ok(plan) => {
                self.confirm = Some(Confirm::Export {
                    exists: path.exists(),
                    path,
                    format,
                    losses: plan.losses,
                })
            }
            Err(e) => self.editor.set_status(format!("Cannot export: {e}")),
        }
    }

    fn do_export(&mut self, path: &Path, format: ImageFormat, losses: &[String]) {
        let result = export::export_flat(self.editor.document(), format, 90, losses)
            .map_err(|e| e.to_string());
        let status = match result {
            Ok(flat) => {
                let outcome = save::save(path, &flat.bytes, true);
                if outcome.is_success() {
                    format!("Exported {}", path.display())
                } else {
                    format!("Not exported safely: {}", outcome.reasons.join("; "))
                }
            }
            Err(e) => format!("Cannot export: {e}"),
        };
        self.editor.set_status(status);
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
        let text_focus = self.editor.is_editing_text() && !ctx.egui_wants_keyboard_input();
        for event in events {
            match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } => match key {
                    // Holding Backspace or Enter repeats while typing; one-shot commands do not.
                    egui::Key::Enter if text_focus && !modifiers.command => {
                        self.editor.type_text("\n")
                    }
                    egui::Key::Backspace if text_focus => self.editor.backspace(),
                    _ if repeat => {}
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
                    egui::Key::Enter if modifiers.command => self.editor.commit_text(),
                    egui::Key::Escape => {
                        self.editor.cancel_gesture();
                        self.editor.cancel_text();
                    }
                    _ => {}
                },
                // Focus loss ends an unfinished gesture without publishing it.
                egui::Event::WindowFocused(false) => {
                    self.editor.cancel_gesture();
                    self.editor.commit_text();
                }
                // Typed characters go to the text being edited unless a text field has the keyboard.
                egui::Event::Text(typed) if text_focus => self.editor.type_text(&typed),
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
            if ui.button("Open").clicked() {
                self.open_requested();
            }
            if ui.button("Export").clicked() {
                self.export_requested();
            }
            if ui.button("Interface").clicked() {
                self.picking = Some(self.variant());
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
            self.install(e);
            self.bound = None;
        }
    }

    fn tool_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.heading("Tools");
        ui.horizontal_wrapped(|ui| {
            for tool in Tool::ALL {
                if ui
                    .selectable_label(self.editor.tool == tool, tool.name())
                    .clicked()
                {
                    self.editor.select_tool(tool);
                }
            }
        });
        let tool = self.editor.tool;
        if matches!(
            tool,
            Tool::Brush | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse
        ) {
            ui.horizontal(|ui| {
                let label = ui.label("Size");
                ui.add(egui::DragValue::new(&mut self.editor.options.size).range(1..=1024))
                    .labelled_by(label.id);
            });
        }
        if tool == Tool::Brush {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.editor.options.round, true, "Round");
                ui.selectable_value(&mut self.editor.options.round, false, "Square");
            });
        }
        if matches!(tool, Tool::Rect | Tool::Ellipse) {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.editor.options.style,
                    ShapeStyle::Outline,
                    "Outline",
                );
                ui.selectable_value(&mut self.editor.options.style, ShapeStyle::Filled, "Filled");
                ui.selectable_value(&mut self.editor.options.style, ShapeStyle::Both, "Both");
            });
            if self.editor.options.style == ShapeStyle::Both {
                let c = self.editor.secondary.0;
                let mut color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                let swatch = ui.color_edit_button_srgba(&mut color);
                swatch.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::ColorWell, true, "Fill color")
                });
                if swatch.changed() {
                    let [r, g, b, a] = color.to_srgba_unmultiplied();
                    self.editor.secondary = Rgba([r, g, b, a]);
                }
            }
        }
        if tool == Tool::Text {
            ui.horizontal(|ui| {
                let label = ui.label("Text size");
                ui.add(
                    egui::DragValue::new(&mut self.editor.options.text_scale)
                        .range(1..=16)
                        .suffix("×"),
                )
                .labelled_by(label.id);
            });
            ui.label("Click the canvas, type, then click elsewhere or press Ctrl+Enter.");
        }
        if tool == Tool::Fill {
            ui.horizontal(|ui| {
                let label = ui.label("Tolerance");
                ui.add(egui::DragValue::new(&mut self.editor.options.tolerance).range(0..=255))
                    .labelled_by(label.id);
            });
        }
        if tool == Tool::Select {
            let (w, h) = self.editor.document().canvas;
            ui.horizontal(|ui| {
                if ui.button("Select all").clicked() {
                    let corners = Some(((0, 0), (i64::from(w) - 1, i64::from(h) - 1)));
                    self.request(&ctx, code::SELECT_ALL, Action::Select { corners });
                }
                if ui.button("Deselect").clicked() {
                    self.request(&ctx, code::DESELECT, Action::Select { corners: None });
                }
            });
        }
        ui.separator();
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
        let Some(t) = target else {
            ui.label("Drawing on: (no layer)");
            return;
        };
        let Some((name, opacity, index)) = self.editor.layer_properties(t) else {
            return;
        };
        ui.label(format!("Drawing on: {name}"));
        let top = self.editor.document().roots.len() - 1;

        // Name: edited locally, applied when the field loses focus.
        let buf = match &mut self.name_edit {
            Some(b) if b.0 == t => b,
            other => other.insert((t, name.clone())),
        };
        ui.horizontal(|ui| {
            let label = ui.label("Layer name");
            let r = ui
                .add(egui::TextEdit::singleline(&mut buf.1))
                .labelled_by(label.id);
            if r.lost_focus() {
                if buf.1 != name {
                    let rename = Action::Rename {
                        layer: t,
                        name: buf.1.clone(),
                    };
                    self.editor.request(token(&ctx, code::RENAME), rename);
                }
            } else if !r.has_focus() {
                buf.1.clone_from(&name);
            }
        });

        // Opacity: a drag is one transaction, applied on release.
        let obuf = match &mut self.opacity_edit {
            Some(b) if b.0 == t => b,
            other => other.insert((t, opacity)),
        };
        ui.horizontal(|ui| {
            let label = ui.label("Opacity");
            let r = ui
                .add(egui::DragValue::new(&mut obuf.1).range(0..=255))
                .labelled_by(label.id);
            if r.drag_stopped() || r.lost_focus() {
                if obuf.1 != opacity {
                    let set = Action::SetOpacity {
                        layer: t,
                        opacity: obuf.1,
                    };
                    self.editor.request(token(&ctx, code::OPACITY), set);
                }
            } else if !r.dragged() && !r.has_focus() {
                obuf.1 = opacity;
            }
        });

        ui.horizontal(|ui| {
            if ui
                .add_enabled(index < top, egui::Button::new("Move up"))
                .clicked()
            {
                self.request(
                    &ctx,
                    code::MOVE,
                    Action::MoveLayer {
                        layer: t,
                        to_index: index + 1,
                    },
                );
            }
            if ui
                .add_enabled(index > 0, egui::Button::new("Move down"))
                .clicked()
            {
                self.request(
                    &ctx,
                    code::MOVE,
                    Action::MoveLayer {
                        layer: t,
                        to_index: index - 1,
                    },
                );
            }
            if ui.button("Delete layer").clicked() {
                self.request(&ctx, code::DELETE, Action::DeleteLayer { layer: t });
            }
        });
    }

    fn confirm_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
        let (title, text, go) = match &confirm {
            Confirm::DiscardForNew | Confirm::DiscardForOpen(_) => (
                "Discard changes?",
                "The current document has unsaved changes.".to_owned(),
                "Continue",
            ),
            Confirm::ReplaceFile(p) => (
                "Replace file?",
                format!(
                    "{} already exists and is not this document's file.",
                    p.display()
                ),
                "Continue",
            ),
            Confirm::QuitUnsaved => (
                "Quit without saving?",
                "There are unsaved changes. A recovery copy is kept and offered next time."
                    .to_owned(),
                "Quit anyway",
            ),
            Confirm::Export {
                path,
                format,
                losses,
                exists,
            } => {
                let mut t = format!("Export {} as {}.", path.display(), format.name());
                if !losses.is_empty() {
                    t += &format!("\nThis export loses: {}.", losses.join(", "));
                }
                if *exists {
                    t += "\nThe file exists and will be replaced.";
                }
                ("Export", t, "Export now")
            }
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button(go).clicked() {
                        match confirm {
                            Confirm::DiscardForNew => self.new_document(),
                            Confirm::DiscardForOpen(p) => {
                                if let Err(e) = self.open_path(&p) {
                                    self.editor.set_status(format!("Cannot open: {e}"));
                                }
                            }
                            Confirm::ReplaceFile(p) => self.do_save(&ctx, p),
                            Confirm::QuitUnsaved => {
                                self.allow_close = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            Confirm::Export {
                                path,
                                format,
                                losses,
                                ..
                            } => self.do_export(&path, format, &losses),
                        }
                        self.confirm = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm = None;
                    }
                });
            });
    }

    /// Offer work left behind by a crashed session.
    fn recovery_dialog(&mut self, ui: &mut egui::Ui) {
        if self.recoverable.is_empty() {
            return;
        }
        let ctx = ui.ctx().clone();
        let item = self.recoverable[0].clone();
        let name = item
            .original
            .clone()
            .unwrap_or_else(|| "an unsaved document".to_owned());
        egui::Window::new("Recover unsaved work?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label(format!(
                    "PaintCraft found unsaved work from an earlier session: {name}."
                ));
                ui.horizontal(|ui| {
                    if ui.button("Recover").clicked() {
                        match Editor::recover(&item.checkpoint) {
                            Ok(e) => {
                                self.install(e);
                                self.bound = None;
                                if let Some(o) = &item.original {
                                    self.path_text.clone_from(o);
                                }
                                if let Some(dir) = &self.recovery_dir {
                                    Recovery::discard(dir, item.session);
                                }
                                self.recoverable.remove(0);
                            }
                            Err(e) => self.editor.set_status(format!("Cannot recover: {e}")),
                        }
                    }
                    if ui.button("Discard").clicked() {
                        if let Some(dir) = &self.recovery_dir {
                            Recovery::discard(dir, item.session);
                        }
                        self.recoverable.remove(0);
                    }
                    if ui.button("Ask later").clicked() {
                        self.recoverable.clear();
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

                self.draw_overlays(&painter, &view, ppp);
            });
    }

    /// Display-only feedback: the committed selection and the unpublished gesture.
    fn draw_overlays(&self, painter: &egui::Painter, view: &View, ppp: f32) {
        let screen = |p: (i64, i64)| {
            let s = view.to_screen(p);
            Pos2::new(s.0, s.1)
        };
        let px = view.points_per_pixel();
        let box_of = |a: (i64, i64), b: (i64, i64)| {
            Rect::from_min_max(
                screen((a.0.min(b.0), a.1.min(b.1))),
                screen((a.0.max(b.0) + 1, a.1.max(b.1) + 1)),
            )
        };
        let ants = |r: Rect| {
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0 / ppp, Color32::BLACK),
                egui::StrokeKind::Outside,
            );
            painter.rect_stroke(
                r.expand(1.0 / ppp),
                0.0,
                egui::Stroke::new(1.0 / ppp, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        };
        if let Selection::Rect(r) = self.editor.selection() {
            ants(box_of((r.x0, r.y0), (r.x1 - 1, r.y1 - 1)));
        }
        let color = |c: Rgba| Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3]);
        match self.editor.preview() {
            Some(Preview::Path {
                points,
                size,
                round,
                color: c,
            }) => {
                // Erasing previews as a pale outline; the real pixels change on release.
                let shade = if c.a() == 0 {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 160)
                } else {
                    color(c)
                };
                let width = (size as f32 * px).max(1.0 / ppp);
                let centres: Vec<Pos2> = points
                    .iter()
                    .map(|p| screen(*p) + Vec2::splat(px / 2.0))
                    .collect();
                if centres.len() > 1 {
                    painter.add(egui::Shape::line(
                        centres.clone(),
                        egui::Stroke::new(width, shade),
                    ));
                }
                for c in centres {
                    if round {
                        painter.circle_filled(c, width / 2.0, shade);
                    } else {
                        painter.rect_filled(
                            Rect::from_center_size(c, Vec2::splat(width)),
                            0.0,
                            shade,
                        );
                    }
                }
            }
            Some(Preview::Shape {
                tool,
                from,
                to,
                stroke,
                fill,
            }) => {
                let r = box_of(from, to);
                let line = stroke.map(|(c, size)| {
                    egui::Stroke::new((size as f32 * px).max(1.0 / ppp), color(c))
                });
                match tool {
                    Tool::Line => {
                        if let Some(l) = line {
                            let half = Vec2::splat(px / 2.0);
                            painter.line_segment([screen(from) + half, screen(to) + half], l);
                        }
                    }
                    Tool::Rect => {
                        if let Some(f) = fill {
                            painter.rect_filled(r, 0.0, color(f));
                        }
                        if let Some(l) = line {
                            painter.rect_stroke(r, 0.0, l, egui::StrokeKind::Inside);
                        }
                    }
                    _ => {
                        if let Some(f) = fill {
                            painter.add(egui::epaint::EllipseShape::filled(
                                r.center(),
                                r.size() / 2.0,
                                color(f),
                            ));
                        }
                        if let Some(l) = line {
                            painter.add(egui::epaint::EllipseShape::stroke(
                                r.center(),
                                r.size() / 2.0,
                                l,
                            ));
                        }
                    }
                }
            }
            Some(Preview::Marquee { from, to }) => ants(box_of(from, to)),
            Some(Preview::Text {
                at,
                text,
                scale,
                color: c,
            }) => {
                let canvas = paintcraft_types::Rect::new(
                    i64::MIN / 4,
                    i64::MIN / 4,
                    i64::MAX / 2,
                    i64::MAX / 2,
                )
                .expect("a huge clip");
                if let Ok(spans) =
                    paintcraft_core::font::layout(&text, at, i64::from(scale), &canvas)
                {
                    for s in spans {
                        painter.rect_filled(
                            Rect::from_min_max(screen((s.x0, s.y)), screen((s.x1 + 1, s.y + 1))),
                            0.0,
                            color(c),
                        );
                    }
                }
                // A caret after the last typed character, and a frame around the box.
                let lines: Vec<&str> = text.split('\n').collect();
                let cols = lines.last().map_or(0, |l| l.chars().count()) as i64;
                let (cx, cy) = (
                    at.0 + cols * 8 * i64::from(scale),
                    at.1 + (lines.len() as i64 - 1) * 8 * i64::from(scale),
                );
                painter.rect_filled(
                    Rect::from_min_max(
                        screen((cx, cy)),
                        screen((cx + 1, cy + 8 * i64::from(scale))),
                    ),
                    0.0,
                    Color32::from_rgb(0, 95, 184),
                );
            }
            None => {}
        }
    }

    /// Lay out one pass of the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let first_pass = ctx.current_pass_index() == 0;
        if first_pass {
            self.handle_global_input(ui);
        }
        self.apply_style(&ctx);
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
        egui::Panel::left("layers").show(ui, |ui| {
            self.tool_panel(ui);
            self.layer_panel(ui);
        });
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui, first_pass));
        self.picker_dialog(ui);
        self.recovery_dialog(ui);
        self.confirm_dialog(ui);
        if first_pass
            && ctx.input(|i| i.viewport().close_requested())
            && self.editor.is_dirty()
            && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::QuitUnsaved);
        }
        if self.editor.run_pending() > 0 {
            ctx.request_repaint();
            self.sync_texture(&ctx);
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_autosave >= self.autosave_every.as_secs_f64() {
            self.last_autosave = now;
            self.editor.autosave();
        }
        if self.editor.is_dirty() {
            ctx.request_repaint_after(self.autosave_every.max(Duration::from_millis(250)));
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

/// A small schematic of each variant's window, so the choice is visual.
fn draw_variant_thumbnail(p: &egui::Painter, rect: Rect, v: ClassicVariant, selected: bool) {
    let (bar, ribbon, canvas) = match v {
        ClassicVariant::V7 => (
            Color32::from_rgb(120, 160, 215),
            Color32::from_rgb(214, 224, 240),
            Color32::WHITE,
        ),
        ClassicVariant::V10 => (
            Color32::from_rgb(255, 255, 255),
            Color32::from_rgb(235, 235, 235),
            Color32::WHITE,
        ),
        ClassicVariant::V11 => (
            Color32::from_rgb(243, 243, 243),
            Color32::from_rgb(250, 250, 250),
            Color32::WHITE,
        ),
    };
    let round = if v == ClassicVariant::V11 { 8.0 } else { 0.0 };
    let stroke = if selected {
        egui::Stroke::new(3.0, Color32::from_rgb(0, 95, 184))
    } else {
        egui::Stroke::new(1.0, Color32::GRAY)
    };
    p.rect_filled(rect, round, Color32::from_gray(220));
    p.rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), 14.0)),
        round,
        bar,
    );
    p.rect_filled(
        Rect::from_min_size(
            rect.min + Vec2::new(0.0, 14.0),
            Vec2::new(rect.width(), 28.0),
        ),
        0.0,
        ribbon,
    );
    for i in 0..6 {
        let x = rect.min.x + 8.0 + i as f32 * 14.0;
        p.rect_filled(
            Rect::from_min_size(Pos2::new(x, rect.min.y + 20.0), Vec2::splat(10.0)),
            if v == ClassicVariant::V11 { 5.0 } else { 0.0 },
            Color32::from_gray(150),
        );
    }
    p.rect_filled(
        Rect::from_min_max(
            rect.min + Vec2::new(8.0, 50.0),
            rect.max - Vec2::new(8.0, 8.0),
        ),
        0.0,
        canvas,
    );
    p.rect_stroke(rect, round, stroke, egui::StrokeKind::Inside);
}
