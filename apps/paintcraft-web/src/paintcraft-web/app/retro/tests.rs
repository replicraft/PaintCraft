use super::*;

#[test]
fn saved_retro_choice_starts_fresh_real_owner_and_keeps_other_theme() {
    let ctx = egui::Context::default();
    let prefs = crate::preferences::Preferences {
        frontend: Frontend::Retro,
        theme: Some(egui::Theme::Dark),
        ..Default::default()
    };
    let mut app = WebApp::from_preferences(&ctx, prefs.clone(), None);
    assert_eq!(app.frontend, Frontend::Retro);
    assert_eq!(app.preferences.theme, Some(egui::Theme::Dark));
    assert_eq!(
        app.format_profile(),
        paintcraft_formats::registry::Frontend::RetroWin95
    );
    let first = app.editor.session.history_ticket();
    app.edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([12, 13, 14, 255]),
    });
    let fresh = WebApp::from_preferences(&ctx, prefs, None);
    assert_ne!(first.owner, fresh.editor.session.history_ticket().owner);
    assert!(!fresh.unsaved());
    assert_eq!(fresh.editor.session.undo_depth(), 0);
}

#[test]
fn all_sixteen_choices_reach_actual_tools_and_refused_composition_is_atomic() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    assert!(app.switch_frontend(&ctx, Frontend::Retro));
    for choice in paintcraft_retro::ToolChoice::ALL {
        assert!(app.choose_retro_tool(&ctx, choice), "{choice:?}");
        assert_eq!(app.editor.tools.tool, choice.tool());
        assert!(choice.matches(&app.controls));
        assert_eq!(app.editor.tools.options, app.controls.options);
    }
    app.test_start_preedit("unfinished");
    let controls = app.controls;
    assert!(!app.choose_retro_tool(&ctx, paintcraft_retro::ToolChoice::Airbrush));
    assert_eq!(app.controls.tool, controls.tool);
    assert_eq!(app.controls.options, controls.options);
    assert!(app.text.is_composing());
}

#[test]
fn retro_eraser_picker_status_and_about_use_real_host_state() {
    use paintcraft_core::types::{
        Rgba,
        picker::{PickerAfter, SampleSize, SampleSource},
    };
    use paintcraft_ui::actions::ViewAction;
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.switch_frontend(&ctx, Frontend::Retro);
    app.controls.tool = Tool::ColorPicker;
    app.configure_paint_style_tools();
    assert_eq!(app.editor.tools.options.picker_source, SampleSource::Image);
    assert_eq!(app.editor.tools.options.picker_size, SampleSize::One);
    assert_eq!(app.editor.tools.options.picker_after, PickerAfter::Previous);
    assert_eq!(app.editor.tools.options.eraser_color, Rgba([255; 4]));
    assert!(app.status_bar_visible());
    app.action(&ctx, Action::View(ViewAction::StatusBar));
    assert!(!app.status_bar_visible());
    assert!(app.preferences.status_bar);
    app.retro_intents(&ctx, vec![paintcraft_retro::Intent::About]);
    assert!(app.settings);
}

#[test]
fn retro_export_choices_and_project_reopen_use_the_captured_real_profile() {
    use paintcraft_ui::actions::ImageExport;
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.switch_frontend(&ctx, Frontend::Retro);
    let formats = app.enabled_export_formats();
    assert_eq!(formats.len(), 2);
    assert!(formats.contains(&ImageExport::Png) && formats.contains(&ImageExport::Bmp));
    app.edit(Edit::ClearCanvas {
        color: paintcraft_core::types::Rgba([9, 17, 33, 255]),
    });
    let source = app.editor.session.snapshot();
    let bytes = paintcraft_formats::pcnd::write(&source).unwrap();
    let loaded =
        crate::files::decode_for_profile(&bytes, "retro.pcnd".into(), app.format_profile())
            .unwrap();
    assert!(source.same_semantics(&loaded.document));
    for (name, allowed) in [
        ("pixel.png", true),
        ("pixel.bmp", false),
        ("pixel.webp", false),
    ] {
        let png = paintcraft_formats::image::encode(
            paintcraft_formats::image::ImageFormat::Png,
            1,
            1,
            &[9, 17, 33, 255],
            90,
        )
        .unwrap();
        assert_eq!(
            crate::files::decode_for_profile(&png, name.into(), app.format_profile()).is_ok(),
            allowed
        );
    }
}

