use super::*;
use paintcraft_core::types::resources::SharedBudget;
use std::fs;
fn png() -> &'static [u8] {
    include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/png-retention-fixtures/rgba8.png"
    )
}
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/png-source-retention/native")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn png_pending_adoption_and_pcnd_reopen_share_host_metadata_realm_and_stay_unbound_unsaved() {
    let folder = Folder::new();
    let path = folder.0.join("original.png");
    fs::write(&path, png()).unwrap();
    let mut app = App::new(None);
    let budget = SharedBudget::new(1 << 20);
    app.source_metadata_budget = budget.clone();
    app.open_path(&path).unwrap();
    let pending = app.pending_import.as_ref().unwrap();
    let metadata = pending.source_metadata.as_ref().unwrap().clone();
    assert!(budget.usage().reserved > 0);
    assert!(app.editor.document().source_metadata.is_none());
    let ctx = super::raster_review::tests::authorize(&mut app);
    app.resolve_import(&ctx, true);
    assert!(app.pending_import.is_none());
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert_eq!(
        app.editor.document().source_metadata.as_ref().unwrap(),
        &metadata
    );
    assert_eq!(app.imported_source.as_ref(), Some(&path));
    let project = paintcraft_formats::pcnd::write_compact(app.editor.document()).unwrap();
    let reopened = app.read_project_with_metadata(&project).unwrap();
    assert_eq!(reopened.source_metadata.as_ref().unwrap(), &metadata);
    let charge = metadata.storage_usage().1;
    assert_eq!(budget.usage().reserved, charge * 2);
    drop(reopened);
    drop(metadata);
    drop(app);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(fs::read(&path).unwrap(), png());
}
#[test]
fn canceled_stale_changed_source_and_exhausted_realm_never_adopt_or_leak_metadata() {
    for mode in 0..4 {
        let folder = Folder::new();
        let path = folder.0.join("original.png");
        fs::write(&path, png()).unwrap();
        let mut app = App::new(None);
        let budget = SharedBudget::new(if mode == 3 { 0 } else { 1 << 20 });
        app.source_metadata_budget = budget.clone();
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let result = app.open_path(&path);
        if mode == 3 {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        } else {
            result.unwrap();
            let ctx = super::raster_review::tests::authorize(&mut app);
            if mode == 1 {
                app.settings.classic_variant = Some(ClassicVariant::V7);
            }
            if mode == 2 {
                fs::write(&path, b"changed source").unwrap();
            }
            app.resolve_import(&ctx, mode != 0);
        }
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.history_view().ticket);
        assert!(app.pending_import.is_none());
        assert_eq!(budget.usage().reserved, 0);
        assert!(app.bound.is_none());
    }
}
