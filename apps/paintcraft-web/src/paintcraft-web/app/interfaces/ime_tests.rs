use super::*;
#[test]
fn parked_frontend_never_resurrects_a_previous_composition_or_accepts_its_late_result() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let original = app.frontend;
    let other = if original == Frontend::Studio {
        Frontend::Classic
    } else {
        Frontend::Studio
    };
    app.test_start_preedit("old candidate");
    app.editor.tools.type_text("Base");
    // Start a valid lifetime after the base revision changed.
    app.text.reset_composition();
    app.test_start_preedit("old candidate");
    let owner = app.editor.session.owner_id();
    let ticket = app.editor.session.ticket();
    app.switch_frontend(&ctx, other);
    assert!(!app.text.is_composing());
    app.controls.tool = paintcraft_tools::Tool::Text;
    app.editor.tools.choose_tool(paintcraft_tools::Tool::Text);
    app.sync_text();
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (0, 0));
    app.editor.tools.type_text("Incoming");
    app.text_event(
        &ctx,
        egui::Event::Ime(egui::ImeEvent::Commit("late from old frontend".into())),
    );
    assert_eq!(app.editor.tools.text_content(), Some("Incoming"));

    app.switch_frontend(&ctx, original);
    assert!(!app.text.is_composing());
    assert_eq!(app.editor.session.owner_id(), owner);
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.tools.text_content(), Some("Base"));
    app.text_event(
        &ctx,
        egui::Event::Ime(egui::ImeEvent::Commit("late".into())),
    );
    assert_eq!(app.editor.tools.text_content(), Some("Base"));
    app.test_start_preedit("fresh");
    app.text_event(
        &ctx,
        egui::Event::Ime(egui::ImeEvent::Commit("fresh".into())),
    );
    assert_eq!(app.editor.tools.text_content(), Some("Basefresh"));
    assert_eq!(app.editor.session.undo_depth(), 0);
}
