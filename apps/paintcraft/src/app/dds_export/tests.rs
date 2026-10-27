use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rgba;
use paintcraft_ui::dds_export::{Encoding, Filter, Mips, Quality, Transfer};
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
fn document(pixel: [u8; 4]) -> paintcraft_document::Document {
    paintcraft_document::new_document(7, 5, Some(Rgba(pixel)), 0, paintcraft_document::POLICY)
        .unwrap()
}
fn app(language: Language, source: &Path, pixel: [u8; 4]) -> App {
    fs::write(
        source,
        paintcraft_formats::pcnd::write(&document(pixel)).unwrap(),
    )
    .unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.settings.language = language;
    app.initialize_editing(app.context.clone());
    app.open_path(source).unwrap();
    app
}
fn harness(app: App) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_max_steps(40)
        .with_size(egui::vec2(1100.0, 1000.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    h.run();
    h
}
fn prepared(h: &mut Harness<App>) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        h.step();
        if let Some(s) = &h.state().dds_export {
            assert!(s.error.is_none(), "{:?}", s.error);
            if s.texture.is_some() && s.shown.is_some() {
                break;
            }
        } else {
            panic!("{}", h.state().editor.status());
        }
        assert!(Instant::now() < until, "DDS preparation stalled");
        std::thread::sleep(Duration::from_millis(1));
    }
    h.run();
}
fn choose(h: &mut Harness<App>, lang: Language) {
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Choose destination…"))
        .click();
    h.run();
    assert!(h.state().dds_export.as_ref().unwrap().destination.is_some());
}
#[test]
fn effective_defaults_do_not_acquire_an_advanced_route_from_inactive_fields() {
    for mips in [Mips::None, Mips::Custom(1), Mips::Full] {
        let settings = Settings {
            mips,
            filter: Filter::BoxStored,
            quality: Quality::High,
            ..Default::default()
        };
        assert!(!advanced(settings, (1, 1)));
        assert_eq!(effective(settings, (1, 1)).unwrap(), Settings::default());
    }
    assert!(advanced(
        Settings {
            mips: Mips::Full,
            ..Default::default()
        },
        (7, 5)
    ));
    assert!(
        effective(
            Settings {
                mips: Mips::Custom(0),
                ..Default::default()
            },
            (7, 5)
        )
        .is_err()
    );
}
#[test]
fn admitted_worker_retains_one_artifact_and_shows_linear_samples_in_srgb() {
    let ctx = egui::Context::default();
    let service = controller::background::Service::new(ctx).unwrap();
    let doc = Arc::new(document([128, 188, 255, 127]));
    let settings = Settings {
        transfer: Transfer::Linear8,
        ..Default::default()
    };
    let quote = worker::quote(&doc, settings).unwrap();
    let pending = service
        .submit_dds_export(quote, {
            let doc = doc.clone();
            move |cancel| worker::prepare(doc, settings, quote, cancel)
        })
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    let mut result = loop {
        if let Some(result) = pending.poll() {
            break result.unwrap();
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(result.artifact.source_state(), doc.state_id);
    assert!(
        result
            .artifact
            .losses()
            .iter()
            .any(|s| s == export::dds::LINEAR_QUANTIZATION_LOSS)
    );
    assert_eq!(
        &result.artifact.encoded().bytes()[148..152],
        &[55, 128, 255, 127]
    );
    let image = result.preview.take().unwrap();
    let expected = egui::Color32::from_rgba_unmultiplied(128, 188, 255, 127);
    assert!(image.pixels.iter().all(|p| *p == expected));
    assert_ne!(
        expected,
        egui::Color32::from_rgba_unmultiplied(55, 128, 255, 127)
    );
    drop(result);
    drop(pending);
}
#[test]
fn actual_english_and_japanese_export_preserves_document_and_full_mips() {
    for language in [Language::English, Language::Japanese] {
        for encoding in [Encoding::Rgba8, Encoding::Bc1, Encoding::Bc2, Encoding::Bc3] {
            let root = std::env::temp_dir().join(format!("paintcraft-dds-ui-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let source = root.join("original.pcnd");
            let target = root.join("texture.dds");
            let mut app = app(language, &source, [255, 0, 0, 255]);
            let snapshot = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            let dirty = app.editor.is_dirty();
            let bound = app.bound.clone();
            let text = target.with_extension("pcnd").to_string_lossy().into_owned();
            app.path_text = text.clone();
            app.begin_dds_export(Settings {
                encoding,
                mips: Mips::Full,
                filter: Filter::BoxStored,
                quality: Quality::Normal,
                ..Default::default()
            });
            let mut h = harness(app);
            prepared(&mut h);
            assert!(!target.exists());
            assert!(
                h.query_all_by_label_contains(&translate(language, "Base image preview"))
                    .count()
                    > 0
            );
            if encoding != Encoding::Rgba8 {
                assert!(
                    h.query_all_by_label_contains(&translate(
                        language,
                        "Block compression can change colors and transparency."
                    ))
                    .count()
                        > 0
                );
            }
            assert!(
                !h.state()
                    .dds_export
                    .as_ref()
                    .unwrap()
                    .prepared
                    .as_ref()
                    .unwrap()
                    .artifact
                    .losses()
                    .iter()
                    .any(|s| s == dds::OTHER_MIPS_LOSS)
            );
            choose(&mut h, language);
            let exact = h
                .state()
                .dds_export
                .as_ref()
                .unwrap()
                .prepared
                .as_ref()
                .unwrap()
                .artifact
                .encoded()
                .bytes()
                .to_vec();
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"))
                .click();
            h.run();
            assert_eq!(fs::read(&target).unwrap(), exact);
            let info = dds::inspect_top_mip_checked(&exact, 35, &|| Ok(())).unwrap();
            assert_eq!(info.levels.len(), 3);
            let decoded = dds::decode_top_mip_checked(&exact, 35, &|| Ok(())).unwrap();
            assert!(
                decoded
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [255, 0, 0, 255])
            );
            assert!(Arc::ptr_eq(
                &snapshot,
                &h.state().editor.document_snapshot()
            ));
            assert_eq!(ticket, h.state().editor.history_view().ticket);
            assert_eq!(dirty, h.state().editor.is_dirty());
            assert_eq!(bound, h.state().bound);
            assert_eq!(text, h.state().path_text);
            assert_eq!(h.state().editor.path(), Some(source.as_path()));
            drop(h);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
#[test]
fn actual_cancel_stale_draft_tab_and_renamed_source_alias_never_publish() {
    for language in [Language::English, Language::Japanese] {
        for case in 0..5 {
            let root =
                std::env::temp_dir().join(format!("paintcraft-dds-guard-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let source = root.join("source.pcnd");
            let target = root.join("target.dds");
            let mut app = app(language, &source, [255, 0, 0, 255]);
            app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
            let snapshot = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            app.begin_dds_export(Settings {
                mips: Mips::Full,
                ..Default::default()
            });
            let mut h = harness(app);
            prepared(&mut h);
            match case {
                0 => {
                    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                        .click();
                    h.run();
                }
                1 => {
                    h.state_mut().settings.experience = Experience::Classic;
                    h.run();
                }
                2 => {
                    h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
                    h.run();
                }
                3 => {
                    let renamed = root.join("renamed.pcnd");
                    fs::rename(&source, &renamed).unwrap();
                    fs::hard_link(&renamed, &target).unwrap();
                    h.get_by_role_and_label(
                        egui::Role::Button,
                        &translate(language, "Choose destination…"),
                    )
                    .click();
                    h.run();
                    assert!(h.state().dds_export.is_none());
                }
                _ => {
                    let ctx = h.state().context.clone();
                    h.state_mut()
                        .add_document_tab(Editor::new_document(4, 3).unwrap(), &ctx)
                        .unwrap();
                    let current = h.state().editor.document_snapshot();
                    h.run();
                    assert!(h.state().dds_export.is_none());
                    assert!(Arc::ptr_eq(&current, &h.state().editor.document_snapshot()));
                    assert!(
                        h.state()
                            .tabs
                            .iter()
                            .filter_map(|tab| tab.state.as_ref())
                            .any(|s| Arc::ptr_eq(&snapshot, &s.editor.document_snapshot()))
                    );
                }
            }
            if case != 3 {
                assert!(!target.exists());
            } else {
                assert_eq!(
                    fs::read(&target).unwrap(),
                    fs::read(root.join("renamed.pcnd")).unwrap()
                );
            }
            if case != 4 {
                assert!(Arc::ptr_eq(
                    &snapshot,
                    &h.state().editor.document_snapshot()
                ));
                assert_eq!(ticket, h.state().editor.history_view().ticket);
            }
            drop(h);
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn real_options_controls_prepare_a_new_preview_and_bc1_refusal_remains_editable() {
    for language in [Language::English, Language::Japanese] {
        let root = std::env::temp_dir().join(format!("paintcraft-dds-controls-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.pcnd");
        let target = root.join("texture.dds");
        let mut app = app(language, &source, [128, 188, 255, 127]);
        app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
        app.begin_export_options(paintcraft_ui::actions::ImageExport::Dds);
        let mut h = harness(app);
        assert!(h.state().dds_export.is_none());
        h.get_by_role_and_label(egui::Role::ComboBox, &translate(language, "Mip levels"))
            .click();
        h.run();
        h.get_by_label(&translate(language, "Full mip chain"))
            .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Preview export"))
            .click();
        h.run();
        prepared(&mut h);
        let previous = h
            .state()
            .dds_export
            .as_ref()
            .unwrap()
            .prepared
            .as_ref()
            .unwrap()
            .artifact
            .encoded()
            .bytes()
            .to_vec();
        h.get_by_role_and_label(egui::Role::ComboBox, &translate(language, "DDS encoding"))
            .click();
        h.run();
        h.get_by_label(&translate(language, "BC1 (opaque)")).click();
        h.run();
        let until = Instant::now() + Duration::from_secs(10);
        while h.state().dds_export.as_ref().unwrap().error.is_none() {
            h.step();
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
        h.run();
        assert!(h.state().dds_export.as_ref().unwrap().prepared.is_none());
        assert!(h.state().dds_export.as_ref().unwrap().texture.is_none());
        assert!(
            h.query_all_by_label_contains(&translate(
                language,
                "BC1 requires a picture without transparency. Choose RGBA8, BC2 or BC3."
            ))
            .count()
                > 0
        );
        assert!(!target.exists());
        h.get_by_role_and_label(egui::Role::ComboBox, &translate(language, "DDS encoding"))
            .click();
        h.run();
        h.get_by_label(&translate(language, "BC3 (straight alpha)"))
            .click();
        h.run();
        prepared(&mut h);
        assert_ne!(
            h.state()
                .dds_export
                .as_ref()
                .unwrap()
                .prepared
                .as_ref()
                .unwrap()
                .artifact
                .encoded()
                .bytes(),
            previous
        );
        assert!(h.state().dds_export.as_ref().unwrap().destination.is_none());
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
            .click();
        h.run();
        assert!(!target.exists());
        assert!(h.state().dds_export.is_none());
        assert_eq!(h.state().worker_budget.usage().reserved, 0);
        drop(h);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn actual_nonuniform_bc1_box_controls_publish_the_qualified_whole_file() {
    const PIXELS: &[u8] = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/dds-texture-write-fixtures/sources/7x5-opaque.rgba"
    );
    const BOX: &[u8] = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-BoxStored.dds"
    );
    const NEAREST: &[u8] = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-Nearest.dds"
    );
    assert_ne!(
        BOX, NEAREST,
        "the owned input must distinguish the filter mapping"
    );
    let root = std::env::temp_dir().join(format!("paintcraft-dds-filter-ui-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let target = root.join("texture.dds");
    let mut doc = document([0, 0, 0, 255]);
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster = paintcraft_raster::RasterLayer::from_rgba(
        7,
        5,
        Rgba([0, 0, 0, 255]),
        PIXELS,
        &mut paintcraft_raster::Admission::new(1 << 20),
    )
    .unwrap();
    let original = paintcraft_formats::pcnd::write(&doc).unwrap();
    fs::write(&source, &original).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.initialize_editing(app.context.clone());
    app.open_path(&source).unwrap();
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    let snapshot = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let dirty = app.editor.is_dirty();
    let bound = app.bound.clone();
    let path_text = app.path_text.clone();
    app.begin_export_options(paintcraft_ui::actions::ImageExport::Dds);
    let mut h = harness(app);
    for (control, selection) in [
        ("DDS encoding", "BC1 (opaque)"),
        ("DDS color samples", "sRGB samples"),
        ("Mip levels", "Full mip chain"),
        ("Mip filter", "Box (alpha-weighted stored samples)"),
        ("Encoding effort", "Normal effort"),
    ] {
        h.get_by_role_and_label(egui::Role::ComboBox, control)
            .click();
        h.run();
        h.get_by_label(selection).click();
        h.run();
    }
    h.get_by_role_and_label(egui::Role::Button, "Preview export")
        .click();
    h.run();
    prepared(&mut h);
    assert_eq!(
        h.state()
            .dds_export
            .as_ref()
            .unwrap()
            .prepared
            .as_ref()
            .unwrap()
            .artifact
            .encoded()
            .bytes(),
        BOX
    );
    assert!(!target.exists());
    choose(&mut h, Language::English);
    h.get_by_role_and_label(egui::Role::Button, "Export now")
        .click();
    h.run();
    let actual = fs::read(&target).unwrap();
    assert_eq!(actual, BOX);
    assert_ne!(actual, NEAREST);
    assert_eq!(fs::read(&source).unwrap(), original);
    assert!(Arc::ptr_eq(
        &snapshot,
        &h.state().editor.document_snapshot()
    ));
    assert_eq!(ticket, h.state().editor.history_view().ticket);
    assert_eq!(dirty, h.state().editor.is_dirty());
    assert_eq!(bound, h.state().bound);
    assert_eq!(path_text, h.state().path_text);
    assert_eq!(h.state().editor.path(), Some(source.as_path()));
    drop(h);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn held_budget_without_a_job_reports_an_editable_quota_error_and_can_recover() {
    for language in [Language::English, Language::Japanese] {
        let root = std::env::temp_dir().join(format!("paintcraft-dds-quota-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.pcnd");
        let mut app = app(language, &source, [255, 0, 0, 255]);
        // These stored errors can be created before the first UI locale pass.
        let opaque = Error::new(
            ErrorCode::CapabilityUnsupported,
            "DDS declared opaque source contains nonopaque pixels",
        );
        let opaque_text = translate(
            language,
            "BC1 requires a picture without transparency. Choose RGBA8, BC2 or BC3.",
        );
        assert_eq!(app.dds_preparation_error(&opaque), opaque_text);
        app.dds_error(opaque);
        assert!(app.editor.status().ends_with(&opaque_text));
        app.dds_error(stale());
        assert!(app.editor.status().ends_with(&translate(
            language,
            "The document changed; choose the image again."
        )));
        let budget = paintcraft_runtime::SharedBudget::new(2 << 30);
        app.worker_budget = budget.clone();
        app.initialize_editing(app.context.clone());
        let held = budget.reserve(1).unwrap();
        let service = app.editing.as_ref().unwrap();
        assert_eq!(
            service
                .try_submit_dds_export(64, |_| unreachable!("quota must refuse before work"))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        app.begin_dds_export(Settings {
            mips: Mips::Full,
            ..Default::default()
        });
        let mut h = harness(app);
        let until = Instant::now() + Duration::from_secs(5);
        while h.state().dds_export.as_ref().unwrap().error.is_none() {
            h.step();
            assert!(Instant::now() < until, "quota was treated as live work");
            std::thread::sleep(Duration::from_millis(1));
        }
        h.run();
        let state = h.state().dds_export.as_ref().unwrap();
        assert!(state.pending.is_none() && state.prepared.is_none());
        let message = translate(
            language,
            "Not enough memory to export with these settings. Use a smaller image or fewer layers.",
        );
        assert_eq!(state.error.as_deref(), Some(message.as_str()));
        assert!(h.query_all_by_label_contains(&message).count() > 0);
        assert_eq!(h.state().confirm, Some(Confirm::DdsExport));
        assert!(
            h.query_all_by_label_contains(&translate(language, "DDS encoding"))
                .count()
                > 0
        );
        assert_eq!(budget.usage().reserved, 1);
        drop(held);
        h.get_by_role_and_label(egui::Role::ComboBox, &translate(language, "Mip filter"))
            .click();
        h.run();
        h.get_by_label(&translate(language, "Box (alpha-weighted stored samples)"))
            .click();
        h.run();
        prepared(&mut h);
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
            .click();
        h.run();
        assert_eq!(budget.usage().reserved, 0);
        drop(h);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn only_a_live_executor_job_retries_and_an_oversized_quote_never_does() {
    let budget = paintcraft_runtime::SharedBudget::new(3 << 30);
    let service =
        controller::background::Service::with_budget(egui::Context::default(), budget.clone())
            .unwrap();
    let (started, wait_started) = std::sync::mpsc::channel();
    let (release, wait_release) = std::sync::mpsc::channel();
    let pending = service
        .submit_dds_export(64, move |cancel| {
            started.send(()).unwrap();
            wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
            cancel.check()?;
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "test preparation retired",
            ))
        })
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(pending); // Abandoned generation still executing and retaining admission.
    assert_eq!(budget.usage().reserved, 2 << 30);
    assert!(
        service
            .try_submit_dds_export(64, |_| unreachable!())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        service
            .try_submit_dds_export(u64::MAX, |_| unreachable!())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    release.send(()).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while budget.usage().reserved != 0 {
        service.reap_abandoned();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    // Once the actual job is gone, an unrelated reservation is a final quota
    // refusal, even though the shared budget still reports reserved bytes.
    let held = budget.reserve(2 << 30).unwrap();
    assert_eq!(
        service
            .try_submit_dds_export(64, |_| unreachable!())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    drop(held);
    let doc = Arc::new(document([255, 0, 0, 255]));
    let settings = Settings {
        mips: Mips::Full,
        ..Default::default()
    };
    let quote = worker::quote(&doc, settings).unwrap();
    let pending = service
        .try_submit_dds_export(quote, move |cancel| {
            worker::prepare(doc, settings, quote, cancel)
        })
        .unwrap()
        .unwrap();
    let result = loop {
        if let Some(result) = pending.poll() {
            break result.unwrap();
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(budget.usage().reserved, 2 << 30);
    drop(result);
    drop(pending);
    service.reap_abandoned();
    assert_eq!(budget.usage().reserved, 0);
}
