//! Save-gated transitions between independent frontend document workspaces.
use super::*;
#[derive(Debug)]
pub(super) struct Switch {
    target: Experience,
    discard: bool,
    remaining: Vec<Uuid>,
    awaiting: Option<Uuid>,
    origin: Uuid,
}

impl App {
    pub(super) fn interface_switch_dialog(&mut self, ctx: &egui::Context, target: Experience) {
        let response=egui::Modal::new(egui::Id::unique("switch-interface-save")).show(ctx,|ui| {
            ui.heading("Save before switching interfaces");
            ui.label("Classic and Studio have separate document workspaces. Save changed documents, or discard them and switch.");
            ui.horizontal(|ui| {
                if ui.button("Save and switch").clicked() && ctx.current_pass_index()==0 {
                    self.confirm=None;
                    self.begin_interface_switch(target,ctx);
                }
                if ui.button("Discard and switch").clicked() && ctx.current_pass_index()==0 {
                    self.confirm=None;
                    self.start_interface_switch(target,true,ctx);
                }
                if ui.button("Cancel").clicked() && ctx.current_pass_index()==0 {self.confirm=None;}
            });
        });
        if response.should_close() && ctx.current_pass_index() == 0 {
            self.confirm = None;
        }
    }

    pub(super) fn begin_interface_switch(&mut self, target: Experience, ctx: &egui::Context) {
        self.start_interface_switch(target, false, ctx);
    }

    fn start_interface_switch(&mut self, target: Experience, discard: bool, ctx: &egui::Context) {
        self.switching = Some(Switch {
            discard,
            target,
            remaining: self.tabs.iter().map(|t| t.id).rev().collect(),
            awaiting: None,
            origin: self.active_tab_id(),
        });
        self.pump_interface_switch(ctx);
    }

    pub(super) fn pump_interface_switch(&mut self, ctx: &egui::Context) {
        if self.any_pending_edits()
            || self.any_deferred_files()
            || self.confirm.is_some()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            || self
                .saving
                .as_ref()
                .is_some_and(saving::NativeSaving::pending)
        {
            return;
        }
        let Some(mut transition) = self.switching.take() else {
            return;
        };
        if self
            .saving
            .as_ref()
            .is_some_and(saving::NativeSaving::failed)
        {
            self.cancel_interface_switch(
                transition.origin,
                "Resolve incomplete saves before switching interfaces.",
            );
            return;
        }
        if let Some(id) = transition.awaiting.take() {
            self.swap_tab_state(id);
            if self.editor.is_dirty() || self.has_document_drafts() {
                self.cancel_interface_switch(transition.origin,"Interface unchanged: saving was cancelled, failed, or newer edits remain unsaved.");
                return;
            }
        }
        while let Some(id) = transition.remaining.pop() {
            self.swap_tab_state(id);
            if self.editor.is_dirty() || self.has_document_drafts() {
                if transition.discard {
                    self.editor.cancel_gesture();
                    self.editor.cancel_text();
                    self.name_edit = None;
                    self.opacity_edit = None;
                    self.try_close_tab(id, ctx);
                    if self.tabs.iter().any(|tab| tab.id == id) {
                        transition.remaining.push(id);
                        if self
                            .saving
                            .as_ref()
                            .is_some_and(saving::NativeSaving::pending)
                        {
                            self.switching = Some(transition);
                        } else {
                            self.cancel_interface_switch(
                                transition.origin,
                                "Interface unchanged: the document could not be discarded safely.",
                            );
                        }
                        return;
                    }
                    continue;
                }
                self.resolve_tab_input(ctx);
                transition.awaiting = Some(id);
                self.switching = Some(transition);
                self.save_requested(ctx);
                ctx.request_repaint();
                return;
            }
        }
        self.swap_tab_state(transition.origin);
        if self.any_dirty_documents() {
            self.cancel_interface_switch(
                transition.origin,
                "Interface unchanged: documents still have unsaved changes.",
            );
            return;
        }
        match self.clean_all_recovery_before_close() {
            Ok(false) => self.adopt_experience(transition.target),
            Ok(true) => self.switching = Some(transition),
            Err(error) => self.cancel_interface_switch(
                transition.origin,
                &format!("Interface unchanged: {error}"),
            ),
        }
        ctx.request_repaint();
    }

    fn cancel_interface_switch(&mut self, origin: Uuid, message: &str) {
        self.swap_tab_state(origin);
        self.reset_tab_presentation();
        self.editor.set_status(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_gate_saves_each_changed_document_then_restores_its_own_workspace() {
        for native in [false, true] {
            let ctx = egui::Context::default();
            let dir = std::env::temp_dir().join(format!("paintcraft-switch-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let mut app = App::new(None);
            app.settings.experience = Experience::Studio;
            if native {
                app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
            }
            let mut snapshots = Vec::new();
            for i in 0..3 {
                if i > 0 {
                    app.new_document();
                }
                app.editor.request(i + 1, Action::AddLayer);
                app.editor.run_pending();
                app.path_text = dir
                    .join(format!("doc-{i}.pcnd"))
                    .to_string_lossy()
                    .into_owned();
                snapshots.push(app.editor.document_snapshot());
            }
            app.choose_experience(Experience::Classic);
            assert_eq!(app.experience(), Experience::Studio);
            assert_eq!(
                app.confirm,
                Some(Confirm::SwitchInterface(Experience::Classic))
            );
            app.confirm = None;
            app.begin_interface_switch(Experience::Classic, &ctx);
            for _ in 0..16 {
                app.editor.run_pending();
                app.finish_save();
                if native {
                    let start = std::time::Instant::now();
                    while app.saving.as_ref().unwrap().pending() {
                        app.poll_native_saves(&ctx);
                        assert!(start.elapsed() < Duration::from_secs(10));
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                app.pump_interface_switch(&ctx);
            }
            assert_eq!(app.experience(), Experience::Classic);
            assert_eq!(app.document_count(), 1);
            for (i, snapshot) in snapshots.iter().enumerate() {
                assert!(
                    Editor::open(&dir.join(format!("doc-{i}.pcnd")))
                        .unwrap()
                        .document()
                        .same_semantics(snapshot)
                );
                assert_ne!(app.editor.document().document_id, snapshot.document_id);
            }
            app.choose_experience(Experience::Studio);
            assert_eq!(app.document_count(), 3);
            assert!(!app.any_dirty_documents());
            assert!(app.editor.document().same_semantics(&snapshots[2]));
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn failed_save_keeps_the_source_workspace_and_does_not_adopt_the_new_interface() {
        let ctx = egui::Context::default();
        let mut app = App::new(None);
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let source = app.editor.document_snapshot();
        app.path_text = format!("/missing-paintcraft-{}/work.pcnd", Uuid::new_v4());
        app.choose_experience(Experience::Studio);
        assert!(matches!(
            app.confirm,
            Some(Confirm::SwitchInterface(Experience::Studio))
        ));
        app.confirm = None;
        app.begin_interface_switch(Experience::Studio, &ctx);
        app.editor.run_pending();
        app.finish_save();
        app.pump_interface_switch(&ctx);
        assert_eq!(app.experience(), Experience::Classic);
        assert!(app.switching.is_none());
        assert!(app.editor.is_dirty());
        assert!(app.editor.document().same_semantics(&source));
        assert!(app.other_workspace.is_none());
    }
}
