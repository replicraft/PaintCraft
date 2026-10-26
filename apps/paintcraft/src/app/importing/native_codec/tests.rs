use super::*;
use std::time::Instant;
pub(super) const INPUT: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl"
);
fn folder() -> PathBuf {
    let root = std::env::temp_dir().join(format!("paintcraft-jxl-app-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    root
}
pub(super) fn app() -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.initialize_editing(app.context.clone());
    assert!(app.editing.is_some());
    app
}
pub(super) fn retire(app: &App) {
    let end = Instant::now() + Duration::from_secs(15);
    while app.worker_budget.usage().reserved != 0 {
        app.editing.as_ref().unwrap().reap_abandoned();
        assert!(Instant::now() < end, "source job did not retire");
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn draw(app: &mut App) {
    let ctx = app.context.clone();
    let mut output = ctx.run_ui(Default::default(), |ui| app.native_codec_dialog(ui.ctx()));
    output.textures_delta.clear();
}
#[test]
fn native_jxl_profiles_drafts_input_extent_and_admission_fail_before_any_publication() {
    let root = folder();
    let path = root.join("source.jxl");
    std::fs::write(&path, INPUT).unwrap();
    let mut app = app();
    let before = app.editor.document_snapshot();
    app.settings.experience = Experience::Classic;
    assert!(app.stage_native_jxl(path.clone()).is_err());
    assert!(app.open_path(&path).is_err());
    app.settings.experience = Experience::Studio;
    assert!(app.open_path(&path).is_err());
    app.editor.begin_text(Uuid::nil(), (0, 0));
    assert_eq!(
        app.stage_native_jxl(root.join("absent.jxl"))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    app.editor.cancel_text();
    let lease = app
        .worker_budget
        .reserve(app.worker_budget.usage().limit)
        .unwrap();
    assert_eq!(
        app.stage_native_jxl_with_launch(
            path.clone(),
            Box::new(|_, _| panic!("unadmitted import must never launch"))
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert!(app.pending_import.is_none());
    drop(lease);
    let oversized = root.join("large.jxl");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(wire::INPUT_BYTES_MAX as u64 + 1)
        .unwrap();
    assert_eq!(
        app.stage_native_jxl(oversized).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(app.pending_import.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.worker_budget.usage().reserved, 0);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn native_jxl_pending_owner_changes_and_document_closing_cancel_without_adoption() {
    let root = folder();
    let path = root.join("source.jxl");
    std::fs::write(&path, INPUT).unwrap();
    for change in 0..5 {
        let mut app = app();
        app.stage_native_jxl(path.clone()).unwrap();
        assert!(app.pending_import.is_some());
        let ctx = app.context.clone();
        match change {
            0 => {
                app.editor = Editor::with_document(app.editor.document().clone(), None);
            }
            1 => app.settings.experience = Experience::Classic,
            2 => app
                .add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
                .unwrap(),
            3 => {
                app.editor.select_tool(Tool::Text);
                app.editor.begin_text(Uuid::nil(), (0, 0));
            }
            _ => {
                app.pending_import = None;
                app.confirm = None;
            }
        }
        let current = app.editor.document_snapshot();
        if change != 4 {
            draw(&mut app);
        }
        assert!(app.pending_import.is_none());
        assert!(Arc::ptr_eq(&current, &app.editor.document_snapshot()));
        retire(&app);
        assert!(app.auxiliary_sources.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}
