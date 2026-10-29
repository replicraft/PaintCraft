//! Desktop chrome integration.

use super::*;
use paintcraft_ui::t;

impl App {
    pub(super) fn action_context(&self) -> paintcraft_ui::actions::Context {
        use paintcraft_ui::actions::{Context, Frontend};
        let pending_text = self.editor.is_editing_text();
        let layer_index = self
            .editor
            .target()
            .and_then(|id| self.editor.document().roots.iter().position(|r| *r == id));
        let layer_visible = self
            .editor
            .target()
            .and_then(|id| self.editor.document().layer(&id))
            .is_some_and(|l| l.visible);
        Context {
            frontend: match self.experience() {
                Experience::Classic => Frontend::Classic,
                Experience::Studio => Frontend::Studio,
            },
            document_open: self.document_count() > 0,
            can_undo: self.editor.undo_depth() > 0
                || self.editor.has_image_paste()
                || pending_text
                || self.editor.has_pending_edits(),
            can_redo: (self.editor.redo_depth() > 0 || self.editor.has_pending_edits())
                && !pending_text,
            can_recent: true,
            can_paste: self.paste.supported() && !self.paste.busy() && !self.clipboard.busy(),
            can_paste_onto: self.paste.supported()
                && !self.paste.busy()
                && !self.clipboard.busy()
                && !self.editor.has_image_paste(),
            can_save_all: self.tab_close_wait.is_none()
                && self.tab_save_close.is_none()
                && (self.any_dirty_documents()
                    || self.any_pending_edits()
                    || self.any_deferred_files()),
            selection_active: self.editor.selection() != Selection::Inactive,
            tool: self.editor.tool,
            layer_index,
            layer_count: self.editor.document().roots.len(),
            layer_visible,
            zoom: self.zoom,
            pixel_grid: self.pixel_grid,
            can_transform_image: !matches!(
                self.editor.selection(),
                Selection::Mask(_) | Selection::Empty
            ) || self.selected_bounds().is_ok_and(|bounds| bounds.is_some()),
            can_erase_selection: match self.editor.selection() {
                Selection::Rect(r) => r.intersect(&self.editor.document().canvas_rect()).is_some(),
                Selection::Mask(mask) => {
                    mask.uniform_coverage() != Some(0)
                        && mask
                            .domain()
                            .intersect(&self.editor.document().canvas_rect())
                            .is_some()
                }
                _ => false,
            },
            can_fit_selection: self.selected_bounds().is_ok_and(|bounds| bounds.is_some()),
            can_crop_selection: self.selected_bounds().is_ok_and(|bounds| bounds.is_some()),
            status_bar: self.settings.classic_status_bar,
            rulers: self.settings.classic_rulers,
            fullscreen: self.fullscreen,
            can_exit: true,
            desktop_download: false,
            auto_hide_toolbar: self.settings.classic_toolbar_auto_hide,
            thumbnail: self.classic.thumbnail,
            blocked: self.save_all.is_some()
                || self.picking.is_some()
                || self.switching.is_some()
                || self.closing_workspace
                || self.close_after_saves
                || self.close_after_edits
                || !self.deferred_files.is_empty()
                || self.confirm.is_some()
                || self.tab_closing.is_some()
                || self.dialogs.as_ref().is_some_and(NativeDialogs::pending),
        }
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.experience() == Experience::Studio {
            ui.horizontal(|ui| {
                ui.strong(t!(ui.ctx(), "PaintCraft Studio"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if paintcraft_ui::widgets::icon_button(
                        ui,
                        "Settings…",
                        paintcraft_ui::icons::Icon::Settings,
                        false,
                        false,
                    )
                    .clicked()
                    {
                        self.dispatch_chrome(&ctx, vec![paintcraft_ui::actions::Action::Settings]);
                    }
                    let actions =
                        paintcraft_ui::chrome::interface_changer(ui, self.action_context());
                    self.dispatch_chrome(&ctx, actions);
                });
            });
        }
        let mut controls = paintcraft_ui::chrome::Controls {
            tool: self.editor.tool,
            options: self.editor.options,
            primary: self.editor.color,
            secondary: self.editor.secondary,
        };
        self.studio.resource_shape = self.editor.resource_shape_name().map(str::to_owned);
        self.studio.sampled_brush = self.editor.adopted_brush.as_ref().map(|adopted| {
            (
                self.editor.sampled_brush_name().unwrap_or("").into(),
                (adopted.tip().width(), adopted.tip().height()),
            )
        });
        let action_context = self.action_context();
        self.classic.custom_colors = self.settings.custom_colors;
        let text_before = self.settings.text.clone();
        let fonts_loading = self.fonts.loading();
        let actions = ui
            .push_id(("document-chrome", self.active_tab_id()), |ui| {
                match self.experience() {
                    Experience::Classic => paintcraft_classic::chrome_with_text(
                        ui,
                        self.variant(),
                        &mut controls,
                        &mut self.classic,
                        action_context,
                        Some(paintcraft_classic::TextControls {
                            options: &mut self.settings.text,
                            families: &self.fonts.families,
                            search: &mut self.fonts.search,
                            loading: fonts_loading,
                        }),
                    ),
                    Experience::Studio => paintcraft_studio::toolbar(
                        ui,
                        &mut controls,
                        &mut self.studio,
                        action_context,
                    ),
                }
            })
            .inner;
        self.apply_chrome(&ctx, controls, actions);
        self.finish_text_preferences(ui, &text_before);
        if self.editor.tool == Tool::Text
            && (self.experience() == Experience::Studio
                || (self.variant() == ClassicVariant::V11 && self.classic.toolbar.visible()))
        {
            self.classic_text_options(ui);
        }
        if self.editor.tool == Tool::Text {
            self.editor.set_text_style(self.settings.text.clone());
        }
        if self.experience() == Experience::Classic {
            self.classic.toolbar.remember_bounds(ui.min_rect());
        }
    }

