//! One browser attachment realm survives pending loads, snapshots and interfaces.
use super::*;
use crate::{WebApp, importing::Owner};
use egui_kittest::{Harness, kittest::Queryable};

fn source() -> Vec<u8> {
    include_bytes!("../../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd").to_vec()
}
fn load(files: &Files) -> Result<Loaded> {
    decode_input_with_metadata(
        &source(),
        "source.pcnd".into(),
        Frontend::Studio,
        false,
        &files.metadata_budget(),
        &|| Ok(()),
    )
}
fn charge() -> u64 {
    let files = Files::default();
    let loaded = load(&files).unwrap();
    let bytes = loaded
        .document
        .source_metadata
        .as_ref()
        .unwrap()
        .storage_usage()
        .1;
    drop(loaded);
    assert_eq!(files.metadata_budget().usage().reserved, 0);
    bytes
}

#[test]
fn shared_browser_metadata_admission_bounds_pending_and_retained_snapshots_and_releases_on_drop() {
    let bytes = charge();
    let budget = SharedBudget::new(bytes * 2 - 1);
    let files = Files::with_metadata_budget(budget.clone());
    let first = load(&files).unwrap();
    assert_eq!(budget.usage().reserved, bytes);
    let retained = first.document.clone();
    drop(first);
    assert_eq!(load(&files).unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().reserved, bytes);
    drop(retained);
    assert_eq!(budget.usage().reserved, 0);
    let replacement = load(&files).unwrap();
    assert_eq!(budget.usage().reserved, bytes);
    drop(replacement);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn real_interface_switch_keeps_the_same_metadata_realm_and_parked_profile_charge() {
    let bytes = charge();
    let budget = SharedBudget::new(bytes * 2 - 1);
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.state_mut().files = Files::with_metadata_budget(budget.clone());
    let loaded = load(&h.state().files).unwrap();
    let owner = Owner::capture(h.state());
    h.state_mut().import_owner = Some(owner);
    let ctx = h.ctx.clone();
    h.state_mut().accept_loaded(loaded, &ctx);
    assert!(
        h.state()
            .editor
            .session
            .document()
            .source_metadata
            .is_some()
    );
    h.run();
    h.get_by_label("Interface").click();
    h.run();
    h.get_by_label("Studio").click();
    h.run();
    assert_eq!(h.state().frontend, paintcraft_ui::actions::Frontend::Studio);
    assert!(
        h.state()
            .editor
            .session
            .document()
            .source_metadata
            .is_none()
    );
    assert_eq!(budget.usage().reserved, bytes);
    assert_eq!(
        load(&h.state().files).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    h.get_by_label("Interface").click();
    h.run();
    h.get_by_label("Classic").click();
    h.run();
    assert!(
        h.state()
            .editor
            .session
            .document()
            .source_metadata
            .is_some()
    );
    assert_eq!(budget.usage().reserved, bytes);
    drop(ctx);
    drop(h);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn stale_browser_owner_drops_the_charged_candidate_without_replacing_current_work() {
    let budget = SharedBudget::new(charge());
    let ctx = eframe::egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.files = Files::with_metadata_budget(budget.clone());
    app.import_owner = Some(Owner::capture(&app));
    let loaded = load(&app.files).unwrap();
    app.editor = crate::editor::Editor::new(8, 8).unwrap();
    let before = app.editor.session.snapshot();
    app.accept_loaded(loaded, &ctx);
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &app.editor.session.snapshot()
    ));
    assert!(app.editor.session.document().source_metadata.is_none());
    assert_eq!(budget.usage().reserved, 0);
}
