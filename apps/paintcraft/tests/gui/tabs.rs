use super::studio::studio_harness;
use super::*;

#[test]
fn studio_new_preserves_other_documents_and_each_history() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    drag(&mut h, (180.0, 10.0), (240.0, 10.0));
    let a = h.state().editor().document_snapshot();
    let a_ticket = h.state().editor().history_view().ticket;
    h.get_by_label("New").click();
    h.run();
    assert_eq!(h.state().document_count(), 2);
    assert!(!h.state().editor().is_dirty());
    drag(&mut h, (180.0, 30.0), (240.0, 30.0));
    let b = h.state().editor().document_snapshot();
    h.get_by_label("Document 1: Untitled 1 *").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&a));
    assert_eq!(h.state().editor().history_view().ticket, a_ticket);
    h.get_by_label("Undo").click();
    h.run();
    h.get_by_label("Document 2: Untitled 2 *").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&b));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancelling_or_discarding_a_tab_close_never_discards_another_document() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    let a = h.state().editor().document_snapshot();
    h.get_by_label("New").click();
    h.run();
    let b = h.state().editor().document_snapshot();
    h.get_by_label("Close document 1").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().document_count(), 2);
    assert!(h.state().editor().document().same_semantics(&a));
    h.get_by_label("Close document 1").click();
    h.run();
    h.get_by_label("Don't save").click();
    h.run();
    assert_eq!(h.state().document_count(), 1);
    assert!(h.state().editor().document().same_semantics(&b));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_and_close_writes_the_captured_document_then_leaves_an_empty_workspace() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    drag(&mut h, (180.0, 10.0), (240.0, 10.0));
    let snapshot = h.state().editor().document_snapshot();
    let path = dir.join("saved.pcnd");
    h.state_mut().set_path_text(&path.to_string_lossy());
    h.get_by_label("Close document 1").click();
    h.run();
    h.get_by_label("Save and close").click();
    h.run();
    assert_eq!(h.state().document_count(), 0);
    let saved = paintcraft_formats::pcnd::read(&std::fs::read(&path).unwrap()).unwrap();
    assert!(saved.same_semantics(&snapshot));
    h.get_by_label("New").click();
    h.run();
    assert_eq!(h.state().document_count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_save_and_close_keeps_the_document_open() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    let snapshot = h.state().editor().document_snapshot();
    let path = dir.join("missing/cannot-save.pcnd");
    h.state_mut().set_path_text(&path.to_string_lossy());
    h.get_by_label("Close document 1").click();
    h.run();
    h.get_by_label("Save and close").click();
    h.run();
    assert_eq!(h.state().document_count(), 1);
    assert!(h.state().editor().is_dirty());
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert!(!path.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn studio_documents_remain_in_their_own_workspace_after_switching() {
    let (mut h, dir) = studio_harness(true);
    let mut snapshots = Vec::new();
    for i in 0..5 {
        if i > 0 {
            h.get_by_label("New").click();
            h.run();
        }
        drag(
            &mut h,
            (180.0, 10.0 + 10.0 * i as f32),
            (240.0, 10.0 + 10.0 * i as f32),
        );
        snapshots.push(h.state().editor().document_snapshot());
        h.state_mut()
            .set_path_text(dir.join(format!("studio-{i}.pcnd")).to_str().unwrap());
        h.get_by_label("Save").click();
        h.run();
    }
    switch_interface(&mut h, "Classic interface");
    h.run();
    h.get_by_label("Continue").click();
    h.run();
    assert_eq!(
        h.state().document_count(),
        1,
        "Classic starts with its own canvas"
    );
    assert_ne!(
        h.state().editor().document().document_id,
        snapshots[0].document_id
    );
    switch_interface(&mut h, "Studio interface");
    h.run();
    assert_eq!(h.state().document_count(), 5);
    for (i, snapshot) in snapshots.iter().enumerate() {
        h.get_by_label(&format!("Document {}: studio-{i}.pcnd", i + 1))
            .click();
        h.run();
        assert!(h.state().editor().document().same_semantics(snapshot));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_document_tabs_for_review() {
    let (mut h, dir) = studio_harness(false);
    for i in 0..3 {
        if i > 0 {
            h.get_by_label("New").click();
            h.run();
        }
        drag(
            &mut h,
            (180.0, 20.0 + 20.0 * i as f32),
            (240.0, 50.0 + 20.0 * i as f32),
        );
    }
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-studio-tabs.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
