use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, format_message, translate};
use std::{
    fs,
    time::{Duration, Instant},
};
const PIXEL: [u8; 4] = [13, 71, 201, 0];
struct FileFixture {
    root: PathBuf,
    path: PathBuf,
    bytes: Vec<u8>,
}
impl FileFixture {
    fn new(unsupported_first: bool) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/tiff-selected-page")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let path = root.join("pages.tiff");
        let bytes = fixture(unsupported_first);
        fs::write(&path, &bytes).unwrap();
        Self { root, path, bytes }
    }
}
impl Drop for FileFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
// Independent little-endian uncompressed classic TIFF, two straight-alpha RGBA
// IFDs. Directory and payload offsets are authored here, not decoder output.
fn fixture(unsupported_first: bool) -> Vec<u8> {
    let mut bytes = vec![0; 8 + 2 * 154];
    bytes[..4].copy_from_slice(b"II*\0");
    bytes[4..8].copy_from_slice(&8u32.to_le_bytes());
    for index in 0..2usize {
        let base = 8 + index * 154;
        let bits = base + 138;
        let strip = bits + 8;
        bytes[base..base + 2].copy_from_slice(&11u16.to_le_bytes());
        let tags = [
            (256, 4, 1, 1),
            (257, 4, 1, 1),
            (258, 3, 4, bits as u32),
            (259, 3, 1, 1),
            (
                262,
                3,
                1,
                if index == 0 && unsupported_first {
                    5
                } else {
                    2
                },
            ),
            (273, 4, 1, strip as u32),
            (277, 3, 1, 4),
            (278, 4, 1, 1),
            (279, 4, 1, 4),
            (284, 3, 1, 1),
            (338, 3, 1, 2),
        ];
        for (n, (tag, kind, count, value)) in tags.into_iter().enumerate() {
            let at = base + 2 + n * 12;
            bytes[at..at + 2].copy_from_slice(&(tag as u16).to_le_bytes());
            bytes[at + 2..at + 4].copy_from_slice(&(kind as u16).to_le_bytes());
            bytes[at + 4..at + 8].copy_from_slice(&(count as u32).to_le_bytes());
            bytes[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
        }
        if index == 0 {
            bytes[base + 134..base + 138].copy_from_slice(&(8 + 154u32).to_le_bytes());
        }
        bytes[bits..bits + 8].copy_from_slice(&[8, 0, 8, 0, 8, 0, 8, 0]);
        bytes[strip..strip + 4].copy_from_slice(if index == 0 {
            &[99, 88, 77, 255]
        } else {
            &PIXEL
        });
    }
    bytes
}
fn app() -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.initialize_editing(app.context.clone());
    app
}
fn source(app: &App) -> &Source {
    match &app.pending_import.as_ref().expect("pending TIFF").candidate {
        Candidate::Tiff(source) => source,
        _ => panic!("not TIFF"),
    }
}
fn poll(app: &mut App, selected: bool) {
    let ctx = app.context.clone();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        app.poll_tiff_import(&ctx);
        assert!(app.pending_import.is_some(), "{}", app.editor.status());
        let source = source(app);
        if source.job.is_none() {
            assert!(
                if selected {
                    source.prepared.is_some()
                } else {
                    source.frozen.is_some()
                },
                "{:?}",
                source.error
            );
            break;
        }
        assert!(Instant::now() < end, "TIFF phase did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        app.worker_budget.usage().reserved,
        0,
        "ready chooser must release its transient worker reservation"
    );
}
fn prepared(fixture: &FileFixture) -> App {
    let mut app = app();
    app.stage_tiff_import(fixture.path.clone()).unwrap();
    poll(&mut app, false);
    let ctx = app.context.clone();
    app.choose_tiff_page(&ctx, 1);
    poll(&mut app, true);
    app
}
fn retire(app: &App) {
    let end = Instant::now() + Duration::from_secs(10);
    while app.worker_budget.usage().reserved != 0 {
        app.editing.as_ref().unwrap().reap_abandoned();
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn finish_edits(app: &mut App) {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        app.editor.run_pending();
        if !app.editor.has_pending_edits() {
            break;
        }
        assert!(Instant::now() < end, "edit did not retire");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn authorize_preview(app: &mut App) {
    let ctx = app.context.clone();
    for _ in 0..2 {
        let mut output = ctx.run_ui(Default::default(), |ui| app.tiff_import_dialog(ui.ctx()));
        output.textures_delta.clear();
    }
    let Candidate::Tiff(source) = &mut app.pending_import.as_mut().unwrap().candidate else {
        panic!()
    };
    source.review.as_mut().unwrap().consent_for_test();
}
#[test]
fn async_inventory_requires_explicit_later_page_and_visible_loss_consent_before_one_editable_copy()
{
    let fixture = FileFixture::new(true);
    let mut app = app();
    let ctx = app.context.clone();
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    app.import_selected(fixture.path.clone());
    assert!(app.pending_import.is_some(), "{}", app.editor.status());
    assert_eq!(source(&app).selected, None);
    app.resolve_import(&ctx, true);
    assert!(app.pending_import.is_some());
    poll(&mut app, false);
    let frozen = source(&app).frozen.as_ref().unwrap();
    assert!(!frozen.choices[0].supported);
    assert!(frozen.choices[1].supported);
    app.choose_tiff_page(&ctx, 0);
    assert!(source(&app).job.is_none());
    assert!(source(&app).prepared.is_none());
    app.choose_tiff_page(&ctx, 1);
    poll(&mut app, true);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(ticket, app.editor.history_view().ticket);
    assert!(
        source(&app)
            .prepared
            .as_ref()
            .unwrap()
            .losses
            .iter()
            .any(|loss| loss == pages::LOSS_OTHER_PAGES)
    );
    app.accept_tiff_import(&ctx);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    authorize_preview(&mut app);
    app.accept_tiff_import(&ctx);
    assert!(app.pending_import.is_none());
    assert_eq!(app.editor.document().canvas, (1, 1));
    let doc = app.editor.document();
    assert_eq!(doc.layers[&doc.roots[0]].raster.get(0, 0).0, PIXEL);
    assert!(app.editor.path().is_none() && app.bound.is_none());
    assert!(app.editor.is_dirty());
    assert!(app.recent.entries.is_empty());
    assert_eq!(app.imported_source.as_deref(), Some(fixture.path.as_path()));
    assert_eq!(app.auxiliary_sources.len(), 1);
    assert!(app.guard_save_destination(&fixture.path).is_err());
    assert!(app.guard_export_destination(&fixture.path).is_err());
    let alias = fixture.root.join("source-alias.tiff");
    fs::hard_link(&fixture.path, &alias).unwrap();
    assert!(app.guard_save_destination(&alias).is_err());
    assert!(app.guard_export_destination(&alias).is_err());
    let imported = app.editor.document_snapshot();
    app.editor.request(
        80001,
        Action::ClearCanvas {
            color: Rgba([1, 2, 3, 255]),
        },
    );
    finish_edits(&mut app);
    assert_ne!(
        paintcraft_render::render_flat(app.editor.document()).unwrap(),
        paintcraft_render::render_flat(&imported).unwrap()
    );
    app.editor.request(80002, Action::Undo);
    finish_edits(&mut app);
    assert_eq!(
        paintcraft_render::render_flat(app.editor.document()).unwrap(),
        paintcraft_render::render_flat(&imported).unwrap()
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), fixture.bytes);
}
#[test]
fn source_changes_cancel_stale_owner_and_new_drafts_never_publish_or_retarget() {
    for case in 0..7 {
        let fixture = FileFixture::new(false);
        let mut app = prepared(&fixture);
        let ctx = app.context.clone();
        authorize_preview(&mut app);
        match case {
            0 => fs::write(
                &fixture.path,
                fixture
                    .bytes
                    .iter()
                    .enumerate()
                    .map(|(n, b)| {
                        if n == fixture.bytes.len() - 8 {
                            b ^ 1
                        } else {
                            *b
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            1 => {
                let displaced = fixture.root.join("old.tiff");
                fs::rename(&fixture.path, &displaced).unwrap();
                fs::write(&fixture.path, &fixture.bytes).unwrap();
            }
            2 => {
                app.editor = Editor::with_document(app.editor.document().clone(), None);
            }
            3 => app.settings.experience = Experience::Classic,
            4 => {
                app.editor.begin_text(Uuid::nil(), (0, 0));
            }
            5 => {
                app.add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                    .unwrap();
            }
            _ => app.resolve_import(&ctx, false),
        }
        let before = app.editor.document_snapshot();
        let history = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        app.accept_tiff_import(&ctx);
        assert!(
            Arc::ptr_eq(&before, &app.editor.document_snapshot()),
            "case {case}"
        );
        assert_eq!(history, app.editor.history_view().ticket);
        assert_eq!(dirty, app.editor.is_dirty());
        app.resolve_import(&ctx, false);
        retire(&app);
    }
}
#[test]
fn source_and_preview_admission_cancel_and_old_open_policy_are_preserved() {
    let fixture = FileFixture::new(false);
    let mut app = app();
    let ctx = app.context.clone();
    let before = app.editor.document_snapshot();
    assert!(
        app.open_path(&fixture.path).is_err(),
        "ordinary Open must not silently select TIFF page zero"
    );
    app.settings.experience = Experience::Classic;
    assert!(app.stage_tiff_import(fixture.path.clone()).is_err());
    app.settings.experience = Experience::Studio;
    // Exhaust only this fixture's metadata realm: the production default is
    // process-shared and may already contain allocations from parallel Apps.
    let limit = app
        .editing
        .as_ref()
        .unwrap()
        .metadata_budget()
        .usage()
        .limit;
    let budget = paintcraft_runtime::SharedBudget::new(limit);
    let service = app.editing.take().unwrap().with_metadata(budget.clone());
    app.editor.connect_editing(service.clone());
    app.editing = Some(service);
    let lease = budget.reserve(limit).unwrap();
    app.stage_tiff_import(fixture.path.clone()).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while app.pending_import.is_some() {
        app.poll_tiff_import(&ctx);
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    retire(&app);
    drop(lease);
    app.stage_tiff_import(fixture.path.clone()).unwrap();
    app.resolve_import(&ctx, false);
    retire(&app);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn actual_page_two_controls_and_preview_consent_work_in_all_seven_languages() {
    for language in Language::ALL {
        let fixture = FileFixture::new(true);
        let mut app = app();
        app.settings.language = language;
        app.import_selected(fixture.path.clone());
        poll(&mut app, false);
        let before = app.editor.document_snapshot();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1000., 800.))
            .build_ui_state(
                |ui, app: &mut App| {
                    paintcraft_ui::i18n::set_language(ui.ctx(), app.settings.language);
                    app.tiff_import_dialog(ui.ctx());
                },
                app,
            );
        h.run();
        let label = |number| {
            format_message(
                language,
                "Page { $number }: { $width } × { $height } px",
                &[
                    ("number", number),
                    ("width", "1".into()),
                    ("height", "1".into()),
                ],
            )
        };
        assert!(
            h.get_by_label(&label("1".into()))
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            h.get_by_label(&translate(language, "Import selected page"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&label("2".into())).click();
        h.run();
        h.get_by_role_and_label(
            egui::Role::Button,
            &translate(language, "Import selected page"),
        )
        .click();
        let end = Instant::now() + Duration::from_secs(10);
        // The click starts asynchronous work whose spinner intentionally keeps
        // repainting. Poll individual frames, not Harness::run's settle limit.
        h.step();
        while source(h.state()).prepared.is_none() {
            h.step();
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        h.run();
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        h.get_by_label(&translate(
            language,
            "Only the selected TIFF page will be imported.",
        ));
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&translate(language, "Accept import limitations"))
            .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        assert!(h.state().pending_import.is_none());
        let doc = h.state().editor.document();
        assert_eq!(doc.layers[&doc.roots[0]].raster.get(0, 0).0, PIXEL);
    }
}

#[test]
fn independent_libtiff_selected16_uses_the_reported_high_byte_policy_and_retains_hidden_rgb() {
    const INPUT: &[u8] = include_bytes!(
        "../../../../../../crates/services/paintcraft-formats/tests/tiff-fixtures/unsupported-first-rgba16.tif"
    );
    const EXPECTED: [u8; 16] = [
        0, 0, 1, 0, 171, 152, 118, 128, 255, 0, 127, 255, 1, 128, 255, 1,
    ];
    let mut fixture = FileFixture::new(false);
    fixture.bytes = INPUT.to_vec();
    fs::write(&fixture.path, &fixture.bytes).unwrap();
    let mut app = prepared(&fixture);
    let ctx = app.context.clone();
    let prepared = source(&app).prepared.as_ref().unwrap();
    assert_eq!(prepared.assessment.source_depth(), 16);
    assert_eq!(prepared.assessment.policy(), pages::CONVERSION_POLICY);
    assert_eq!(prepared.thumbnail, EXPECTED);
    assert!(
        prepared
            .losses
            .iter()
            .any(|loss| loss == "depth-16-reduced-to-8")
    );
    assert!(
        prepared
            .losses
            .iter()
            .any(|loss| loss == pages::LOSS_OTHER_PAGES)
    );
    let before = app.editor.document_snapshot();
    app.accept_tiff_import(&ctx);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    authorize_preview(&mut app);
    app.accept_tiff_import(&ctx);
    let doc = app.editor.document();
    assert_eq!(doc.canvas, (2, 2));
    for (index, pixel) in EXPECTED.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            doc.layers[&doc.roots[0]]
                .raster
                .get((index % 2) as i64, (index / 2) as i64)
                .0
                .as_slice(),
            pixel
        );
    }
    assert_eq!(fs::read(&fixture.path).unwrap(), INPUT);
}

#[test]
fn retained_source_thumbnail_and_upload_copy_have_independent_fallible_lifetimes() {
    use paintcraft_core::types::ErrorCode;
    let fixture = FileFixture::new(false);
    let source_charge = fixture.bytes.len() as u64 + (1 << 20);
    for spare in [4, 8] {
        let budget = SharedBudget::new(source_charge + spare);
        let identity = SourceIdentity::existing_destination(&fixture.path)
            .unwrap()
            .unwrap();
        let frozen = worker::inventory(
            identity,
            fixture.path.clone(),
            fixture.bytes.len() as u64,
            budget.clone(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(budget.usage().reserved, source_charge);
        let residency = SharedBudget::new(16 << 20);
        let prepared = worker::decode(
            frozen.clone(),
            fixture.path.clone(),
            1,
            budget.clone(),
            residency.clone(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(budget.usage().reserved, source_charge + 4);
        let review = review::Review::new(&egui::Context::default(), &prepared);
        if spare == 4 {
            assert!(matches!(review,Err(ref error) if error.code==ErrorCode::ResourceLimit));
            assert_eq!(budget.usage().reserved, source_charge + 4);
        } else {
            assert!(review.is_ok());
            assert_eq!(budget.usage().reserved, source_charge + 8);
        }
        drop(prepared);
        assert_eq!(
            residency.usage().reserved,
            0,
            "private editable tiles retire with the prepared result"
        );
        drop(frozen);
        assert_eq!(
            budget.usage().reserved,
            if spare == 4 { 0 } else { 4 },
            "upload copy remains charged until its texture owner retires"
        );
        drop(review);
        assert_eq!(budget.usage().reserved, 0);
    }
    let budget = SharedBudget::new(source_charge + 8);
    let identity = SourceIdentity::existing_destination(&fixture.path)
        .unwrap()
        .unwrap();
    let cancelled = worker::inventory(
        identity,
        fixture.path.clone(),
        fixture.bytes.len() as u64,
        budget.clone(),
        &|| {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel inventory",
            ))
        },
    );
    assert!(matches!(cancelled,Err(ref error) if error.code==ErrorCode::OperationCancelled));
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn source_modified_between_inventory_and_selected_decode_never_creates_a_preview() {
    let fixture = FileFixture::new(false);
    let mut app = app();
    let ctx = app.context.clone();
    app.stage_tiff_import(fixture.path.clone()).unwrap();
    poll(&mut app, false);
    let before = app.editor.document_snapshot();
    let mut changed = fixture.bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    fs::write(&fixture.path, &changed).unwrap();
    app.choose_tiff_page(&ctx, 1);
    let end = Instant::now() + Duration::from_secs(10);
    while source(&app).job.is_some() {
        app.poll_tiff_import(&ctx);
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(source(&app).prepared.is_none() && source(&app).review.is_none());
    assert!(source(&app).error.is_some());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.resolve_import(&ctx, false);
    retire(&app);
    assert_eq!(fs::read(&fixture.path).unwrap(), changed);
}

#[test]
fn different_page_choices_have_distinct_document_state_and_layer_ids_and_repeat_stably() {
    let fixture = FileFixture::new(false);
    let budget = SharedBudget::new(4 << 20);
    let identity = SourceIdentity::existing_destination(&fixture.path)
        .unwrap()
        .unwrap();
    let frozen = worker::inventory(
        identity,
        fixture.path.clone(),
        fixture.bytes.len() as u64,
        budget.clone(),
        &|| Ok(()),
    )
    .unwrap();
    let residency = SharedBudget::new(16 << 20);
    let decode = |index| {
        worker::decode(
            frozen.clone(),
            fixture.path.clone(),
            index,
            budget.clone(),
            residency.clone(),
            &|| Ok(()),
        )
        .unwrap()
    };
    let first = decode(0);
    let second = decode(1);
    let repeated = decode(1);
    assert_ne!(first.assessment.digest(), second.assessment.digest());
    assert_eq!(
        first.assessment.source_sha256(),
        second.assessment.source_sha256()
    );
    assert_ne!(first.document.document_id, second.document.document_id);
    assert_ne!(first.document.state_id, second.document.state_id);
    assert_ne!(first.document.roots, second.document.roots);
    assert_ne!(
        first.document.layers[&first.document.roots[0]]
            .raster
            .get(0, 0),
        second.document.layers[&second.document.roots[0]]
            .raster
            .get(0, 0)
    );
    assert_eq!(second.document.document_id, repeated.document.document_id);
    assert_eq!(second.document.state_id, repeated.document.state_id);
    assert_eq!(second.document.roots, repeated.document.roots);
    assert_eq!(second.assessment.digest(), repeated.assessment.digest());
}

#[path = "followup_tests.rs"]
mod followup;