    fn apply_chrome(
        &mut self,
        ctx: &egui::Context,
        controls: paintcraft_ui::chrome::Controls,
        actions: Vec<paintcraft_ui::chrome::Action>,
    ) {
        // Controls belong to the document that produced this frame. An action
        // below may activate another document with its own tool preferences.
        self.editor.options = controls.options;
        self.editor.color = controls.primary;
        self.editor.secondary = controls.secondary;
        self.dispatch_chrome(ctx, actions);
    }

    pub(super) fn dispatch_chrome(
        &mut self,
        ctx: &egui::Context,
        actions: Vec<paintcraft_ui::chrome::Action>,
    ) {
        for action in actions {
            self.chrome_outbox
                .push(token(ctx, action.activation_code()), action);
        }
        for action in self.chrome_outbox.drain() {
            // Earlier events in a coalesced frame may have queued an edit or
            // history move. Its result determines this action's enabled state.
            if !matches!(
                action,
                paintcraft_ui::chrome::Action::Copy
                    | paintcraft_ui::chrome::Action::CopyVisible
                    | paintcraft_ui::chrome::Action::Cut
                    | paintcraft_ui::chrome::Action::PasteLayer
                    | paintcraft_ui::chrome::Action::Paste
            ) {
                self.editor.run_pending();
            }
            if !action.enabled(self.action_context()) {
                continue;
            }
            use paintcraft_ui::chrome::Action as Chrome;
            if self.editor.has_image_paste() {
                match action {
                    Chrome::Undo => self.cancel_paste(ctx),
                    Chrome::View(_) | Chrome::Settings => {}
                    _ => {
                        self.editor.set_status(t!(
                            ctx,
                            "Finish placing the pasted image before editing or saving."
                        ));
                        continue;
                    }
                }
                if matches!(action, Chrome::Undo) {
                    continue;
                }
            }
            match action {
                Chrome::New => {
                    if self.experience() == Experience::Classic
                        && (self.editor.is_dirty() || self.has_document_drafts())
                    {
                        self.confirm = Some(Confirm::DiscardForNew);
                    } else {
                        self.new_document();
                    }
                }
                Chrome::Open => self.open_requested(ctx),
                Chrome::ImportImage => {
                    if self.dialogs.is_some() {
                        self.request_dialog(ctx, Kind::Import);
                    } else {
                        self.import_selected(PathBuf::from(self.path_text.trim()));
                    }
                }
                Chrome::RecentFiles => self.begin_recent_files(ctx),
                Chrome::Save => self.save_requested(ctx),
                Chrome::SaveAll => self.begin_save_all(ctx),
                Chrome::SaveAs => self.request_dialog(ctx, Kind::SaveAs),
                Chrome::Export => self.export_requested(ctx),
                Chrome::Print => self.print_requested(ctx),
                Chrome::Copy => self.copy_requested(ctx),
                Chrome::Cut => self.cut_requested(ctx),
                Chrome::PasteLayer => self.paste_layer_requested(ctx),
                Chrome::Paste => self.paste_requested(ctx),
                Chrome::CopyVisible => self.copy_source_requested(ctx, true),
                Chrome::ExportImage(format) => self.export_image_requested(ctx, format),
                Chrome::ExportIcons => self.begin_icon_export(),
                Chrome::ExportIcns => self.begin_icns_export(),
                Chrome::Palettes => self.begin_palettes(),
                Chrome::BrushResources => self.begin_brushes(),
                Chrome::ShapeResources => self.begin_shapes(),
                Chrome::Undo => self.request(ctx, code::UNDO, Action::Undo),
                Chrome::Redo => self.request(ctx, code::REDO, Action::Redo),
                Chrome::View(action) => self.dispatch_view_action(ctx, action),
                Chrome::Layer(action) => self.dispatch_layer_action(ctx, action),
                Chrome::Close => self.request_close_tab(self.active_tab_id(), ctx),
                Chrome::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Chrome::Settings => self.open_settings(ctx),
                Chrome::ChooseFrontend(frontend) => self.choose_experience(match frontend {
                    paintcraft_ui::actions::Frontend::Classic => Experience::Classic,
                    paintcraft_ui::actions::Frontend::Studio => Experience::Studio,
                }),
                Chrome::Interface => self.picking = Some(self.variant()),
                Chrome::SelectTool(tool) => self.editor.select_tool(tool),
                Chrome::InvertSelection => {
                    self.editor.cancel_gesture();
                    self.request(ctx, action.activation_code(), Action::InvertSelection);
                }
                Chrome::SelectAll => {
                    let (w, h) = self.editor.document().canvas;
                    self.request(
                        ctx,
                        code::SELECT_ALL,
                        Action::Select {
                            corners: Some(((0, 0), (i64::from(w) - 1, i64::from(h) - 1))),
                        },
                    );
                }
                Chrome::EditColors | Chrome::EditSecondaryColors => {
                    self.begin_color_dialog(ctx, action == Chrome::EditSecondaryColors)
                }
                Chrome::EditCustomColor(slot) => self.begin_custom_color_dialog(ctx, slot, false),
                Chrome::EditSecondaryCustomColor(slot) => {
                    self.begin_custom_color_dialog(ctx, slot, true)
                }
                Chrome::ClearCanvas => {
                    let color = match self.experience() {
                        Experience::Classic => self.editor.canvas_color(),
                        Experience::Studio => Rgba::TRANSPARENT,
                    };
                    self.request(ctx, code::CLEAR_CANVAS, Action::ClearCanvas { color });
                }
                Chrome::FlattenImage => {
                    if self.ime_composing {
                        continue;
                    }
                    self.editor.commit_text();
                    self.request(ctx, action.activation_code(), Action::FlattenImage);
                }
                Chrome::ResizeSkew => self.begin_resize_skew(),
                Chrome::Crop => self.request(ctx, code::CROP, Action::CropToSelection),
                Chrome::EraseSelection => {
                    let color = if self.experience() == Experience::Classic {
                        self.editor.canvas_color()
                    } else {
                        Rgba::TRANSPARENT
                    };
                    self.request(
                        ctx,
                        action.activation_code(),
                        Action::EraseSelection { color },
                    );
                }
                Chrome::TransformImage(transform) => {
                    let clear = if self.experience() == Experience::Classic {
                        self.editor.canvas_color()
                    } else {
                        Rgba::TRANSPARENT
                    };
                    self.request(
                        ctx,
                        action.activation_code(),
                        Action::TransformImage { transform, clear },
                    );
                }
                Chrome::ImageProperties => self.begin_image_properties(),
                Chrome::CanvasSize | Chrome::CanvasDefaults => {
                    self.begin_canvas_dialog(ctx, action == Chrome::CanvasDefaults)
                }
                Chrome::Deselect => {
                    self.request(ctx, code::DESELECT, Action::Select { corners: None })
                }
            }
        }
    }

