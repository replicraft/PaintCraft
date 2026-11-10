use super::*;

#[test]
fn third_key_preserves_all_three_real_sources_and_views() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut sources = Vec::new();
    for (i, frontend) in [Frontend::Classic, Frontend::Studio, Frontend::Retro]
        .into_iter()
        .enumerate()
    {
        assert!(app.switch_frontend(&ctx, frontend));
        app.name = format!("owner {i}");
        app.zoom = i as f32 + 1.5;
        app.controls.options.pencil_size = i as u32 + 3;
        app.text.search = format!("font {i}");
        app.edit(Edit::ClearCanvas {
            color: paintcraft_core::types::Rgba([i as u8 + 7, 8, 9, 255]),
        });
        sources.push((
            frontend,
            app.editor.session.history_ticket(),
            app.editor.session.snapshot(),
        ));
    }
    assert_ne!(sources[0].1.owner, sources[1].1.owner);
    assert_ne!(sources[1].1.owner, sources[2].1.owner);
    assert_ne!(sources[0].1.owner, sources[2].1.owner);
    for (i, (frontend, ticket, source)) in sources.into_iter().enumerate() {
        assert!(app.switch_frontend(&ctx, frontend));
        assert_eq!(app.editor.session.history_ticket(), ticket);
        assert!(std::sync::Arc::ptr_eq(
            &app.editor.session.snapshot(),
            &source
        ));
        assert_eq!(app.name, format!("owner {i}"));
        assert_eq!(app.zoom, i as f32 + 1.5);
        assert_eq!(app.controls.options.pencil_size, i as u32 + 3);
        assert_eq!(app.text.search, format!("font {i}"));
        assert_eq!(app.preferences.frontend, frontend);
    }
}

#[test]
fn failed_retro_creation_keeps_both_owners_and_preference() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let classic = app.editor.session.history_ticket();
    assert!(app.switch_frontend(&ctx, Frontend::Studio));
    let studio = app.editor.session.history_ticket();
    let source = app.editor.session.snapshot();
    app.preferences.canvas.configured = (4096, 4096);
    assert!(!app.switch_frontend(&ctx, Frontend::Retro));
    assert_eq!(app.frontend, Frontend::Studio);
    assert_eq!(app.preferences.frontend, Frontend::Studio);
    assert_eq!(app.editor.session.history_ticket(), studio);
    assert!(std::sync::Arc::ptr_eq(
        &source,
        &app.editor.session.snapshot()
    ));
    assert_eq!(
        app.parked
            .get(Frontend::Classic)
            .unwrap()
            .editor
            .session
            .history_ticket(),
        classic
    );
    assert!(app.parked.get(Frontend::Retro).is_none());
}

#[test]
fn retro_dirty_download_cancel_and_discard_keep_destination_owner() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 850.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let ctx = h.ctx.clone();
    h.state_mut().switch_frontend(&ctx, Frontend::Studio);
    let studio = h.state().editor.session.history_ticket();
    h.state_mut().switch_frontend(&ctx, Frontend::Retro);
    h.state_mut().edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([8, 9, 10, 255]),
    });
    let retro = h.state().editor.session.history_ticket();
    h.state_mut()
        .action(&ctx, Action::ChooseFrontend(Frontend::Studio));
    h.state_mut().download(&ctx, false);
    h.state_mut()
        .accept_download_name(retro.owner, Some("retro.pcnd".into()));
    assert!(h.state().unsaved());
    assert_eq!(h.state().frontend, Frontend::Retro);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.session.history_ticket(), retro);
    h.state_mut()
        .action(&ctx, Action::ChooseFrontend(Frontend::Studio));
    h.run();
    h.get_by_label("Discard and continue").click();
    h.run();
    assert_eq!(h.state().editor.session.history_ticket(), studio);
    h.state_mut().switch_frontend(&ctx, Frontend::Retro);
    assert_ne!(h.state().editor.session.history_ticket().owner, retro.owner);
    assert!(!h.state().unsaved());
}

#[test]
fn actual_interface_menu_enters_retro_chrome_and_can_leave_it() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut h = Harness::builder()
        .with_size(egui::vec2(1300.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.get_by_label("Interface").click();
    h.run();
    h.get_by_label("Retro").click();
    h.run();
    assert_eq!(h.state().frontend, Frontend::Retro);
    assert!(h.query_by_label("Win95-inspired").is_some());
    assert!(h.query_by_label("Colors").is_some());
    assert!(h.query_by_label("Help").is_some());
    assert!(h.state().canvas.controls.is_none());
    h.get_by_label("Interface").click();
    h.run();
    h.get_by_label("Studio").click();
    h.run();
    assert_eq!(h.state().frontend, Frontend::Studio);
    assert!(h.query_by_label("Window").is_some());
    assert!(h.query_by_label("Win95-inspired").is_none());
}
