//! Document state is parked without cloning its editor or replacing its I/O owner.
use super::*;
use saving::sessions::SessionState;
#[path = "tabs/chrome.rs"]
mod chrome;
#[path = "tabs/closing.rs"]
mod closing;
#[cfg(test)]
#[path = "tabs/tests.rs"]
mod tests;

pub(super) const MAX_TABS: usize = 64;

#[derive(Debug)]
pub(super) struct Tab {
    pub id: Uuid,
    pub number: u64,
    pub(super) state: Option<DocumentState>,
}

impl Tab {
    pub(super) fn initial() -> Self {
        Self {
            id: Uuid::new_v4(),
            number: 1,
            state: None,
        }
    }
}

#[derive(Debug)]
pub(super) struct DocumentState {
    pub(super) editor: Editor,
    session: SessionState,
    zoom: f32,
    studio_fit: Option<view::StudioFit>,
    initial_fit: bool,
    pixel_grid: bool,
    pending_center: Option<(f32, f32)>,
    path_text: String,
    bound: Option<PathBuf>,
    pending_save: Option<(PathBuf, Option<PathBuf>)>,
    pub(super) deferred_files: std::collections::VecDeque<editing::DeferredFile>,
    save_activation: Option<u64>,
    recovery_needs_init: bool,
    recovery_source: Option<saving::restoring::Handoff>,
    last_autosave: f64,
    name_edit: Option<(Uuid, String)>,
    opacity_edit: Option<(Uuid, u8)>,
}

impl DocumentState {
    fn new(editor: Editor, recovery: bool) -> Self {
        Self {
            editor,
            session: Default::default(),
            zoom: 1.0,
            studio_fit: None,
            initial_fit: true,
            pixel_grid: false,
            pending_center: None,
            path_text: "untitled.pcnd".into(),
            bound: None,
            pending_save: None,
            deferred_files: Default::default(),
            save_activation: None,
            recovery_needs_init: recovery,
            recovery_source: None,
            last_autosave: 0.0,
            name_edit: None,
            opacity_edit: None,
        }
    }

    fn swap(&mut self, app: &mut App) {
        std::mem::swap(&mut self.editor, &mut app.editor);
        if let Some(service) = &app.editing {
            app.editor.connect_editing(service.clone());
        }
        std::mem::swap(&mut self.zoom, &mut app.zoom);
        std::mem::swap(&mut self.studio_fit, &mut app.studio_fit);
        std::mem::swap(&mut self.initial_fit, &mut app.initial_fit);
        std::mem::swap(&mut self.pixel_grid, &mut app.pixel_grid);
        std::mem::swap(&mut self.pending_center, &mut app.pending_center);
        std::mem::swap(&mut self.path_text, &mut app.path_text);
        std::mem::swap(&mut self.bound, &mut app.bound);
        std::mem::swap(&mut self.pending_save, &mut app.pending_save);
        std::mem::swap(&mut self.deferred_files, &mut app.deferred_files);
        std::mem::swap(&mut self.save_activation, &mut app.save_activation);
        std::mem::swap(&mut self.recovery_needs_init, &mut app.recovery_needs_init);
        std::mem::swap(&mut self.recovery_source, &mut app.recovery_source);
        std::mem::swap(&mut self.last_autosave, &mut app.last_autosave);
        std::mem::swap(&mut self.name_edit, &mut app.name_edit);
        std::mem::swap(&mut self.opacity_edit, &mut app.opacity_edit);
        if let Some(saving) = &mut app.saving {
            saving.swap_session(&mut self.session);
        }
    }
}

pub(crate) fn same_destination(a: &Path, b: &Path) -> bool {
    fn namespace(path: &Path) -> Option<PathBuf> {
        std::fs::canonicalize(path).ok().or_else(|| {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            Some(std::fs::canonicalize(parent).ok()?.join(path.file_name()?))
        })
    }
    a == b || namespace(a).is_some_and(|a| namespace(b).as_ref() == Some(&a))
}

impl App {
    pub(super) fn active_tab_id(&self) -> Uuid {
        self.tabs.get(self.active_tab).map_or(Uuid::nil(), |t| t.id)
    }

    /// Internal owner routing only; never resolves input, draws, or polls I/O.
    pub(super) fn swap_tab_state(&mut self, id: Uuid) -> bool {
        let Some(index) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        if index == self.active_tab {
            return true;
        }
        let mut next = self.tabs[index]
            .state
            .take()
            .expect("inactive document state");
        next.swap(self);
        self.tabs[self.active_tab].state = Some(next);
        self.active_tab = index;
        true
    }

    pub(crate) fn tab_for_session(&self, owner: Uuid) -> Option<Uuid> {
        if self
            .saving
            .as_ref()
            .is_some_and(|s| s.session_owner() == owner)
        {
            return self.tabs.get(self.active_tab).map(|t| t.id);
        }
        self.tabs
            .iter()
            .find(|t| t.state.as_ref().is_some_and(|s| s.session.owner() == owner))
            .map(|t| t.id)
    }

