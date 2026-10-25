//! Browser integration of shared Classic presentation and editor services.
use paintcraft_ui::t;
#[cfg(test)]
mod chrome_tests;
#[cfg(test)]
#[path = "crop_tests.rs"]
mod crop_tests;
mod documents;
#[cfg(test)]
#[path = "floating_tests.rs"]
mod floating_tests;
mod forms;
mod interfaces;
#[cfg(test)]
#[path = "lasso_tests.rs"]
mod lasso_tests;
mod presentation;
mod settings;
mod view;
use crate::{
    canvas::Canvas,
    editor::{Editor, WHITE},
};
use eframe::egui;
use paintcraft_classic::{ClassicVariant, Controls, Workspace};
use paintcraft_tools::{Action as Edit, Tool, ToolOptions};
use paintcraft_ui::actions::{Action, Context, Frontend, ImageExport, ViewAction};
use paintcraft_ui::i18n::{set_language, tr_status};

#[derive(Debug)]
pub struct WebApp {
    pub(crate) frontend: Frontend,
    pub(crate) studio: paintcraft_studio::Workspace,
    parked: Option<Box<interfaces::Parked>>,
    fullscreen: crate::fullscreen::State,
    pub(crate) printer: crate::printing::State,
    pub(crate) controls: Controls,
    pub(crate) editor: Editor,
    pub(crate) canvas: Canvas,
    pub(crate) workspace: Workspace,
    pub(crate) variant: ClassicVariant,
    name: String,
    pub(crate) zoom: f32,
    settings: bool,
    applied_style: Option<(egui::Theme, ClassicVariant, Frontend)>,
    settings_draft: Option<settings::Draft>,
    preferences: crate::preferences::Preferences,
    export_confirm: Option<ImageExport>,
    pub(crate) export_quality: u8,
    confirm_replace: Option<Action>,
    pub(crate) files: crate::files::Files,
    pub(crate) text: crate::text_ui::TextUi,
    pub(crate) pending_import: Option<crate::importing::Pending>,
    pub(crate) pending_pages: Option<crate::importing::pages::Pending>,
    pub(crate) import_owner: Option<crate::importing::Owner>,
    form: Option<forms::Form>,
    pub(crate) message: String,
    pub(crate) paste: crate::paste_ui::State,
    pub(crate) clipboard: crate::clipboard_ui::State,
}

impl WebApp {
    pub fn new(ctx: &egui::Context) -> Self {
        let (preferences, warning) = crate::preferences::Preferences::load();
        if let Some(theme) = preferences.theme {
            ctx.set_theme(theme);
        }
        let (width, height) = preferences.canvas.new_size();
        set_language(ctx, preferences.language);
        paintcraft_ui::fonts::install_for_locale(ctx, preferences.language.id());
        paintcraft_ui::theme::apply(ctx, ctx.theme());
        Self {
            frontend: Frontend::Classic,
            studio: Default::default(),
            parked: None,
            fullscreen: Default::default(),
            printer: Default::default(),
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
            applied_style: None,
            settings_draft: None,
            text: crate::text_ui::TextUi {
                style: paintcraft_classic::TextOptions {
                    family: preferences.default_font.clone(),
                    ..Default::default()
                },
                ..Default::default()
            },
            preferences,
            export_confirm: None,
            export_quality: 90,
            confirm_replace: None,
            files: crate::files::Files::default(),
            pending_import: None,
            pending_pages: None,
            import_owner: None,
            form: None,
            clipboard: Default::default(),
            paste: Default::default(),
            message: warning.unwrap_or_else(|| {
                "Ready - Save downloads a project copy; work stays in this tab".into()
            }),
        }
    }

