#[path = "app/appearance.rs"]
mod appearance;
#[path = "app/auto_scroll.rs"]
mod auto_scroll;
#[path = "app/brushes.rs"]
mod brushes;
#[path = "app/canvas.rs"]
mod canvas;
#[path = "app/canvas_settings.rs"]
mod canvas_settings;
#[path = "app/chrome.rs"]
mod chrome;
#[path = "app/clipboard.rs"]
mod clipboard;
#[path = "app/colors.rs"]
mod colors;
pub mod controller;
#[path = "app/cpu_rendering.rs"]
mod cpu_rendering;
#[path = "app/cursor.rs"]
mod cursor;
#[path = "app/dds_export.rs"]
mod dds_export;
mod destination;
#[path = "app/editing.rs"]
mod editing;
#[path = "app/export_options.rs"]
mod export_options;
#[path = "app/export_sources.rs"]
mod export_sources;
#[path = "app/files.rs"]
mod files;
#[path = "app/format_admission.rs"]
mod format_admission;
#[path = "app/format_filters.rs"]
mod format_filters;
#[path = "app/gpu_display.rs"]
mod gpu_display;
#[path = "app/history.rs"]
mod history;
#[path = "app/icns_export.rs"]
mod icns_export;
#[path = "app/icon_export.rs"]
mod icon_export;
#[path = "app/importing.rs"]
mod importing;
#[path = "app/input.rs"]
mod input;
#[path = "app/jxr_export.rs"]
mod jxr_export;
#[path = "app/layer_panel.rs"]
mod layer_panel;
#[path = "app/layer_thumbnails.rs"]
mod layer_thumbnails;
#[path = "app/layers.rs"]
mod layers;
#[cfg(target_os = "macos")]
#[path = "app/native_menu.rs"]
mod native_menu;
#[path = "app/palettes.rs"]
mod palettes;
#[path = "app/pan_tool.rs"]
mod pan_tool;
#[path = "app/paste.rs"]
mod paste;
#[path = "app/preferences.rs"]
mod preferences;
#[path = "app/printing.rs"]
mod printing;
#[path = "app/recent.rs"]
mod recent;
#[path = "app/recovery.rs"]
mod recovery;
#[path = "app/rendering.rs"]
mod rendering;
#[path = "app/residency.rs"]
mod residency;
#[path = "app/resize_skew.rs"]
mod resize_skew;
#[path = "app/save_all.rs"]
mod save_all;
mod saving;
#[path = "app/selection_ui.rs"]
mod selection_ui;
pub mod settings;
#[path = "app/shapes.rs"]
mod shapes;
#[path = "app/status.rs"]
mod status;
#[path = "app/studio_canvas.rs"]
mod studio_canvas;
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
#[path = "app/thumbnail.rs"]
mod thumbnail;
#[path = "app/view.rs"]
mod view;
#[path = "app/window.rs"]
mod window;
#[path = "app/zoom_tool.rs"]
mod zoom_tool;
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
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use paintcraft_core::types::Rgba;
use paintcraft_document::{hex, import_image};
use paintcraft_editor::execute::Selection;
use paintcraft_formats::export::{self, KNOWN_LOSSES};
use paintcraft_formats::image::{self, ImageFormat};
use paintcraft_formats::recovery::{Found, Recovery};
use paintcraft_formats::save;
use paintcraft_platform::dialogs::{Kind, NativeDialogs};
use paintcraft_render::gpu::GpuRenderer;
#[cfg(test)]
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
    pub const CLEAR_CANVAS: u64 = 15;
    pub const EXPORT: u64 = 16;
    pub const CROP: u64 = 17;
}

const DEFAULT_SIZE: (u32, u32) = settings::DEFAULT_SIZE;
const STUDIO_SIZE: (u32, u32) = (320, 320);