    pub(super) fn studio_windows(&mut self, ctx: &egui::Context) {
        self.studio.canvas_top = self.canvas_viewport.map_or(150.0, |rect| rect.top());
        let mut controls = paintcraft_ui::chrome::Controls {
            tool: self.editor.tool,
            options: self.editor.options,
            primary: self.editor.color,
            secondary: self.editor.secondary,
        };
        let enabled = self.document_count() > 0 && !self.closing_workspace;
        let state_id = egui::Id::unique(("document-controls", self.active_tab_id()));
        let actions = paintcraft_studio::floating_controls(
            ctx,
            &mut controls,
            &mut self.studio,
            enabled,
            state_id,
        );
        self.apply_chrome(ctx, controls, actions);
        let mut open = self.studio.layers;
        paintcraft_studio::layers_window(ctx, &mut open, self.studio.canvas_top, |ui| {
            if self.closing_workspace {
                ui.disable();
            }
            self.layer_panel(ui)
        });
        self.studio.layers = open;
        self.studio_history(ctx);
    }

    pub(super) fn confirm_dialog(&mut self, ui: &mut egui::Ui) {
        if self.confirm == Some(Confirm::Brushes) {
            self.brush_dialog(ui.ctx());
            return;
        }
        if self.confirm == Some(Confirm::Shapes) {
            self.shape_dialog(ui.ctx());
            return;
        }
        if self.confirm == Some(Confirm::Palettes) {
            self.palette_dialog(ui.ctx());
            return;
        }
        if self.confirm == Some(Confirm::PasteLayer) {
            self.paste_layer_dialog(ui.ctx());
            return;
        }
        if self.confirm == Some(Confirm::ImportLosses) {
            self.import_dialog(ui.ctx());
            return;
        }
        if let Some(Confirm::SwitchInterface(target)) = self.confirm.clone() {
            self.interface_switch_dialog(ui.ctx(), target);
            return;
        }
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
        if confirm == Confirm::RecentFiles {
            self.recent_files_dialog(&ctx);
            return;
        }
        if confirm == Confirm::IcnsExport {
            self.icns_export_dialog(&ctx);
            return;
        }
        if confirm == Confirm::MspExport {
            self.msp_export_dialog(&ctx);
            return;
        }
        if confirm == Confirm::DdsExport {
            self.dds_export_dialog(&ctx);
            return;
        }
        if confirm == Confirm::JxrExport {
            self.jxr_export_dialog(&ctx);
            return;
        }
        if confirm == Confirm::IconExport {
            self.icon_export_dialog(&ctx);
            return;
        }
        if let Confirm::ExportOptions(draft) = confirm {
            self.export_options_dialog(&ctx, draft);
            return;
        }
        if let Confirm::Settings(draft) = confirm {
            self.settings_dialog(&ctx, draft);
            return;
        }
        if let Confirm::Colors(draft) = confirm {
            self.classic_color_dialog(&ctx, draft);
            return;
        }
        if let Confirm::ResizeSkew(draft) = confirm {
            self.resize_skew_dialog(&ctx, draft);
            return;
        }
        if let Confirm::Canvas(draft) = confirm {
            self.canvas_settings_dialog(&ctx, draft);
            return;
        }
        if let Confirm::LayerProperties(draft) = confirm {
            self.layer_properties_dialog(&ctx, draft);
            return;
        }
        let (title, text, go) = match &confirm {
            Confirm::Brushes | Confirm::Shapes | Confirm::Palettes | Confirm::PasteLayer | Confirm::ImportLosses | Confirm::RecentFiles | Confirm::ExportOptions(_) | Confirm::IcnsExport | Confirm::IconExport | Confirm::JxrExport | Confirm::DdsExport | Confirm::MspExport | Confirm::Settings(_) | Confirm::SwitchInterface(_) | Confirm::Colors(_) | Confirm::Canvas(_) | Confirm::ResizeSkew(_) | Confirm::LayerProperties(_) => unreachable!("properties handled above"),
            Confirm::DiscardForNew
            | Confirm::DiscardForOpen(_)
            | Confirm::DiscardForRecovery(_)
            | Confirm::DiscardForSaveSnapshot(_) => (
                "Discard changes?",
                "The current document has unsaved changes.".to_owned(),
                "Continue",
            ),
            Confirm::ForgetSaveSnapshot(_) => (
                "Forget captured save snapshot?",
                "This discards the snapshot retained for an incomplete save. The destination may have changed, and forgetting does not establish that it was saved safely.".to_owned(),
                "Forget snapshot",
            ),
            Confirm::ReplaceFile(p) => (
                "Replace file?",
                format!(
                    "{} already exists and is not this document's file.",
                    p.display()
                ),
                "Continue",
            ),
            Confirm::ReplaceSharedFile(p) => (
                "Replace a shared file?",
                format!("Another open document uses {}. Saving here can replace its saved contents.",p.display()),
                "Continue",
            ),
            Confirm::QuitUnsaved => {
                let incomplete = self
                    .saving
                    .as_ref()
                    .is_some_and(saving::NativeSaving::failed);
                (if incomplete { "Quit with an incomplete save?" } else { "Quit without saving?" },
                if incomplete {
                    if self.saving.as_ref().is_some_and(saving::NativeSaving::has_volatile_attempts) {
                        "A requested save failed or is unconfirmed. Some captured snapshots could not be stored for restart recovery; quitting abandons those in-memory snapshots. Available crash recovery data will be kept."
                    } else {
                        "A requested save or recovery operation failed or is unconfirmed. Captured snapshots already stored for restart recovery will be kept."
                    }
                }
                else { "There are unsaved changes. Available recovery data will be kept." }.to_owned(), "Quit anyway")
            }
            Confirm::Export {
                path,
                format,
                losses,
                exists,
                ..
            } => {
                let mut t = t!(&ctx, "Export { $path } as { $format }.", &[("path", path.display().to_string()), ("format", format.name().into())]);
                for loss in losses {
                    let explanation = paintcraft_ui::i18n::import_loss_description(loss).unwrap_or(match loss.as_str() {
                        "flatten" => "Visible layers will be flattened into one picture.",
                        "hidden-rgb" => "Invisible color information will not be retained.",
                        "lossy-jpeg" => "JPEG uses lossy compression.",
                        "lossy-avif" => "AVIF uses lossy compression.",
                        "alpha-lossy" => "Transparency may change during AVIF compression.",
                        "alpha-discarded" => "Transparency will not be preserved.",
                        "palette-quantized" => "Colors will be reduced to a GIF palette.",
                        "alpha-binarized" => "Partial transparency will become fully transparent or opaque.",
                        "transparent-rgb-discarded" => "Invisible color information will not be retained.",
                        _ => loss,
                    });
                    t.push('\n');
                    t.push_str(&t!(&ctx, explanation));
                }
                if *exists { t.push('\n'); t.push_str(&t!(&ctx, "The file exists and will be replaced.")); }
                ("Export", t, "Export now")
            }
        };
        use paintcraft_ui::confirmation::{Choice, Prompt};
        let choice = paintcraft_ui::confirmation::show(
            &ctx,
            egui::Id::unique(("confirmation", title)),
            Prompt {
                title,
                message: &text,
                accept: go,
                discard: None,
            },
        );
        if ctx.current_pass_index() != 0 {
            return;
        }
        match choice {
            Some(Choice::Accept) => {
                self.confirm = None;
                match confirm {
                    Confirm::PasteLayer
                    | Confirm::ImportLosses
                    | Confirm::RecentFiles
                    | Confirm::ExportOptions(_)
                    | Confirm::Brushes
                    | Confirm::Shapes
                    | Confirm::Palettes
                    | Confirm::IcnsExport
                    | Confirm::IconExport
                    | Confirm::JxrExport
                    | Confirm::DdsExport
                    | Confirm::MspExport
                    | Confirm::Settings(_)
                    | Confirm::SwitchInterface(_)
                    | Confirm::Colors(_)
                    | Confirm::Canvas(_)
                    | Confirm::ResizeSkew(_)
                    | Confirm::LayerProperties(_) => {
                        unreachable!("properties handled above")
                    }
                    Confirm::DiscardForNew => self.new_document(),
                    Confirm::DiscardForOpen(p) => {
                        if let Err(e) = self.open_path(&p) {
                            self.editor.set_status(format!("Cannot open: {e}"));
                        }
                    }
                    Confirm::DiscardForRecovery(item) => self.do_recover(item),
                    Confirm::DiscardForSaveSnapshot(sequence) => self.open_saved_snapshot(sequence),
                    Confirm::ForgetSaveSnapshot(sequence) => self.forget_incomplete_save(sequence),
                    Confirm::ReplaceFile(p) | Confirm::ReplaceSharedFile(p) => {
                        self.do_save(&ctx, p)
                    }
                    Confirm::QuitUnsaved => {
                        self.allow_close = true;
                        self.close_after_requested_saves(&ctx);
                    }
                    Confirm::Export {
                        path,
                        format,
                        losses,
                        jpeg_quality,
                        jpeg_matte,
                        pcx_encoding,
                        source,
                        frozen,
                        metadata_policy,
                        ..
                    } => {
                        if source == self.editor.preview_stamp() && frozen.current(self) {
                            self.do_export_frozen(
                                &path,
                                format,
                                &losses,
                                export_options::Settings {
                                    jpeg_quality,
                                    jpeg_matte,
                                    pcx_encoding,
                                    source_metadata: Default::default(),
                                },
                                frozen,
                                metadata_policy,
                            )
                        } else {
                            self.editor.set_status(t!(
                                &ctx,
                                "The document changed; choose the image again."
                            ));
                        }
                    }
                }
            }
            Some(Choice::Cancel) => self.confirm = None,
            Some(Choice::Discard) => unreachable!("this prompt has no discard action"),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_keeps_its_own_colors_instead_of_outgoing_controls() {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        let ctx = egui::Context::default();
        let outgoing = app.active_tab_id();
        let controls = paintcraft_ui::chrome::Controls {
            tool: app.editor.tool,
            options: app.editor.options,
            primary: Rgba([12, 34, 56, 255]),
            secondary: Rgba([65, 43, 21, 255]),
        };
        let default_colors = (app.editor.color, app.editor.secondary);
        app.apply_chrome(&ctx, controls, vec![paintcraft_ui::chrome::Action::New]);
        assert_eq!(app.document_count(), 2);
        assert_eq!((app.editor.color, app.editor.secondary), default_colors);
        app.activate_tab(outgoing, &ctx);
        assert_eq!(app.editor.color, Rgba([12, 34, 56, 255]));
        assert_eq!(app.editor.secondary, Rgba([65, 43, 21, 255]));
    }
}
