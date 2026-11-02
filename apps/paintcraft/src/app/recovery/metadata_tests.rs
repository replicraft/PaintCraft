use super::*;
use paintcraft_core::types::{ErrorCode, resources::SharedBudget};
use std::fs;
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("paintcraft-recovery-metadata-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn orphan(path: &std::path::Path) -> Found {
    let doc = paintcraft_formats::pcnd::read(include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    )).unwrap();
    let recovery = Recovery::start(path).unwrap();
    recovery.checkpoint_document(&doc, None).unwrap();
    drop(recovery);
    Recovery::scan(path).remove(0)
}
#[test]
fn synchronous_recovery_cannot_bypass_the_actual_host_metadata_quota() {
    let folder = Folder::new();
    let offer = orphan(&folder.0);
    let payload = fs::read(&offer.checkpoint).unwrap();
    let mut app = App::new(None);
    app.source_metadata_budget = SharedBudget::new(0);
    app.recoverable.push(offer.clone());
    let before = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    assert_eq!(
        app.recover_found(&offer).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(app.editor.document().same_semantics(&before));
    assert_eq!(app.editor.history_view().ticket, history);
    assert_eq!(app.recoverable, vec![offer.clone()]);
    assert_eq!(fs::read(&offer.checkpoint).unwrap(), payload);
    assert!(app.recovery_source.is_none());
}

#[test]
fn discovery_preservation_notices_use_every_supported_locale() {
    use paintcraft_core::types::Error;
    use paintcraft_ui::i18n::{Language, set_language, translate};
    let cases = [
        (
            ErrorCode::ResourceLimit,
            "Crash recovery discovery reached a resource limit. Existing recovery copies were kept.",
        ),
        (
            ErrorCode::OperationCancelled,
            "Crash recovery discovery was cancelled. Existing recovery copies were kept.",
        ),
        (
            ErrorCode::StorageFailed,
            "Crash recovery discovery failed. Existing recovery copies were kept.",
        ),
    ];
    for language in Language::ALL {
        let mut app = App::new(None);
        set_language(&app.context, language);
        for (code, text) in cases {
            app.apply_recovery_discovery(Err(Error::new(code, "diagnostic")));
            assert_eq!(app.editor.status(), translate(language, text));
            if language != Language::English {
                assert_ne!(app.editor.status(), text);
            }
        }
        let text = "Some recovery copies need a newer PaintCraft version. Those copies were kept.";
        app.apply_recovery_discovery(Ok(paintcraft_formats::recovery::ScanReport {
            unsupported_sessions: 1,
            ..Default::default()
        }));
        assert_eq!(app.editor.status(), translate(language, text));
        if language != Language::English {
            assert_ne!(app.editor.status(), text);
        }
    }
}

#[test]
fn denied_recovery_keeps_a_preexisting_private_text_draft_and_history_untouched() {
    let folder = Folder::new();
    let offer = orphan(&folder.0);
    let payload = fs::read(&offer.checkpoint).unwrap();
    let mut app = App::new(None);
    app.source_metadata_budget = SharedBudget::new(0);
    app.recoverable.push(offer.clone());
    app.editor.tool = Tool::Text;
    app.editor.pointer_down((3, 2));
    app.editor.type_text("keep this private draft");
    let document = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    let stamp = app.editor.preview_stamp();
    let text = app.editor.canvas_text_content().unwrap().to_owned();
    app.do_recover(offer.clone());
    assert!(
        app.editor.is_editing_text(),
        "failed recovery must not finish the pre-existing draft"
    );
    assert_eq!(app.editor.canvas_text_content(), Some(text.as_str()));
    assert_eq!(app.editor.preview_stamp(), stamp);
    assert!(app.editor.document().same_semantics(&document));
    assert_eq!(app.editor.history_view().ticket, history);
    assert_eq!(app.recoverable, vec![offer.clone()]);
    assert_eq!(fs::read(&offer.checkpoint).unwrap(), payload);
}

#[test]
fn classic_discard_confirmation_does_not_publish_private_text_and_success_still_recovers() {
    let folder = Folder::new();
    let offer = orphan(&folder.0);
    let mut app = App::new(None);
    app.settings.experience = Experience::Classic;
    app.editor
        .acknowledge_saved_state(app.editor.document().state_id);
    assert!(!app.editor.is_dirty());
    app.editor.tool = Tool::Text;
    app.editor.pointer_down((3, 2));
    app.editor.type_text("private until discard accepted");
    let before = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    let stamp = app.editor.preview_stamp();
    app.recover_requested(offer.clone());
    assert!(matches!(&app.confirm,Some(Confirm::DiscardForRecovery(found)) if found==&offer));
    assert!(app.editor.is_editing_text());
    assert_eq!(app.editor.preview_stamp(), stamp);
    assert!(app.editor.document().same_semantics(&before));
    assert_eq!(app.editor.history_view().ticket, history);
    // Cancel leaves the exact private draft, then a second explicit choice accepts discard.
    app.confirm = None;
    app.recover_requested(offer.clone());
    let Some(Confirm::DiscardForRecovery(accepted)) = app.confirm.take() else {
        panic!("expected discard decision")
    };
    app.do_recover(accepted);
    assert!(app.editor.document().source_metadata.is_some());
    assert!(app.editor.is_dirty());
    assert!(!app.editor.is_editing_text());
    assert_eq!(app.editor.document().document_id, offer.document_id);
}

fn blend_orphan(
    folder: &Folder,
    mode: paintcraft_core::types::LayerBlendMode,
) -> (Found, paintcraft_document::Document, Vec<u8>) {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(paintcraft_core::types::Rgba::TRANSPARENT),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = mode;
    let recovery = Recovery::start(&folder.0).unwrap();
    recovery.checkpoint_document(&doc, None).unwrap();
    drop(recovery);
    let offer = Recovery::scan(&folder.0).remove(0);
    let payload = fs::read(&offer.checkpoint).unwrap();
    assert!(
        Recovery::read_checkpoint(&offer.checkpoint)
            .unwrap()
            .same_semantics(&doc)
    );
    (offer, doc, payload)
}
fn protect_confirmed_recovery(app: App, offer: &Found, doc: &paintcraft_document::Document) {
    let protected = Folder::new();
    // No existing checkpoint owner: this public startup attachment is permitted.
    // Handoff remains retained while the new owner has not written a checkpoint.
    let mut app = app.with_recovery(protected.0.clone());
    assert!(offer.checkpoint.exists());
    assert!(app.recovery_source.is_some());
    assert!(!app.editor.has_checkpoint());
    assert!(app.editor.autosave());
    assert!(app.editor.has_checkpoint());
    app.protect_recovery_source();
    app.pump_recovery_handoff();
    assert!(app.recovery_source.is_none());
    assert!(!offer.checkpoint.exists());
    drop(app);
    let copies = Recovery::scan(&protected.0);
    assert_eq!(copies.len(), 1);
    assert!(
        Recovery::read_checkpoint(&copies[0].checkpoint)
            .unwrap()
            .same_semantics(doc)
    );
}
#[test]
fn classic_blend_recovery_explicit_discard_keeps_private_draft_until_acceptance() {
    use paintcraft_core::types::LayerBlendMode;
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        let folder = Folder::new();
        let (offer, doc, payload) = blend_orphan(&folder, mode);
        let mut app = App::new(None);
        app.settings.experience = Experience::Classic;
        app.recoverable.push(offer.clone());
        app.editor.tool = Tool::Text;
        app.editor.pointer_down((3, 2));
        app.editor.type_text("private until explicit discard");
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let stamp = app.editor.preview_stamp();
        let text = app.editor.canvas_text_content().unwrap().to_owned();
        for _ in 0..2 {
            app.recover_requested(offer.clone());
            assert!(
                matches!(&app.confirm, Some(Confirm::DiscardForRecovery(found)) if found == &offer)
            );
            assert!(std::sync::Arc::ptr_eq(
                &before,
                &app.editor.document_snapshot()
            ));
            assert_eq!(app.editor.history_view().ticket, ticket);
            assert_eq!(app.editor.preview_stamp(), stamp);
            assert_eq!(app.editor.canvas_text_content(), Some(text.as_str()));
            assert_eq!(fs::read(&offer.checkpoint).unwrap(), payload);
            app.confirm = None; // Explicit Cancel on the first decision preserves all state.
        }
        app.recover_requested(offer.clone());
        let Some(Confirm::DiscardForRecovery(accepted)) = app.confirm.take() else {
            panic!("explicit Classic discard decision")
        };
        let owner = app.active_tab_id();
        app.do_recover(accepted);
        assert_eq!(app.active_tab_id(), owner);
        assert!(app.editor.document().same_semantics(&doc));
        assert!(!app.editor.is_editing_text());
        assert!(app.editor.is_dirty());
        assert_eq!(app.editor.undo_depth(), 0);
        assert!(
            !app.recoverable
                .iter()
                .any(|item| item.session == offer.session)
        );
        assert!(app.recovery_source.is_some());
        assert_eq!(fs::read(&offer.checkpoint).unwrap(), payload);
        assert!(Recovery::discard(&folder.0, offer.session).is_err());
        protect_confirmed_recovery(app, &offer, &doc);
    }
}
#[test]
fn studio_blend_recovery_opens_new_owner_and_preserves_prior_input_source_and_history() {
    use paintcraft_core::types::{LayerBlendMode, Rgba};
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        let folder = Folder::new();
        let (offer, doc, payload) = blend_orphan(&folder, mode);
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.recoverable.push(offer.clone());
        app.editor.request(0x62af, Action::AddLayer);
        app.editor.run_pending();
        app.editor.select_tool(Tool::ColorPicker);
        app.editor.color = Rgba([11, 37, 191, 255]);
        app.editor.secondary = Rgba([201, 43, 7, 255]);
        let input = (app.editor.tool, app.editor.color, app.editor.secondary);
        let old_owner = app.active_tab_id();
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let tabs = app.tabs.len();
        app.recover_requested(offer.clone());
        assert!(
            app.confirm.is_none(),
            "Studio opens a new tab instead of Classic discard"
        );
        let recovered_owner = app.active_tab_id();
        assert_ne!(recovered_owner, old_owner);
        assert_eq!(app.tabs.len(), tabs + 1);
        assert!(app.editor.document().same_semantics(&doc));
        assert!(app.editor.is_dirty());
        assert_eq!(app.editor.undo_depth(), 0);
        assert!(app.recovery_source.is_some());
        assert_eq!(fs::read(&offer.checkpoint).unwrap(), payload);
        assert!(Recovery::discard(&folder.0, offer.session).is_err());
        assert!(app.swap_tab_state(old_owner));
        assert!(std::sync::Arc::ptr_eq(
            &before,
            &app.editor.document_snapshot()
        ));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert_eq!(app.editor.undo_depth(), 1);
        assert_eq!(
            (app.editor.tool, app.editor.color, app.editor.secondary),
            input
        );
        assert!(app.swap_tab_state(recovered_owner));
        assert!(app.editor.document().same_semantics(&doc));
        assert!(app.recovery_source.is_some());
        protect_confirmed_recovery(app, &offer, &doc);
    }
}