#[derive(Clone, Debug, PartialEq)]
enum Confirm {
    PasteLayer,
    ImportLosses,
    Settings(preferences::SettingsDraft),
    SwitchInterface(Experience),
    Colors(colors::ColorDraft),
    Canvas(canvas_settings::CanvasDraft),
    ResizeSkew(resize_skew::Draft),
    RecentFiles,
    ExportOptions(export_options::Draft),
    IconExport,
    IcnsExport,
    JxrExport,
    DdsExport,
    Palettes,
    Shapes,
    Brushes,
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
        jpeg_quality: u8,
        pcx_encoding: paintcraft_ui::pcx_export::Encoding,
        source: controller::PreviewStamp,
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
    gpu_texture: Option<paintcraft_render::gpu::GpuTextureFrame>,
    fallback: Option<String>,
    device_epoch: Option<paintcraft_render::gpu::DeviceEpoch>,
}

type RenderKey = (Uuid, u64, u64, paintcraft_core::types::Rect, u8);

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
    gpu_completion: Option<paintcraft_render::gpu::CompletionDriver>,
    renderer: Option<Executor<RenderedFrame>>,
    editing: Option<controller::background::Service>,
    worker_budget: paintcraft_runtime::SharedBudget,
    budget_wakes: Option<residency::BudgetWakes>,
    cpu_renderer: cpu_rendering::CpuRenderer,
    layer_thumbnails: layer_thumbnails::LayerThumbnails,
    selection_outline: paintcraft_raster::selection::OutlineCache,
    selection_bounds: std::cell::RefCell<paintcraft_raster::selection::BoundsCache>,
    thumbnail: thumbnail::Thumbnail,
    pending_frame: Option<(RenderKey, JobHandle)>,
    dialogs: Option<NativeDialogs>,
    printer: paintcraft_platform::printing::PrintService,
    clipboard: clipboard::State,
    paste: paste::State,
    pending_import: Option<importing::Pending>,
    imported_source: Option<PathBuf>,
    auxiliary_sources: Vec<destination::SourceGuard>,
    svg_font_picker: Option<importing::FontPick>,
    icon_export: Option<icon_export::State>,
    icns_export: Option<icns_export::State>,
    jxr_export: Option<jxr_export::State>,
    jxr_export_picker: bool,
    dds_export: Option<dds_export::State>,
    dds_export_picker: bool,
    palettes: palettes::State,
    shapes: shapes::State,
    brushes: brushes::State,
    dialog_owner: Option<Uuid>,
    dialog_frontend: Option<paintcraft_formats::registry::Frontend>,
    dialog_source: Option<(Arc<paintcraft_document::Document>, (Uuid, u64))>,
    export_dialog_quality: Option<u8>,
    export_dialog_pcx: Option<paintcraft_ui::pcx_export::Encoding>,
    export_dialog_stamp: Option<controller::PreviewStamp>,
    pcx_preview: paintcraft_ui::pcx_export::Cache<(
        controller::PreviewStamp,
        paintcraft_ui::pcx_export::Encoding,
    )>,
    texture: Option<egui::TextureHandle>,
    gpu_target: Option<gpu_display::Target>,
    gpu_scaled_display: bool,
    gpu_texture: Option<gpu_display::Texture>,
    texture_key: Option<RenderKey>,
    render_region: Option<paintcraft_core::types::Rect>,
    render_level: u8,
    render_residency_retry: Option<(
        RenderKey,
        paintcraft_core::types::resources::AllocationDenial,
    )>,
    rendered_preview: Option<Arc<Preview>>,
    rendered_image_paste: Option<paintcraft_tools::PreviewRevision>,
    preview_revision: u64,
    preview_gesture: Option<(Uuid, Option<u64>)>,
    preview_start_revision: u64,
    /// CPU-backed flat display; native GPU surfaces deliberately have no host copy.
    rendered: Vec<u8>,
    /// Logical UI points per document pixel; 1.0 is 100% on every display scale.
    zoom: f32,
    studio_fit: Option<view::StudioFit>,
    initial_fit: bool,
    pixel_grid: bool,
    fullscreen: bool,
    pending_center: Option<(f32, f32)>,
    save_all: Option<save_all::Batch>,
    path_text: String,
    window_title: String,
    /// The file this document was opened from or last saved to.
    bound: Option<PathBuf>,
    pending_save: Option<(PathBuf, destination::Previous)>,
    deferred_files: std::collections::VecDeque<editing::DeferredFile>,
    saving: Option<saving::NativeSaving>,
    save_activation: Option<u64>,
    close_after_saves: bool,
    close_after_edits: bool,
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
    recent: recent::History,
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
        let text_renderer = Arc::new(std::sync::Mutex::new(text::bundled()));
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
            gpu_completion: None,
            renderer: None,
            editing: None,
            worker_budget: paintcraft_runtime::SharedBudget::new(3 << 30),
            budget_wakes: None,
            cpu_renderer: cpu_rendering::CpuRenderer::new(),
            layer_thumbnails: Default::default(),
            selection_outline: Default::default(),
            selection_bounds: Default::default(),
            thumbnail: Default::default(),
            pending_frame: None,
            dialogs: None,
            printer: Default::default(),
            clipboard: Default::default(),
            paste: Default::default(),
            pending_import: None,
            imported_source: None,
            auxiliary_sources: Vec::new(),
            svg_font_picker: None,
            icon_export: None,
            icns_export: None,
            jxr_export: None,
            jxr_export_picker: false,
            dds_export: None,
            dds_export_picker: false,
            palettes: Default::default(),
            shapes: Default::default(),
            brushes: Default::default(),
            dialog_owner: None,
            dialog_frontend: None,
            dialog_source: None,
            export_dialog_quality: None,
            export_dialog_pcx: None,
            export_dialog_stamp: None,
            pcx_preview: Default::default(),
            texture: None,
            gpu_target: None,
            gpu_scaled_display: false,
            gpu_texture: None,
            texture_key: None,
            render_region: None,
            render_level: 0,
            render_residency_retry: None,
            rendered_preview: None,
            rendered_image_paste: None,
            preview_revision: 0,
            preview_gesture: None,
            preview_start_revision: 0,
            rendered: Vec::new(),
            zoom: 1.0,
            studio_fit: None,
            initial_fit: true,
            pixel_grid: false,
            fullscreen: false,
            pending_center: None,
            save_all: None,
            path_text,
            window_title: String::new(),
            bound: None,
            pending_save: None,
            deferred_files: Default::default(),
            saving: None,
            save_activation: None,
            close_after_saves: false,
            close_after_edits: false,
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
            recent: Default::default(),
            settings: Settings::default(),
            picking: None,
            applied_variant: None,
            name_edit: None,
            opacity_edit: None,
        };
        if let Some(p) = path
            && let Err(e) = app.open_path(&p)
        {
            eprintln!("cannot open {}: {e}", p.display());
            app.path_text = "untitled.pcnd".into();
            app.editor.set_status(format!("Cannot open: {e}"));
        }
        app.editor.select_tool(Tool::Brush);
        app
    }

    /// Restore a previously chosen view scale instead of fitting the first frame.
    pub fn with_initial_zoom(mut self, zoom: f32) -> Self {
        if zoom.is_finite() {
            self.zoom = zoom.clamp(
                paintcraft_ui::actions::ZOOM_MIN,
                paintcraft_ui::actions::ZOOM_MAX,
            );
            self.initial_fit = false;
        }
        self
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

    /// Document-space bounds of the last installed display, including native GPU output.
    pub fn rendered_region(&self) -> Option<paintcraft_core::types::Rect> {
        self.texture_key
            .filter(|_| self.texture.is_some() || self.gpu_texture.is_some())
            .map(|key| key.3)
    }

    /// Tightly packed display dimensions; they may be smaller than source bounds.
    pub fn rendered_size(&self) -> Option<[u32; 2]> {
        self.rendered_region()
            .map(|_| Self::display_size(self.texture_key.unwrap()))
    }

    /// Document pixels covered by one display texel on the global sampling grid.
    pub fn rendered_sample_step(&self) -> Option<u32> {
        self.rendered_region()
            .map(|_| 1 << self.texture_key.unwrap().4)
    }

    /// Last CPU-backed display (straight RGBA8). Empty for native GPU display.
    pub fn rendered(&self) -> &[u8] {
        &self.rendered
    }
}
