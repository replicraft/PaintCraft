use super::*;

#[test]
fn exact_experience_transfer_keeps_editor_history_tab_and_view_owners() {
    let mut app = App::new(None);
    let classic = app.editor.document_snapshot();
    let classic_ticket = app.editor.history_view().ticket;
    let classic_tab = app.active_tab_id();
    app.zoom = 2.0;
    app.adopt_experience(Experience::Studio);
    assert_eq!(app.experience(), Experience::Studio);
    let studio = app.editor.document_snapshot();
    let studio_ticket = app.editor.history_view().ticket;
    let studio_tab = app.active_tab_id();
    app.zoom = 3.0;
    app.adopt_experience(Experience::Classic);
    assert!(std::sync::Arc::ptr_eq(
        &classic,
        &app.editor.document_snapshot()
    ));
    assert_eq!(app.editor.history_view().ticket, classic_ticket);
    assert_eq!(app.active_tab_id(), classic_tab);
    assert_eq!(app.zoom, 2.0);
    app.adopt_experience(Experience::Studio);
    assert!(std::sync::Arc::ptr_eq(
        &studio,
        &app.editor.document_snapshot()
    ));
    assert_eq!(app.editor.history_view().ticket, studio_ticket);
    assert_eq!(app.active_tab_id(), studio_tab);
    assert_eq!(app.zoom, 3.0);
}

#[test]
fn inactive_bound_sources_are_protected_and_replaced_semantics_become_unsaved() {
    let mut app = App::new(None);
    let path = std::env::temp_dir().join(format!("paintcraft-keyed-{}.pcnd", Uuid::new_v4()));
    app.bound = Some(path.clone());
    let original = app.editor.document_snapshot();
    app.adopt_experience(Experience::Studio);
    assert!(app.document_source_paths().contains(&path.as_path()));
    assert!(app.other_document_destination(&path));
    app.destination_written(&path, &original, None);
    assert!(
        !app.parked_workspaces
            .get(Experience::Classic)
            .unwrap()
            .active
            .editor
            .is_dirty()
    );
    let changed = Editor::new_document(7, 3).unwrap().document_snapshot();
    app.destination_written(&path, &changed, None);
    let retained = &app
        .parked_workspaces
        .get(Experience::Classic)
        .unwrap()
        .active
        .editor;
    assert!(retained.is_dirty());
    assert!(std::sync::Arc::ptr_eq(
        &original,
        &retained.document_snapshot()
    ));
    assert!(
        !app.parked_workspaces
            .values()
            .all(|workspace| workspace.update_restart_idle())
    );
}

#[test]
fn adopted_inactive_uuid_is_not_invalidated_by_its_own_completion() {
    let mut app = App::new(None);
    let path =
        std::env::temp_dir().join(format!("paintcraft-keyed-adopted-{}.pcnd", Uuid::new_v4()));
    app.bound = Some(path.clone());
    let adopted = app.active_tab_id();
    app.adopt_experience(Experience::Studio);
    let changed = Editor::new_document(7, 3).unwrap().document_snapshot();
    app.destination_written(&path, &changed, Some(adopted));
    assert!(
        !app.parked_workspaces
            .get(Experience::Classic)
            .unwrap()
            .active
            .editor
            .is_dirty()
    );
}

#[test]
fn shared_path_save_refuses_close_until_retained_workspace_is_saved_separately() {
    let ctx = egui::Context::default();
    let dir = std::env::temp_dir().join(format!("paintcraft-keyed-close-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    {
        let mut app = App::new(None);
        let shared = dir.join("shared.pcnd");
        let separate = dir.join("classic.pcnd");
        app.bound = Some(shared.clone());
        let retained = app.editor.document_snapshot();
        app.adopt_experience(Experience::Studio);
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        app.do_save(&ctx, shared.clone());
        app.editor.run_pending();
        app.finish_save();
        assert!(shared.is_file(), "{}", app.editor.status());
        assert!(!app.editor.is_dirty());
        let classic = &app
            .parked_workspaces
            .get(Experience::Classic)
            .unwrap()
            .active
            .editor;
        assert!(classic.is_dirty());
        assert!(std::sync::Arc::ptr_eq(
            &retained,
            &classic.document_snapshot()
        ));
        app.request_workspace_close(&ctx);
        assert!(!app.closing_workspace);
        assert!(!app.allow_close);
        assert!(
            app.editor
                .status()
                .contains("Switch to the other interface")
        );
        app.begin_interface_switch(Experience::Classic, &ctx);
        assert_eq!(app.experience(), Experience::Classic);
        assert!(app.editor.is_dirty());
        app.do_save(&ctx, separate.clone());
        app.editor.run_pending();
        app.finish_save();
        assert!(separate.is_file(), "{}", app.editor.status());
        assert!(!app.editor.is_dirty());
        assert!(
            !app.parked_workspaces
                .get(Experience::Studio)
                .unwrap()
                .active
                .editor
                .is_dirty()
        );
        app.request_workspace_close(&ctx);
        assert!(app.closing_workspace);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn inactive_name_draft_refuses_close_until_draft_is_resolved() {
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    app.adopt_experience(Experience::Studio);
    let workspace = app.parked_workspaces.values_mut().next().unwrap();
    let id = workspace.active.editor.document().roots[0];
    workspace.active.name_edit = Some((id, "Retained draft".into(), Default::default()));
    assert!(app.refuse_close_with_inactive_changes(&ctx));
    app.parked_workspaces
        .values_mut()
        .next()
        .unwrap()
        .active
        .name_edit = None;
    assert!(!app.refuse_close_with_inactive_changes(&ctx));
}

#[test]
fn actual_close_refusal_uses_each_contexts_localized_message() {
    use paintcraft_ui::i18n::{Language, set_language, translate};
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.adopt_experience(Experience::Studio);
    let workspace = app.parked_workspaces.values_mut().next().unwrap();
    let id = workspace.active.editor.document().roots[0];
    workspace.active.name_edit = Some((id, "Keep draft".into(), Default::default()));
    let source =
        "Switch to the other interface and save or discard its changed documents before closing.";
    for language in Language::ALL {
        set_language(&ctx, language);
        assert!(app.refuse_close_with_inactive_changes(&ctx));
        let expected = translate(language, source);
        assert_eq!(app.editor.status(), expected);
        if language != Language::English {
            assert_ne!(expected, source);
        }
    }
}
