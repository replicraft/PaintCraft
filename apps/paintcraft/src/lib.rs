#[path = "app/appearance.rs"]
mod appearance;
#[path = "app/canvas.rs"]
mod canvas;
#[path = "app/canvas_settings.rs"]
mod canvas_settings;
#[path = "app/chrome.rs"]
mod chrome;
#[path = "app/colors.rs"]
mod colors;
pub mod controller;
#[path = "app/cursor.rs"]
mod cursor;
#[path = "app/files.rs"]
mod files;
#[path = "app/history.rs"]
mod history;
#[path = "app/input.rs"]
mod input;
#[path = "app/layers.rs"]
mod layers;
#[cfg(target_os = "macos")]
#[path = "app/native_menu.rs"]
mod native_menu;
#[path = "app/preferences.rs"]
mod preferences;
#[path = "app/recovery.rs"]
mod recovery;
#[path = "app/rendering.rs"]
mod rendering;
mod saving;
pub mod settings;
#[path = "app/status.rs"]
mod status;
#[path = "app/switching.rs"]
mod switching;
#[path = "app/tabs.rs"]
mod tabs;
#[cfg(test)]
#[path = "app/tests.rs"]
mod tests;
mod text;
#[path = "app/text_ui.rs"]
mod text_ui;
#[path = "app/view.rs"]
mod view;
#[path = "app/window.rs"]
mod window;
// The PaintCraft desktop UI. Shared widgets live in `paintcraft-ui`;
// the `app` modules connect native services, editor state, and presentation.
//
// Input is read once per frame, in the first layout pass only, so a frame that
// egui lays out twice cannot press, draw or release twice. Buttons and
// shortcuts push tokenized actions into the editor's outbox, which runs each
// activation once.

use paintcraft_runtime::{Executor, JobHandle, Limits as WorkerLimits};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::controller::{Action, Editor, Preview, Tool};
use crate::settings::{ClassicVariant, Experience, Loaded, Settings};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use paintcraft_core::types::Rgba;
use paintcraft_document::{hex, import_image};
use paintcraft_editor::execute::Selection;
use paintcraft_formats::export::{self, KNOWN_LOSSES};
use paintcraft_formats::image::{self, ImageFormat};
use paintcraft_formats::recovery::{Found, Recovery};
use paintcraft_formats::save;
use paintcraft_platform::dialogs::{Kind, NativeDialogs};
use paintcraft_render::gpu::GpuRenderer;
use paintcraft_render::render_flat;
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
    pub const HISTORY: u64 = 13;
    pub const CANVAS_SIZE: u64 = 14;
}

const DEFAULT_SIZE: (u32, u32) = settings::DEFAULT_SIZE;
const STUDIO_SIZE: (u32, u32) = (320, 320);