    pub(crate) fn format_profile(&self) -> paintcraft_formats::registry::Frontend {
        use paintcraft_formats::registry::Frontend as Profile;
        match (self.frontend, self.variant) {
            (Frontend::Studio, _) => Profile::Studio,
            (_, ClassicVariant::V7) => Profile::ClassicV7,
            (_, ClassicVariant::V10) => Profile::ClassicV10,
            (_, ClassicVariant::V11) => Profile::ClassicV11,
        }
    }
    pub(crate) fn enabled_export_formats(&self) -> Vec<paintcraft_ui::actions::ImageExport> {
        use paintcraft_formats::registry::{self, Family, Operation, Platform, Request};
        paintcraft_ui::actions::ImageExport::ALL
            .into_iter()
            .filter(|format| {
                format.available(self.frontend)
                    && Family::from_extension(format.extension()).is_some_and(|family| {
                        registry::admission(Request::new(
                            self.format_profile(),
                            Operation::ExportFlat,
                            family,
                            Platform::Web,
                        ))
                        .available()
                    })
            })
            .collect()
    }
    pub(crate) fn blocked(&self) -> bool {
        self.printing_pending()
            || self.paste.modal()
            || self.settings
            || self.export_confirm.is_some()
            || self.confirm_replace.is_some()
            || self.files.busy
            || self.pending_import.is_some()
            || self.pending_pages.is_some()
            || self.form.is_some()
    }

    fn context(&self) -> Context {
        let doc = self.editor.session.document();
        let index = self
            .editor
            .target
            .and_then(|id| doc.roots.iter().position(|root| *root == id));
        Context {
            frontend: self.frontend,
            document_open: true,
            can_undo: self.editor.session.undo_depth() > 0 || self.editor.has_text(),
            can_redo: self.editor.session.redo_depth() > 0,
            can_recent: false,
            can_paste: self.paste.can_read() && !self.paste.busy() && !self.clipboard.busy(),
            can_paste_onto: self.paste.can_read() && !self.paste.busy() && !self.clipboard.busy(),
            can_save_all: false,
            selection_active: self.editor.session.selection()
                != paintcraft_editor::execute::Selection::Inactive,
            tool: self.controls.tool,
            blocked: self.blocked() || self.paste.is_placing(),
            layer_index: index,
            layer_count: doc.roots.len(),
            layer_visible: self
                .editor
                .target
                .and_then(|id| doc.layer(&id))
                .is_some_and(|l| l.visible),
            zoom: self.zoom,
            pixel_grid: self.canvas.pixel_grid,
            can_transform_image: !matches!(
                self.editor.session.selection(),
                paintcraft_raster::Selection::Mask(_) | paintcraft_raster::Selection::Empty
            ) || self.selected_bounds().is_ok_and(|bounds| bounds.is_some()),
            can_erase_selection: match self.editor.session.selection() {
                paintcraft_raster::Selection::Rect(r) => r.intersect(&doc.canvas_rect()).is_some(),
                paintcraft_raster::Selection::Mask(mask) => {
                    mask.uniform_coverage() != Some(0)
                        && mask.domain().intersect(&doc.canvas_rect()).is_some()
                }
                _ => false,
            },
            can_fit_selection: self.selected_bounds().is_ok_and(|bounds| bounds.is_some()),
            can_crop_selection: self.selected_bounds().is_ok_and(|bounds| bounds.is_some()),
            status_bar: self.frontend == Frontend::Studio || self.preferences.status_bar,
            rulers: self.rulers_visible(),
            fullscreen: self.fullscreen.active(),
            can_exit: false,
            desktop_download: self.frontend == Frontend::Classic
                && self.variant == ClassicVariant::V11,
            auto_hide_toolbar: self.frontend == Frontend::Classic
                && self.preferences.auto_hide_toolbar,
            thumbnail: self.workspace.thumbnail,
        }
    }