#[test]
fn accepted_retro_pencil_fill_and_text_expose_named_live_option_controls() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut h = Harness::builder()
        .with_size(egui::vec2(1300.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    let ctx = h.ctx.clone();
    h.state_mut().switch_frontend(&ctx, Frontend::Retro);
    h.run();
    let source = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.history_ticket();
    h.get_by_role_and_label(egui::Role::Button, "Pencil")
        .click();
    h.run();
    assert_eq!(h.state().editor.tools.tool, Tool::Pencil);
    h.get_by_role_and_label(egui::Role::SpinButton, "Size")
        .set_value_accesskit(7.0);
    h.run();
    assert_eq!(h.state().controls.options.pencil_size, 7);
    h.get_by_role_and_label(egui::Role::Button, "Fill").click();
    h.run();
    assert_eq!(h.state().editor.tools.tool, Tool::Fill);
    h.get_by_role_and_label(egui::Role::SpinButton, "Tolerance")
        .set_value_accesskit(23.0);
    h.run();
    assert_eq!(h.state().controls.options.tolerance, 23);
    h.get_by_role_and_label(egui::Role::Button, "Text").click();
    h.run();
    assert_eq!(h.state().editor.tools.tool, Tool::Text);
    assert!(
        h.query_by_role_and_label(egui::Role::ComboBox, "Default font")
            .is_some()
    );
    h.get_by_role_and_label(egui::Role::SpinButton, "Font size")
        .set_value_accesskit(19.0);
    h.run();
    assert_eq!(h.state().text.style.points, 19);
    assert_eq!(h.state().editor.session.history_ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(
        &source,
        &h.state().editor.session.snapshot()
    ));
}

#[test]
fn retro_startup_new_document_and_missing_owner_respect_remember_last() {
    for (remember_last, expected) in [(true, (48, 36)), (false, (32, 24))] {
        let ctx = egui::Context::default();
        let mut prefs = crate::preferences::Preferences {
            frontend: Frontend::Retro,
            ..Default::default()
        };
        prefs.canvas.configured = (32, 24);
        prefs.canvas.last = Some((48, 36));
        prefs.canvas.remember_last = remember_last;
        let mut startup = WebApp::from_preferences(&ctx, prefs.clone(), None);
        assert_eq!(startup.editor.session.document().canvas, expected);
        let old = startup.editor.session.snapshot();
        let owner = startup.editor.session.history_ticket().owner;
        startup.new_document();
        assert_eq!(startup.editor.session.document().canvas, expected);
        assert_ne!(startup.editor.session.history_ticket().owner, owner);
        assert_eq!(old.canvas, expected);
        prefs.frontend = Frontend::Studio;
        let mut from_studio = WebApp::from_preferences(&ctx, prefs, None);
        let studio = from_studio.editor.session.history_ticket();
        assert_eq!(from_studio.editor.session.document().canvas, (800, 600));
        assert!(from_studio.switch_frontend(&ctx, Frontend::Retro));
        assert_eq!(from_studio.editor.session.document().canvas, expected);
        assert_ne!(
            from_studio.editor.session.history_ticket().owner,
            studio.owner
        );
        assert!(from_studio.switch_frontend(&ctx, Frontend::Studio));
        assert_eq!(from_studio.editor.session.history_ticket(), studio);
    }
}

#[test]
fn retro_effective_light_theme_restores_the_unchanged_preferred_theme() {
    let ctx = egui::Context::default();
    let prefs = crate::preferences::Preferences {
        frontend: Frontend::Retro,
        theme: Some(egui::Theme::Dark),
        ..Default::default()
    };
    let mut app = WebApp::from_preferences(&ctx, prefs, None);
    assert_eq!(ctx.theme(), egui::Theme::Light);
    assert!(app.switch_frontend(&ctx, Frontend::Studio));
    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert!(app.switch_frontend(&ctx, Frontend::Retro));
    assert_eq!(ctx.theme(), egui::Theme::Light);
    assert!(app.switch_frontend(&ctx, Frontend::Classic));
    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert_eq!(app.preferences.theme, Some(egui::Theme::Dark));
}

#[test]
fn actual_retro_host_keeps_narrow_tools_and_reachable_mobile_navigation() {
    use egui_kittest::{Harness, kittest::Queryable};
    for width in [1440.0, 320.0] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(width, 700.0))
            .build_eframe(|cc| {
                WebApp::from_preferences(
                    &cc.egui_ctx,
                    crate::preferences::Preferences {
                        frontend: Frontend::Retro,
                        theme: Some(egui::Theme::Dark),
                        ..Default::default()
                    },
                    None,
                )
            });
        h.run();
        assert_eq!(h.ctx.theme(), egui::Theme::Light);
        let first = h
            .get_by_role_and_label(egui::Role::Button, "Free-form")
            .rect();
        let second = h.get_by_role_and_label(egui::Role::Button, "Select").rect();
        assert!((second.min.x - first.min.x - 25.0).abs() < 1.0);
        assert!(second.max.x <= 64.0, "{second:?}");
        let settings = h
            .get_by_role_and_label(egui::Role::Button, "Settings…")
            .rect();
        let changer = h
            .get_by_role_and_label(egui::Role::Button, "Interface")
            .rect();
        assert!(
            h.ctx.content_rect().contains_rect(settings)
                && h.ctx.content_rect().contains_rect(changer)
        );
        let file = h.get_by_label("File").rect();
        if width < 520.0 {
            assert!(settings.max.y <= file.min.y + 2.0);
        } else {
            assert!((settings.center().y - file.center().y).abs() < 2.0);
        }
        let white = h
            .get_by_role_and_label(egui::Role::ColorWell, "Palette color #FFFFFF")
            .rect();
        assert!(h.ctx.content_rect().contains_rect(white));
        h.get_by_role_and_label(egui::Role::Button, "Settings…")
            .click();
        h.run();
        assert!(h.state().settings);
        assert_eq!(h.ctx.theme(), egui::Theme::Light);
        h.get_by_label("Cancel").click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, "Interface")
            .click();
        h.run();
        h.get_by_label("Studio").click();
        h.run();
        assert_eq!(h.state().frontend, Frontend::Studio);
        assert_eq!(h.ctx.theme(), egui::Theme::Dark);
        assert_eq!(h.state().preferences.theme, Some(egui::Theme::Dark));
    }
}
