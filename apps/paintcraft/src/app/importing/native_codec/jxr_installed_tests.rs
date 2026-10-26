//! Explicit real installed-worker review; no native window is created.
use super::tests::{app as make_app, retire};
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
const INFERRED: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-4x3.jxr"
);
const INPUT: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-dpi96x144.jxr"
);
#[test]
#[ignore = "private copied installation fixture"]
fn fixture_real_installed_jxr_review() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for index in 0..8 {
        let language = if index % 2 == 0 {
            Language::English
        } else {
            Language::Japanese
        };
        let bytes = if index % 2 == 0 { INPUT } else { INFERRED };
        let input = root.join(format!("review-{index}.jxr"));
        fs::write(&input, bytes).unwrap();
        let mut app = make_app();
        app.settings.language = language;
        let before = app.editor.document_snapshot();
        let metadata = app.editing.as_ref().unwrap().metadata_budget();
        let baseline = metadata.usage().reserved;
        app.import_selected(input.clone());
        assert!(app.pending_import.is_some(), "{}", app.editor.status());
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            h.step();
            let pending = h
                .state()
                .pending_import
                .as_ref()
                .expect("review remains pending");
            let Candidate::NativeCodec(source) = &pending.candidate else {
                panic!("codec source")
            };
            if source.review.is_some() {
                break;
            }
            assert!(Instant::now() < until, "{}", h.state().editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert!(metadata.usage().reserved > baseline);
        h.run();
        h.run();
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled()
        );
        if index % 2 == 0 {
            h.get_by_label("96 dpi");
            h.get_by_label("144 dpi");
        } else {
            assert_eq!(h.query_all_by_label("96 dpi").count(), 0);
            assert_eq!(
                h.query_all_by_label(&translate(language, "Default (96 dpi)"))
                    .count(),
                2
            );
            assert_eq!(h.query_all_by_label("0 dpi").count(), 0);
        }
        if index / 2 == 1 {
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                .click();
            h.run();
            assert!(h.state().pending_import.is_none());
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            assert_eq!(metadata.usage().reserved, baseline);
            assert!(h.state().auxiliary_sources.is_empty());
            retire(h.state());
            continue;
        }
        h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(language, "Accept import limitations"),
        )
        .click();
        h.run();
        h.run();
        assert!(
            !h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled(),
            "consent did not enable Open"
        );
        if index / 2 == 2 {
            fs::write(&input, b"changed after preview").unwrap();
        }
        if index / 2 == 3 {
            h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
            h.run();
            assert!(h.state().pending_import.is_none());
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            assert_eq!(metadata.usage().reserved, baseline);
            assert!(h.state().auxiliary_sources.is_empty());
            retire(h.state());
            continue;
        }
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        h.run();
        assert!(
            h.state().pending_import.is_none(),
            "{}",
            h.state().editor.status()
        );
        if index / 2 == 2 {
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            assert_eq!(metadata.usage().reserved, baseline);
            assert!(h.state().auxiliary_sources.is_empty());
            retire(h.state());
            continue;
        }
        assert_eq!(h.state().editor.document().canvas, (4, 3));
        // Independent C JXRLib fixture oracle: owned gradient, unchanged by
        // either encoded96/144 resolution or inferred96 source fields.
        let doc = h.state().editor.document();
        let layer = &doc.layers[&doc.roots[0]];
        for y in 0..3 {
            for x in 0..4 {
                let rgba = layer.raster.get(x, y).0;
                for (channel, value) in rgba.iter().enumerate().take(3) {
                    assert_eq!(
                        *value,
                        ((x * 47 + y * 13 + channel as i64 * 71) % 256) as u8
                    );
                }
                assert_eq!(rgba[3], 255);
            }
        }
        assert!(h.state().editor.is_dirty());
        assert_eq!(h.state().auxiliary_sources.len(), 1);
        assert_eq!(metadata.usage().reserved, baseline);
        let moved = root.join(format!("moved-{index}.jxr"));
        fs::rename(&input, &moved).unwrap();
        let alias = root.join(format!("alias-{index}.pcnd"));
        fs::hard_link(&moved, &alias).unwrap();
        let ctx = h.state().context.clone();
        h.state_mut().save_selected(&ctx, alias.clone());
        assert_eq!(fs::read(alias).unwrap(), bytes);
        retire(h.state());
    }
}