    pub(super) fn resolve_tab_input(&mut self, ctx: &egui::Context) {
        self.editor.cancel_gesture();
        self.editor.commit_text();
        if let Some((id, name)) = &self.name_edit
            && self
                .editor
                .layer_properties(*id)
                .is_some_and(|p| p.0 != *name)
        {
            self.request(
                ctx,
                code::RENAME,
                Action::Rename {
                    layer: *id,
                    name: name.clone(),
                },
            );
        }
        if let Some((id, opacity)) = self.opacity_edit
            && self
                .editor
                .layer_properties(id)
                .is_some_and(|p| p.1 != opacity)
        {
            self.request(
                ctx,
                code::OPACITY,
                Action::SetOpacity { layer: id, opacity },
            );
        }
        self.editor.run_pending();
        self.finish_save();
    }

    pub(super) fn reset_tab_presentation(&mut self) {
        if let Some((_, handle)) = self.pending_frame.take()
            && let Some(renderer) = &self.renderer
        {
            renderer.cancel(handle);
        }
        self.preview_revision = self
            .preview_revision
            .checked_add(1)
            .expect("preview revision exhausted");
        self.rendered_preview = None;
        self.rendered_image_paste = None;
        self.texture = None;
        self.gpu_texture = None;
        self.texture_key = None;
        self.render_region = None;
        self.render_level = 0;
        self.rendered.clear();
        self.canvas_rect = None;
        self.hover_doc = None;
    }

    pub(super) fn activate_tab(&mut self, id: Uuid, ctx: &egui::Context) {
        if self.active_tab_id() == id
            || self.confirm.is_some()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            return;
        }
        self.resolve_tab_input(ctx);
        if self.swap_tab_state(id) {
            self.reset_tab_presentation();
            ctx.request_repaint();
        }
    }

    pub(crate) fn check_tab_capacity(&self) -> paintcraft_core::types::Result<()> {
        if self.tabs.len() >= MAX_TABS || self.next_tab_number == u64::MAX {
            Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "Open-document limit reached; close a document before opening another",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn forget_recovery_offer(&mut self, session: Uuid) {
        self.recoverable.retain(|f| f.session != session);
    }

    pub(super) fn has_live_recovery(&self) -> bool {
        self.editor.recovery_handle().is_some()
            || self.tabs.iter().any(|t| {
                t.state
                    .as_ref()
                    .is_some_and(|s| s.editor.recovery_handle().is_some())
            })
    }

    pub(super) fn recovery_offer_busy(&self, found: &Found) -> bool {
        self.saving.as_ref().is_some_and(|s| s.offer_busy(found))
            || self
                .tabs
                .iter()
                .any(|t| t.state.as_ref().is_some_and(|s| s.session.recovery_busy()))
    }

    pub(super) fn other_document_destination(&self, path: &Path) -> bool {
        self.saving
            .as_ref()
            .is_some_and(|s| s.other_pending_destination(path))
            || self.tabs.iter().any(|t| {
                t.state.as_ref().is_some_and(|s| {
                    !s.editor.is_dirty()
                        && s.bound
                            .as_deref()
                            .is_some_and(|p| same_destination(p, path))
                })
            })
    }

    pub(crate) fn destination_written(
        &mut self,
        path: &Path,
        snapshot: &paintcraft_document::Document,
        adopted: Option<Uuid>,
    ) {
        self.recent.remember(path);
        let current = self.active_tab_id();
        if adopted != Some(current)
            && self
                .bound
                .as_deref()
                .is_some_and(|p| same_destination(p, path))
            && !self.editor.document().same_semantics(snapshot)
        {
            self.editor.invalidate_saved_state();
            self.last_autosave = f64::NEG_INFINITY;
            self.editor.set_status("Another open document replaced this file. This version remains unsaved; use Save As to keep it separately.");
        }
        for tab in &mut self.tabs {
            if adopted == Some(tab.id) {
                continue;
            }
            if let Some(state) = &mut tab.state
                && state
                    .bound
                    .as_deref()
                    .is_some_and(|p| same_destination(p, path))
                && !state.editor.document().same_semantics(snapshot)
            {
                state.editor.invalidate_saved_state();
                state.last_autosave = f64::NEG_INFINITY;
                state.editor.set_status("Another open document replaced this file. This version remains unsaved; use Save As to keep it separately.");
            }
        }
    }

    pub(super) fn tab_bound_to_path(&self, path: &Path) -> Option<Uuid> {
        let resolved = std::fs::canonicalize(path).ok();
        self.tabs
            .iter()
            .find(|t| {
                let bound = t
                    .state
                    .as_ref()
                    .map_or(self.bound.as_ref(), |s| s.bound.as_ref());
                bound.is_some_and(|bound| {
                    bound == path
                        || resolved
                            .as_ref()
                            .is_some_and(|p| std::fs::canonicalize(bound).ok().as_ref() == Some(p))
                })
            })
            .map(|t| t.id)
    }

    pub(super) fn add_document_tab(
        &mut self,
        editor: Editor,
        ctx: &egui::Context,
    ) -> paintcraft_core::types::Result<()> {
        if self.tabs.len() >= MAX_TABS {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "Open-document limit reached; close a document before opening another",
            ));
        }
        self.resolve_tab_input(ctx);
        self.next_tab_number = self.next_tab_number.checked_add(1).ok_or_else(|| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "Document numbering exhausted",
            )
        })?;
        let id = Uuid::new_v4();
        let mut state = DocumentState::new(
            editor.with_text_renderer(self.text_renderer.clone()),
            self.recovery_dir.is_some(),
        );
        if self.tabs.is_empty() {
            if let Some(saving) = &mut self.saving {
                saving.cancel_restore();
                saving.cancel_inspection();
            }
            state.swap(self);
        }
        let empty = self.tabs.is_empty();
        self.tabs.push(Tab {
            id,
            number: self.next_tab_number,
            state: if empty { None } else { Some(state) },
        });
        self.swap_tab_state(id);
        self.reset_tab_presentation();
        if self.recovery_needs_init {
            self.attach_recovery();
        }
        ctx.request_repaint();
        Ok(())
    }

    pub(super) fn any_dirty_documents(&self) -> bool {
        (!self.tabs.is_empty() && (self.editor.is_dirty() || self.has_document_drafts()))
            || self
                .tabs
                .iter()
                .any(|t| t.state.as_ref().is_some_and(|s| s.editor.is_dirty()))
    }

    pub(super) fn has_document_drafts(&self) -> bool {
        self.editor.gesture_in_progress()
            || self.editor.is_editing_text()
            || self.name_edit.as_ref().is_some_and(|(id, name)| {
                self.editor
                    .layer_properties(*id)
                    .is_some_and(|p| p.0 != *name)
            })
            || self.opacity_edit.is_some_and(|(id, opacity)| {
                self.editor
                    .layer_properties(id)
                    .is_some_and(|p| p.1 != opacity)
            })
    }

    pub fn document_count(&self) -> usize {
        self.tabs.len()
    }
}

