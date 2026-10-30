use super::*;
use crate::editor::Editor;
use paintcraft_core::types::{Rgba, filters::Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
fn app(ctx: &egui::Context) -> WebApp {
    let mut app = WebApp::new(ctx);
    let mut doc = Editor::new(192, 96).unwrap().session.document().clone();
    let layer = doc.roots[0];
    doc.layers.get_mut(&layer).unwrap().opacity = 167;
    doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(10),
        true,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    let mut top = doc.layers[&layer].clone();
    top.id = Uuid::from_u128(11);
    top.filters = Default::default();
    top.opacity = 125;
    top.raster = paintcraft_raster::RasterLayer::new(192, 96, Rgba([80, 30, 110, 170]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    app.editor = Editor::from_document(doc).unwrap();
    app.editor.target = Some(layer);
    app.controls.tool = Tool::Text;
    app.editor.tools.choose_tool(Tool::Text);
    app.text.style.points = 22;
    app.controls.primary = Rgba([30, 100, 200, 180]);
    app.sync_text();
    app.editor.tools.begin_text(layer, (7, 7));
    app.editor.tools.type_text("Base ");
    app
}
fn preedit(text: &str) -> egui::Event {
    egui::Event::Ime(egui::ImeEvent::Preedit {
        text: text.into(),
        active_range_chars: None,
    })
}
fn commit(text: &str) -> egui::Event {
    egui::Event::Ime(egui::ImeEvent::Commit(text.into()))
}
fn pixels(app: &WebApp) -> Vec<u8> {
    paintcraft_render::render_flat(&app.canvas_preview_document().unwrap().unwrap()).unwrap()
}
#[test]
fn filtered_preedit_matches_real_commit_and_preserves_source_history_and_draft() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    let source = paintcraft_formats::pcnd::write(app.editor.session.document()).unwrap();
    let ticket = app.editor.session.ticket();
    let stamp = app.editor.tools.text_stamp();
    app.text_event(&ctx, preedit("日本 café"));
    let provisional = pixels(&app);
    assert_eq!(app.editor.tools.text_content(), Some("Base "));
    assert_eq!(app.editor.tools.text_stamp(), stamp);
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert_eq!(
        paintcraft_formats::pcnd::write(app.editor.session.document()).unwrap(),
        source
    );
    assert_ne!(
        provisional,
        paintcraft_render::render_flat(app.editor.session.document()).unwrap()
    );
    app.text_event(&ctx, commit("日本 café"));
    assert!(!app.text.is_composing());
    assert_eq!(app.editor.tools.text_content(), Some("Base 日本 café"));
    assert_eq!(pixels(&app), provisional);
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    app.editor.place_text().unwrap();
    assert_eq!(app.editor.session.undo_depth(), 1);
    assert_eq!(
        paintcraft_render::render_flat(app.editor.session.document()).unwrap(),
        provisional
    );
}
#[test]
#[allow(deprecated)] // Explicit compatibility event, not a claim egui emits it.
fn select_all_replacement_empty_updates_disabled_and_escape_are_private() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    app.text.selected_all = true;
    app.text_event(&ctx, preedit("置換"));
    let expected = pixels(&app);
    assert_eq!(app.editor.tools.text_content(), Some("Base "));
    app.text_event(&ctx, commit("置換"));
    assert_eq!(app.editor.tools.text_content(), Some("置換"));
    assert_eq!(pixels(&app), expected);
    app.text_event(&ctx, preedit("候補"));
    app.text_event(&ctx, preedit(""));
    assert!(app.text.is_composing());
    assert!(app.text.preedit().is_empty());
    assert!(!app.finish_text());
    app.text_event(&ctx, commit("確定"));
    assert_eq!(app.editor.tools.text_content(), Some("置換確定"));
    app.text_event(&ctx, preedit("discard"));
    app.text_event(&ctx, egui::Event::Ime(egui::ImeEvent::Disabled));
    assert!(!app.text.is_composing());
    app.text_event(&ctx, commit("late"));
    assert_eq!(app.editor.tools.text_content(), Some("置換確定"));
    app.text_event(&ctx, preedit("new"));
    app.text_event(
        &ctx,
        egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    );
    assert!(!app.text.is_composing());
    assert_eq!(app.editor.tools.text_content(), Some("置換確定"));
    assert_eq!(app.editor.session.undo_depth(), 0);
}
#[test]
fn stale_session_source_tool_target_frontend_and_exact_stamp_reject_late_result() {
    for case in 0..7 {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.text_event(&ctx, preedit("old"));
        let old = app.composition_key().unwrap();
        match case {
            0 => {
                let doc = app.editor.session.document().clone();
                app.editor = Editor::from_document(doc).unwrap();
                app.controls.tool = Tool::Text;
                app.editor.tools.choose_tool(Tool::Text);
                app.editor.tools.set_text_style(app.text.style.clone());
                app.editor
                    .tools
                    .begin_text(app.editor.target.unwrap(), (7, 7));
                app.editor.tools.type_text("New");
            }
            1 => {
                use paintcraft_core::types::json::Value;
                app.editor
                    .commit(vec![Value::object([
                        ("op", Value::str("layer.set_properties")),
                        (
                            "layer_ref",
                            crate::editor::layer_ref(app.editor.target.unwrap()),
                        ),
                        ("opacity_u8", Value::Int(130)),
                    ])])
                    .unwrap();
            }
            2 => {
                app.controls.tool = Tool::Brush;
                app.editor.tools.choose_tool(Tool::Brush);
            }
            3 => app.editor.target = Some(app.editor.session.document().roots[1]),
            4 => {
                app.frontend = if app.frontend == Frontend::Studio {
                    Frontend::Classic
                } else {
                    Frontend::Studio
                }
            }
            5 => {
                app.editor.tools.type_text("!");
            }
            _ => {
                let mut style = app.text.style.clone();
                style.bold = true;
                app.editor.tools.set_text_style(style);
            }
        }
        let base = app.editor.tools.text_content().map(str::to_owned);
        app.guard_composition(&ctx);
        assert!(!app.text.is_composing(), "{case}");
        assert_ne!(app.composition_key(), Some(old));
        app.text_event(&ctx, commit("late"));
        assert_eq!(app.editor.tools.text_content(), base.as_deref(), "{case}");
    }
}
#[test]
fn invalid_or_excessive_input_is_atomic_and_never_reuses_previous_candidate() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    for text in ["x".repeat(4097), "x".repeat(4092), "bad\tcontrol".into()] {
        app.text_event(&ctx, preedit("valid"));
        app.text_event(&ctx, preedit(&text));
        assert!(!app.text.is_composing());
        assert!(app.text.preedit().is_empty());
        assert_eq!(app.editor.tools.text_content(), Some("Base "));
        app.text_event(&ctx, commit("late"));
        assert_eq!(app.editor.tools.text_content(), Some("Base "));
    }
    app.text_event(&ctx, preedit("valid"));
    app.text_event(&ctx, commit(&"x".repeat(4092)));
    assert_eq!(app.editor.tools.text_content(), Some("Base "));
    assert!(!app.text.is_composing());
    app.text_event(
        &ctx,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "界".into(),
            active_range_chars: Some(0..0),
        }),
    );
    assert!(!app.text.is_composing());
    assert_eq!(app.editor.session.undo_depth(), 0);
}
#[test]
fn empty_result_does_not_erase_selection_and_deletion_is_exact_or_rejected() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    app.text.selected_all = true;
    app.text_event(&ctx, preedit("candidate"));
    app.text_event(&ctx, commit(""));
    assert_eq!(app.editor.tools.text_content(), Some("Base "));
    let stamp = app.editor.tools.text_stamp();
    app.text_event(
        &ctx,
        egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars: 5000,
            after_chars: 0,
        }),
    );
    assert_eq!(app.editor.tools.text_stamp(), stamp);
    assert_eq!(app.editor.tools.text_content(), Some("Base "));
    app.text_event(&ctx, preedit("新"));
    app.text_event(&ctx, commit("新"));
    assert_eq!(app.editor.tools.text_content(), Some("Base 新"));
}