#[derive(Clone, Debug, PartialEq)]
enum Confirm {
    Settings(preferences::SettingsDraft),
    SwitchInterface(Experience),
    Colors(colors::ColorDraft),
    Canvas(canvas_settings::CanvasDraft),
    LayerProperties(layers::PropertiesDraft),
    DiscardForNew,
    DiscardForOpen(PathBuf),
    DiscardForRecovery(Found),
    DiscardForSaveSnapshot(u64),
    ForgetSaveSnapshot(u64),
    ReplaceFile(PathBuf),
    ReplaceSharedFile(PathBuf),
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

#[derive(Debug)]
struct RenderedFrame {
    key: RenderKey,
    pixels: Vec<u8>,
    fallback: Option<String>,
}

type RenderKey = (Uuid, u64, u64);

pub struct App {
    #[cfg(target_os = "macos")]
    native_menu: Option<paintcraft_platform::native_menu::NativeMenu>,
    fonts: text_ui::FontState,
    text_renderer: text::Shared,
    context: egui::Context,
    ime_composing: bool,
    editor: Editor,
    tabs: Vec<tabs::Tab>,
    active_tab: usize,
    next_tab_number: u64,
    tab_closing: Option<Uuid>,
    tab_close_wait: Option<Uuid>,
    tab_save_close: Option<(Uuid, Uuid)>,
    chrome_outbox: paintcraft_ui::outbox::Outbox<paintcraft_ui::chrome::Action>,
    classic: paintcraft_classic::Workspace,
    studio: paintcraft_studio::Workspace,
    other_workspace: Option<Box<tabs::WorkspaceDocuments>>,
    switching: Option<switching::Switch>,
    gpu: Option<Arc<GpuRenderer>>,
    renderer: Option<Executor<RenderedFrame>>,
    pending_frame: Option<(RenderKey, JobHandle)>,
    dialogs: Option<NativeDialogs>,
    dialog_owner: Option<Uuid>,
    texture: Option<egui::TextureHandle>,
    texture_key: Option<RenderKey>,
    rendered_preview: Option<Preview>,
    preview_revision: u64,
    preview_gesture: Option<u64>,
    preview_start_revision: u64,
    /// Straight RGBA8 flat render of what the texture shows.
    rendered: Vec<u8>,
    zoom: f32,
    pixel_grid: bool,
    fullscreen: bool,
    pending_center: Option<(f32, f32)>,
    path_text: String,
    /// The file this document was opened from or last saved to.
    bound: Option<PathBuf>,
    pending_save: Option<(PathBuf, Option<PathBuf>)>,
    saving: Option<saving::NativeSaving>,
    save_activation: Option<u64>,
    close_after_saves: bool,
    closing_workspace: bool,
    confirm: Option<Confirm>,
    canvas_rect: Option<Rect>,
    canvas_viewport: Option<Rect>,
    hover_doc: Option<(i64, i64)>,
    exit_after_frames: Option<u64>,
    recovery_dir: Option<PathBuf>,
    recoverable: Vec<Found>,
    recovery_scope: Uuid,
    recovery_needs_scan: bool,
    recovery_needs_init: bool,
    recovery_source: Option<saving::restoring::Handoff>,
    autosave_every: Duration,
    last_autosave: f64,
    allow_close: bool,
    settings_dir: Option<PathBuf>,
    settings: Settings,
    /// The variant highlighted in the first-visit picker.
    picking: Option<ClassicVariant>,
    applied_variant: Option<(
        Experience,
        ClassicVariant,
        settings::Appearance,
        egui::Theme,
    )>,
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
        let text_renderer = Arc::new(std::sync::Mutex::new(text::TextRenderer::bundled()));
        let mut app = App {
            #[cfg(target_os = "macos")]
            native_menu: None,
            fonts: Default::default(),
            text_renderer: text_renderer.clone(),
            context: Default::default(),
            ime_composing: false,
            editor: Editor::new_document(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
                .expect("default size is valid")
                .with_text_renderer(text_renderer),
            tabs: vec![tabs::Tab::initial()],
            active_tab: 0,
            next_tab_number: 1,
            tab_closing: None,
            tab_close_wait: None,
            tab_save_close: None,
            chrome_outbox: Default::default(),
            classic: Default::default(),
            studio: Default::default(),
            other_workspace: None,
            switching: None,
            gpu: None,
            renderer: None,
            pending_frame: None,
            dialogs: None,
            dialog_owner: None,
            texture: None,
            texture_key: None,
            rendered_preview: None,
            preview_revision: 0,
            preview_gesture: None,
            preview_start_revision: 0,
            rendered: Vec::new(),
            zoom: 1.0,
            pixel_grid: false,
            fullscreen: false,
            pending_center: None,
            path_text,
            bound: None,
            pending_save: None,
            saving: None,
            save_activation: None,
            close_after_saves: false,
            closing_workspace: false,
            confirm: None,
            canvas_rect: None,
            canvas_viewport: None,
            hover_doc: None,
            exit_after_frames: None,
            recovery_dir: None,
            recoverable: Vec::new(),
            recovery_scope: Uuid::new_v4(),
            recovery_needs_scan: false,
            recovery_needs_init: false,
            recovery_source: None,
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

    pub fn autosave_every(mut self, every: Duration) -> App {
        self.autosave_every = every;
        self
    }

    pub fn set_path_text(&mut self, text: &str) {
        self.path_text = text.to_owned();
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
    pub fn canvas_viewport(&self) -> Option<Rect> {
        self.canvas_viewport
    }

    pub fn canvas_rect(&self) -> Option<Rect> {
        self.canvas_rect
    }

    /// The flat render currently shown (straight RGBA8, row-major).
    pub fn rendered(&self) -> &[u8] {
        &self.rendered
    }
}