impl App {
    pub(super) fn tick_documents(&mut self, ctx: &egui::Context) {
        if self.close_after_saves
            || self.closing_workspace
            || self.allow_close
            || ctx.current_pass_index() != 0
        {
            return;
        }
        let original = self.active_tab_id();
        let ids = self.tabs.iter().map(|t| t.id).collect::<Vec<_>>();
        for id in ids {
            self.swap_tab_state(id);
            if self.recovery_needs_init {
                self.attach_recovery();
                ctx.request_repaint_after(Duration::from_millis(250));
            }
            self.pump_restore();
            self.pump_recovery_handoff();
            self.tick_document(ctx);
        }
        self.swap_tab_state(original);
    }

    fn tick_document(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.last_autosave >= self.autosave_every.as_secs_f64()
            && !self.allow_close
            && !self.close_after_saves
        {
            let previous_autosave = self.last_autosave;
            self.last_autosave = now;
            if let Some(saving) = &mut self.saving {
                let result = if !self.editor.is_dirty() && self.editor.checkpoint_info().is_some() {
                    saving.clear_recovery(&mut self.editor).map(|()| false)
                } else {
                    saving.checkpoint(&self.editor)
                };
                if let Err(error) = result {
                    if error.code == paintcraft_core::types::ErrorCode::ResourceLimit
                        && saving.pending()
                    {
                        self.last_autosave = previous_autosave;
                        ctx.request_repaint_after(Duration::from_millis(50));
                    }
                    self.editor
                        .set_status(format!("Autosave not queued: {error}"));
                }
            } else if self.editor.autosave() {
                self.protect_recovery_source();
                self.pump_recovery_handoff();
            }
        }
    }
}

/// A frontend's document workspace. Only clean, idle sessions can be parked.
#[derive(Debug)]
pub(super) struct WorkspaceDocuments {
    pub(super) active: DocumentState,
    pub(super) tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_number: u64,
}
impl WorkspaceDocuments {
    pub(super) fn new(size: (u32, u32), recovery: bool) -> Self {
        Self {
            active: DocumentState::new(
                Editor::new_document(size.0, size.1).expect("validated default size"),
                recovery,
            ),
            tabs: vec![Tab::initial()],
            active_tab: 0,
            next_tab_number: 1,
        }
    }
    pub(super) fn swap(&mut self, app: &mut App) {
        self.active.swap(app);
        app.editor.text_renderer_connect(app.text_renderer.clone());
        if let Some(service) = &app.editing {
            app.editor.connect_editing(service.clone());
        }
        std::mem::swap(&mut self.tabs, &mut app.tabs);
        std::mem::swap(&mut self.active_tab, &mut app.active_tab);
        std::mem::swap(&mut self.next_tab_number, &mut app.next_tab_number);
        app.reset_tab_presentation();
    }
}