    pub(crate) fn edit(&mut self, action: Edit) {
        if self.paste.is_placing() {
            self.message = "Finish placing the pasted image before editing or saving.".into();
            return;
        }
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
        if self.paste.is_placing() {
            if action == Action::Undo {
                self.cancel_paste(ctx);
            } else {
                self.message = t!(
                    ctx,
                    "Finish placing the pasted image before editing or saving."
                );
            }
            return;
        }
        if matches!(
            action,
            Action::New
                | Action::Open
                | Action::ImportImage
                | Action::Save
                | Action::SaveAs
                | Action::Export
                | Action::ExportImage(_)
        ) && !self.floating_file_ready()
        {
            return;
        }
        if matches!(
            action,
            Action::New
                | Action::Open
                | Action::ImportImage
                | Action::Save
                | Action::SaveAs
                | Action::Export
                | Action::ExportImage(_)
        ) && self.editor.tools.is_constructing_shape()
        {
            self.message = paintcraft_tools::FINISH_SHAPE_BEFORE_FILE.into();
            return;
        }
        match action {
            Action::Print => self.print_requested(ctx),
            Action::Copy => self.copy_requested(ctx),
            Action::Cut => self.cut_requested(ctx),
            Action::CopyVisible => self.copy_visible_requested(ctx),
            Action::PasteLayer => self.paste_requested(ctx),
            Action::Paste => self.paste_current_requested(ctx),
            Action::Crop => self.edit(Edit::CropToSelection),
            Action::EraseSelection => self.edit(Edit::EraseSelection {
                color: self.clear_color(),
            }),
            Action::TransformImage(transform) => self.edit(Edit::TransformImage {
                transform,
                clear: self.clear_color(),
            }),
            Action::ChooseFrontend(frontend) if frontend == self.frontend => (),
            Action::ChooseFrontend(_) => {
                if self.unsaved() {
                    self.confirm_replace = Some(action);
                } else {
                    self.replace(ctx, action);
                }
            }
            Action::Settings | Action::Interface => {
                self.editor.tools.cancel_gesture();
                self.open_settings(ctx);
            }
            Action::ResizeSkew => self.begin_resize_skew(),
            Action::CanvasSize | Action::ImageProperties | Action::CanvasDefaults => {
                self.begin_canvas(action)
            }
            Action::EditColors => self.begin_colors(false, None),
            Action::EditSecondaryColors => self.begin_colors(true, None),
            Action::EditCustomColor(slot) => self.begin_colors(false, Some(slot)),
            Action::EditSecondaryCustomColor(slot) => self.begin_colors(true, Some(slot)),
            Action::SelectTool(tool) => {
                if tool != Tool::Magnifier
                    && !(self.editor.tools.tool == Tool::Magnifier && tool == Tool::Text)
                    && !self.finish_text()
                {
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
            Action::FlattenImage => {
                if self.finish_text() {
                    self.edit(Edit::FlattenImage);
                }
            }
            Action::ClearCanvas => self.edit(Edit::ClearCanvas {
                color: self.clear_color(),
            }),
            Action::Deselect => self.edit(Edit::Select { corners: None }),
            Action::InvertSelection => {
                self.editor.tools.cancel_gesture();
                self.edit(Edit::InvertSelection);
            }
            Action::SelectAll => {
                let (w, h) = self.editor.session.document().canvas;
                self.edit(Edit::Select {
                    corners: Some(((0, 0), (i64::from(w) - 1, i64::from(h) - 1))),
                });
            }
            Action::Export => {
                self.editor.tools.cancel_gesture();
                self.export_confirm = Some(ImageExport::Png);
            }
            Action::ExportImage(format) => {
                self.editor.tools.cancel_gesture();
                self.export_confirm = Some(format);
            }
            Action::Save | Action::SaveAs => {
                self.editor.tools.cancel_gesture();
                self.download(ctx, false);
            }
            Action::New | Action::Open | Action::ImportImage => {
                self.editor.tools.cancel_gesture();
                if self.unsaved() {
                    self.confirm_replace = Some(action);
                } else {
                    self.replace(ctx, action);
                }
            }
            Action::View(view) => self.view_action(ctx, view),
            _ => self.unsupported(action),
        }
        self.zoom = self.zoom.clamp(0.125, 10.0);
        ctx.request_repaint();
    }

    fn new_document(&mut self) {
        let (width, height) = if self.frontend == Frontend::Studio {
            (800, 600)
        } else {
            self.preferences.canvas.new_size()
        };
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
        if let Action::ChooseFrontend(frontend) = action {
            if self.confirm_replace == Some(action) && self.unsaved() {
                self.new_document();
            }
            self.switch_frontend(ctx, frontend);
            self.confirm_replace = None;
        } else if matches!(action, Action::Open | Action::ImportImage) {
            if self.files.busy {
                return;
            }
            self.import_owner = Some(crate::importing::Owner::capture(self));
            let frontend = self.format_profile();
            self.files
                .open(ctx, frontend, action == Action::ImportImage);
        } else {
            self.new_document();
        }
    }

    pub(crate) fn install(&mut self, loaded: crate::files::Loaded) {
        let converted = loaded
            .losses
            .iter()
            .any(|loss| paintcraft_formats::project_import::KNOWN_LOSSES.contains(&loss.as_str()));
        match Editor::from_document(loaded.document) {
            Ok(mut editor) => {
                if converted {
                    editor.session.mark_unsaved();
                }
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

    pub(crate) fn remember_canvas_size(&mut self, size: (u32, u32)) {
        if self.frontend == Frontend::Classic && self.preferences.canvas.last != Some(size) {
            self.preferences.canvas.last = Some(size);
            self.persist_preferences();
        }
    }

    fn poll_files(&mut self, ctx: &egui::Context) {
        use crate::files::Outcome;
        match self.files.poll() {
            Some(Ok(Outcome::Loaded(loaded))) => self.accept_loaded(loaded, ctx),
            Some(Ok(Outcome::Pages(source))) => self.accept_pages(source, ctx),
            Some(Ok(Outcome::Cancelled)) => {
                self.import_owner = None;
                self.message = "File dialog cancelled; current document retained".into()
            }
            Some(Ok(Outcome::DownloadRequested {
                owner,
                project_name,
            })) => {
                self.accept_download_name(owner, project_name);
                self.message =
                    "Download requested - verify your copy; this tab remains unsaved".into()
            }
            Some(Err(error)) => {
                self.import_owner = None;
                self.message = format!("File operation failed: {error}");
            }
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
        if self.paste_keyboard(ctx, event) {
            return true;
        }
        if self.paste.busy()
            && !self.paste.is_placing()
            && !self.paste.modal()
            && matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                }
            )
        {
            self.cancel_paste(ctx);
            return true;
        }
        if self.fullscreen.active()
            && !self.blocked()
            && !egui::Popup::is_any_open(ctx)
            && matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                }
            )
        {
            self.editor.cancel_floating();
            self.editor.tools.cancel_gesture();
            self.fullscreen.exit(ctx);
            return true;
        }
        if self.blocked()
            || self.text.widget_owned_input
            || ctx.egui_wants_keyboard_input()
            || egui::Popup::is_any_open(ctx)
            || !self.text.preedit.is_empty()
        {
            return false;
        }
        if matches!(event, egui::Event::Cut) && !self.editor.tools.is_editing_text() {
            if Action::Cut.enabled(self.context()) {
                self.action(ctx, Action::Cut);
            }
            return true;
        }
        if matches!(event, egui::Event::Copy) && !self.editor.tools.is_editing_text() {
            // Some integrations coalesce Copy with Ctrl/Cmd+Shift+C. Choose the
            // richer action before either event claims this frame's activation.
            let merged = ctx.input(|input| {
                input.events.iter().any(|event| {
                    matches!(event,
                egui::Event::Key {key: egui::Key::C, pressed: true, modifiers, ..}
                    if modifiers.matches_exact(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT))
                })
            });
            let action = if merged {
                Action::CopyVisible
            } else {
                Action::Copy
            };
            if action.enabled(self.context()) {
                self.action(ctx, action);
            }
            return true;
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
                    egui::Key::A
                        | egui::Key::B
                        | egui::Key::I
                        | egui::Key::U
                        | egui::Key::C
                        | egui::Key::X
                        | egui::Key::V
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

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_replace.is_some() {
            egui::Modal::new(egui::Id::unique("web-new-unsaved")).show(ctx, |ui| {
                ui.heading(t!(ui.ctx(), "Discard unsaved changes?"));
                ui.label(t!(ui.ctx(), "Download a project copy to keep your work. Browser downloads cannot confirm a disk save; discard explicitly when ready."));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(t!(ui.ctx(), "Download project")).clicked() {
                        self.download(ctx,false);
                    }
                    if ui.button(t!(ui.ctx(), "Discard and continue")).clicked()
                        && let Some(action) = self.confirm_replace.take()
                    { self.replace(ctx,action); }
                    if ui.button(t!(ui.ctx(), "Cancel")).clicked() { self.confirm_replace = None; }
                });
            });
        }
        if self.pending_pages.is_some() {
            self.page_dialog(ctx);
            return;
        }
        if self.pending_import.is_some() {
            let mut accept = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::unique("web-import-losses")).show(ctx, |ui| {
                ui.heading(t!(ui.ctx(), "Open with these import limitations?"));
                for loss in &self
                    .pending_import
                    .as_ref()
                    .expect("pending import")
                    .loaded
                    .losses
                {
                    if let Some(description) = paintcraft_ui::i18n::import_loss_description(loss) {
                        ui.label(t!(ctx, description));
                    }
                }
                ui.horizontal(|ui| {
                    accept = ui.button(t!(ui.ctx(), "Open image")).clicked();
                    cancel = ui.button(t!(ui.ctx(), "Cancel")).clicked();
                });
            });
            if ctx.current_pass_index() == 0
                && (accept || cancel || ctx.input(|input| input.key_pressed(egui::Key::Escape)))
            {
                self.resolve_import(ctx, accept);
            }
        }
        if let Some(mut format) = self.export_confirm {
            egui::Modal::new(egui::Id::unique("web-export-image")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading(t!(ui.ctx(), "Export { $format } copy?", &[("format", format.name().into())]));
                egui::ComboBox::from_label(t!(ui.ctx(), "File type")).selected_text(format.name()).show_ui(ui, |ui| {
                    for choice in self.enabled_export_formats() { ui.selectable_value(&mut format, choice, choice.name()); }
                });
                self.export_confirm = Some(format);
                ui.label(t!(ui.ctx(), format.description()));
                let (width, height) = self.editor.session.document().canvas;
                ui.label(t!(ui.ctx(), "Image dimensions: { $width } × { $height } px", &[("width", width.to_string()), ("height", height.to_string())]));
                if let Some(label) = format.quality_label() {
                    ui.add(egui::Slider::new(&mut self.export_quality, 1..=100).text(t!(ui.ctx(), label)));
                    ui.label(t!(ui.ctx(), if format == ImageExport::Avif {"AVIF uses lossy compression for colors and transparency, even at quality 100."} else {"Higher quality creates a larger file. Transparent pixels are placed on white."}));
                }
                ui.label(t!(ui.ctx(), "Image exports flatten visible layers and omit hidden layer data. Save a PaintCraft project to preserve editable layers."));
                ui.label(t!(ui.ctx(), "Invisible color information will not be retained."));
                if format == ImageExport::Jpeg { ui.label(t!(ui.ctx(), "JPEG uses lossy compression and cannot preserve transparency.")); }
                ui.horizontal(|ui| {
                    if ui.button(t!(ui.ctx(), "Download { $format }", &[("format", format.name().into())])).clicked() {
                        self.download_image(ctx, format);
                        self.export_confirm = None;
                    }
                    if ui.button(t!(ui.ctx(), "Cancel")).clicked() { self.export_confirm = None; }
                });
            });
        }
        self.settings_dialog(ctx);
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