fn delete(count: usize) -> egui::Event {
    egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
        before_chars: count,
        after_chars: 0,
    })
}
#[test]
fn producer_prefix_replacement_and_select_all_are_private_until_the_real_result() {
    for selected in [false, true] {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.text.selected_all = selected;
        let source = paintcraft_formats::pcnd::write(app.editor.session.document()).unwrap();
        let stamp = app.editor.tools.text_stamp();
        assert!(app.canvas_text_input_pair(
            &ctx,
            &delete(if selected { 5 } else { 2 }),
            &preedit("置換")
        ));
        assert_eq!(app.editor.tools.text_content(), Some("Base "));
        assert_eq!(app.editor.tools.text_stamp(), stamp);
        let preview = pixels(&app);
        assert_eq!(
            paintcraft_formats::pcnd::write(app.editor.session.document()).unwrap(),
            source
        );
        app.text_event(&ctx, commit("置換"));
        assert_eq!(
            app.editor.tools.text_content(),
            Some(if selected { "置換" } else { "Bas置換" })
        );
        assert_eq!(pixels(&app), preview);
        assert_eq!(app.editor.session.undo_depth(), 0);
        app.editor.place_text().unwrap();
        assert_eq!(
            paintcraft_render::render_flat(app.editor.session.document()).unwrap(),
            preview
        );
    }
}
#[test]
fn repeated_producer_prefix_shrink_stays_private_in_one_composition() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    assert!(app.canvas_text_input_pair(&ctx, &delete(1), &preedit("first")));
    assert!(app.canvas_text_input_pair(&ctx, &delete(2), &preedit("新")));
    let expected = pixels(&app);
    assert_eq!(app.editor.tools.text_content(), Some("Base "));
    app.text_event(&ctx, commit("新"));
    assert_eq!(app.editor.tools.text_content(), Some("Ba新"));
    assert_eq!(pixels(&app), expected);
}
#[test]
fn prefix_cancel_empty_result_and_invalid_pairs_restore_the_original_draft() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    for selected in [false, true] {
        app.text.selected_all = selected;
        assert!(app.canvas_text_input_pair(&ctx, &delete(2), &preedit("candidate")));
        app.text_event(&ctx, commit(""));
        assert!(!app.text.is_composing());
        assert_eq!(app.editor.tools.text_content(), Some("Base "));
        app.text_event(&ctx, egui::Event::Text("!".into()));
        assert_eq!(app.editor.tools.text_content(), Some("Base !"));
        app.editor.tools.backspace();
    }
    for (count, text) in [
        (5000, "valid".to_owned()),
        (2, "bad\tinput".to_owned()),
        (2, "x".repeat(4097)),
    ] {
        let stamp = app.editor.tools.text_stamp();
        assert!(app.canvas_text_input_pair(&ctx, &delete(count), &preedit(&text)));
        assert!(!app.text.is_composing());
        assert_eq!(app.editor.tools.text_content(), Some("Base "));
        assert_eq!(app.editor.tools.text_stamp(), stamp);
    }
}
#[test]
fn standalone_delete_and_ordinary_producer_replacements_stay_atomic() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    app.text_event(&ctx, delete(2));
    assert_eq!(app.editor.tools.text_content(), Some("Bas"));
    let stamp = app.editor.tools.text_stamp();
    assert!(app.canvas_text_input_pair(&ctx, &delete(1), &egui::Event::Text("x".repeat(4097))));
    assert_eq!(app.editor.tools.text_content(), Some("Bas"));
    assert_eq!(app.editor.tools.text_stamp(), stamp);
    assert!(app.canvas_text_input_pair(&ctx, &delete(1), &egui::Event::Text("e".into())));
    assert_eq!(app.editor.tools.text_content(), Some("Bae"));
    let stamp = app.editor.tools.text_stamp();
    app.text.selected_all = true;
    assert!(app.canvas_text_input_pair(&ctx, &delete(5000), &egui::Event::Text("bad".into())));
    assert_eq!(app.editor.tools.text_content(), Some("Bae"));
    assert_eq!(app.editor.tools.text_stamp(), stamp);

    assert_eq!(app.editor.session.undo_depth(), 0);
}
