//! Actual captured-source TIFF consumers, normalized previews and saved samples.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::json;
use std::{
    fs,
    time::{Duration, Instant},
};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/tiff-orientation-consumers/runs")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Case {
    name: &'static str,
    page: u16,
    bytes: Vec<u8>,
    rgba: Vec<u8>,
    size: (u32, u32),
    orientation: u32,
    losses: Vec<&'static str>,
}
impl Case {
    fn new(name: &'static str, page: u16) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/services/paintcraft-formats/tests/tiff-orientation-fixtures");
        let oracle = json::parse(&fs::read(root.join("oracle.json")).unwrap()).unwrap();
        let row = oracle
            .get("fixtures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r.get("file").unwrap().as_str() == Some(name))
            .unwrap();
        let bytes = fs::read(root.join(name)).unwrap();
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let pages = row.get("pages").unwrap().as_array().unwrap();
        let selected = &pages[usize::from(page)];
        let rgba: Vec<_> = selected
            .get("rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u8)
            .collect();
        assert_eq!(
            hex(&Sha256::digest(&rgba)),
            selected.get("rgba_sha256").unwrap().as_str().unwrap()
        );
        let mut losses = Vec::new();
        if name.starts_with("gray-") {
            losses.push("depth-16-reduced-to-8");
        }
        if pages.len() > 1 {
            losses.push("tiff-other-pages-not-imported");
        }
        Self {
            name,
            page,
            bytes,
            rgba,
            size: (
                selected.get("width").unwrap().as_i64().unwrap() as u32,
                selected.get("height").unwrap().as_i64().unwrap() as u32,
            ),
            orientation: selected.get("orientation").unwrap().as_i64().unwrap() as u32,
            losses,
        }
    }
    fn tag(&self, selected: bool) -> String {
        format!(
            "{}-{}-{}",
            if selected { "selected" } else { "ordinary" },
            self.name,
            self.page
        )
    }
    fn assert_doc(&self, doc: &paintcraft_document::Document) {
        assert_eq!(doc.canvas, self.size, "{} page{}", self.name, self.page);
        assert_eq!(doc.roots.len(), 1);
        let mut rgba = Vec::new();
        for y in 0..self.size.1 {
            for x in 0..self.size.0 {
                rgba.extend(
                    doc.layers[&doc.roots[0]]
                        .raster
                        .get(i64::from(x), i64::from(y))
                        .0,
                );
            }
        }
        assert_eq!(
            rgba, self.rgba,
            "raw hidden RGB, independent orientation order"
        );
    }
}
fn pending(app: &App) -> &Source {
    let Candidate::Tiff(s) = &app.pending_import.as_ref().unwrap().candidate else {
        panic!()
    };
    s
}
fn poll(app: &mut App, decoded: bool) {
    let until = Instant::now() + Duration::from_secs(10);
    let ctx = app.context.clone();
    loop {
        app.poll_tiff_import(&ctx);
        let s = pending(app);
        if s.job.is_none() {
            assert!(
                if decoded {
                    s.prepared.is_some()
                } else {
                    s.frozen.is_some()
                },
                "{:?}",
                s.error
            );
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn stage(path: &Path, page: u16) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.initialize_editing(app.context.clone());
    app.stage_tiff_import(path.into()).unwrap();
    poll(&mut app, false);
    assert_eq!(pending(&app).selected, None);
    let ctx = app.context.clone();
    app.choose_tiff_page(&ctx, page);
    poll(&mut app, true);
    app
}
fn review(app: App, case: &Case) -> Harness<'static, App> {
    let prepared = pending(&app).prepared.as_ref().unwrap();
    assert_eq!(prepared.losses, case.losses);
    assert_eq!(
        prepared.thumbnail_size,
        [case.size.0 as usize, case.size.1 as usize]
    );
    assert_eq!(
        prepared.thumbnail, case.rgba,
        "actual preview carries normalized hidden samples"
    );
    assert_eq!(prepared.assessment.source_dimensions(), (3, 2));
    assert_eq!(
        prepared.assessment.descriptor().orientation,
        case.orientation
    );
    assert_eq!(prepared.assessment.normalized_orientation(), 1);
    assert_eq!(&prepared.assessment.encode()[..8], b"PCTIFF02");
    case.assert_doc(&prepared.document);
    let expected = import_image(
        case.size.0,
        case.size.1,
        &case.rgba,
        &hex(&prepared.assessment.document_identity()),
    )
    .unwrap();
    assert_eq!(expected.state_id, prepared.document.state_id);
    let legacy = import_image(
        case.size.0,
        case.size.1,
        &case.rgba,
        &hex(&Sha256::digest(&case.bytes)),
    )
    .unwrap();
    assert_ne!(legacy.document_id, prepared.document.document_id);
    assert_ne!(legacy.state_id, prepared.document.state_id);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 850.0))
        .build_ui_state(|ui, app: &mut App| app.tiff_import_dialog(ui.ctx()), app);
    h.run();
    assert!(
        h.get_by_role_and_label(egui::Role::Button, "Open image")
            .accesskit_node()
            .is_disabled()
    );
    let before = h.state().editor.document_snapshot();
    let ticket = h.state().editor.history_view().ticket;
    let ctx = h.ctx.clone();
    h.state_mut().accept_tiff_import(&ctx);
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    assert_eq!(ticket, h.state().editor.history_view().ticket);
    for loss in &case.losses {
        h.get_by_label(import_loss_description(loss).unwrap());
    }
    h.get_by_label("Accept import limitations").click();
    h.run();
    h
}
fn finish_edits(app: &mut App) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        app.editor.run_pending();
        if !app.editor.has_pending_edits() {
            break;
        }
        assert!(Instant::now() < until, "edit did not retire");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn save(app: &mut App, path: &Path) {
    let ctx = app.context.clone();
    app.save_selected(&ctx, path.into());
    app.editor.run_pending();
    let until = Instant::now() + Duration::from_secs(15);
    while app.bound.as_deref() != Some(path) {
        app.poll_edits(&ctx);
        app.finish_save();
        assert!(Instant::now() < until, "{}", app.editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn artifact(case: &Case, selected: bool, path: &Path) {
    if let Some(root) = std::env::var_os("PAINTCRAFT_TIFF_ORIENTATION_EVIDENCE") {
        let root = PathBuf::from(root).join("native");
        fs::create_dir_all(&root).unwrap();
        fs::copy(path, root.join(format!("{}.pcnd", case.tag(selected)))).unwrap();
    }
}
#[test]
fn selected_orientation_reviews_preview_exact_samples_save_once_and_bind_v2_source_policy_ids() {
    let folder = Folder::new();
    for (name, page) in [
        ("rgba8-le-eight-orientations.tif", 0),
        ("rgba8-le-eight-orientations.tif", 3),
        ("rgba8-le-eight-orientations.tif", 5),
        ("rgba8-le-eight-orientations.tif", 7),
        ("rgba8-le-orientation-5.tif", 0),
        ("rgba8-be-orientation-8.tif", 0),
        ("gray-alpha16-be-orientation-2.tif", 0),
        ("gray-alpha16-be-orientation-7.tif", 0),
    ] {
        let case = Case::new(name, page);
        let source = folder.0.join(name);
        fs::write(&source, &case.bytes).unwrap();
        let mut previous = None;
        for repeat in 0..2 {
            let mut h = review(stage(&source, page), &case);
            h.get_by_role_and_label(egui::Role::Button, "Open image")
                .click();
            h.run();
            assert!(h.state().pending_import.is_none());
            let mut app = h.into_state();
            case.assert_doc(app.editor.document());
            assert!(app.editor.is_dirty());
            let id = app.editor.document().state_id;
            if let Some(old) = previous {
                assert_eq!(old, id);
            }
            previous = Some(id);
            let copy = folder.0.join(format!("{}-{repeat}.pcnd", case.tag(true)));
            save(&mut app, &copy);
            let reopened = paintcraft_formats::pcnd::read(&fs::read(&copy).unwrap()).unwrap();
            case.assert_doc(&reopened);
            assert_eq!(reopened.state_id, id);
            app.open_path(&copy).unwrap();
            case.assert_doc(app.editor.document());
            assert_eq!(app.editor.document().state_id, id);
            if repeat == 0 {
                artifact(&case, true, &copy);
            }
            assert_eq!(fs::read(&source).unwrap(), case.bytes);
        }
    }
}
#[test]
fn ordinary_oriented_rgba_both_endians_reopen_without_double_transform_or_invented_losses() {
    let folder = Folder::new();
    for name in ["rgba8-le-orientation-5.tif", "rgba8-be-orientation-8.tif"] {
        let case = Case::new(name, 0);
        let source = folder.0.join(name);
        fs::write(&source, &case.bytes).unwrap();
        let mut previous = None;
        for repeat in 0..2 {
            let mut app = App::new(None);
            app.settings.experience = Experience::Studio;
            app.open_path(&source).unwrap();
            assert!(app.pending_import.is_none(), "no invented metadata loss");
            case.assert_doc(app.editor.document());
            let decoded = image::Decoded {
                format: ImageFormat::Tiff,
                width: case.size.0,
                height: case.size.1,
                rgba: case.rgba.clone(),
                losses: vec![],
            };
            let key = image::document_conversion_key(&case.bytes, &decoded, &|| Ok(()))
                .unwrap()
                .unwrap();
            let expected = import_image(case.size.0, case.size.1, &case.rgba, &key.hex()).unwrap();
            assert_eq!(expected.state_id, app.editor.document().state_id);
            let legacy = import_image(
                case.size.0,
                case.size.1,
                &case.rgba,
                &hex(&Sha256::digest(&case.bytes)),
            )
            .unwrap();
            assert_ne!(legacy.state_id, expected.state_id);
            let id = app.editor.document().state_id;
            if let Some(previous) = previous {
                assert_eq!(previous, id);
            }
            previous = Some(id);
            let copy = folder.0.join(format!("ordinary-{name}-{repeat}.pcnd"));
            save(&mut app, &copy);
            app.open_path(&copy).unwrap();
            case.assert_doc(app.editor.document());
            assert_eq!(app.editor.document().state_id, id);
            if repeat == 0 {
                artifact(&case, false, &copy);
            }
        }
    }
}
#[test]
fn oriented_source_cancel_stale_and_post_save_alias_guards_keep_history_and_pixels() {
    let folder = Folder::new();
    let case = Case::new("gray-alpha16-be-orientation-7.tif", 0);
    for mode in 0..4 {
        let source = folder.0.join(format!("guard-{mode}.tif"));
        fs::write(&source, &case.bytes).unwrap();
        let mut h = review(stage(&source, 0), &case);
        let ctx = h.ctx.clone();
        match mode {
            0 => {
                h.get_by_role_and_label(egui::Role::Button, "Cancel")
                    .click();
                h.run();
            }
            1 => {
                let mut changed = case.bytes.clone();
                *changed.last_mut().unwrap() ^= 1;
                fs::write(&source, changed).unwrap();
            }
            2 => {
                h.state_mut().editor =
                    Editor::with_document(h.state().editor.document().clone(), None);
            }
            _ => h.state_mut().settings.experience = Experience::Classic,
        };
        let before = h.state().editor.document_snapshot();
        let ticket = h.state().editor.history_view().ticket;
        let dirty = h.state().editor.is_dirty();
        h.state_mut().accept_tiff_import(&ctx);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(ticket, h.state().editor.history_view().ticket);
        assert_eq!(dirty, h.state().editor.is_dirty());
        h.state_mut().resolve_import(&ctx, false);
    }
    let source = folder.0.join("alias.tif");
    fs::write(&source, &case.bytes).unwrap();
    let mut h = review(stage(&source, 0), &case);
    h.get_by_role_and_label(egui::Role::Button, "Open image")
        .click();
    h.run();
    let mut app = h.into_state();
    let imported = app.editor.document().clone();
    app.editor.request(
        91001,
        Action::ClearCanvas {
            color: Rgba([7, 8, 9, 255]),
        },
    );
    finish_edits(&mut app);
    assert_ne!(app.editor.document().state_id, imported.state_id);
    app.editor.request(91002, Action::Undo);
    finish_edits(&mut app);
    case.assert_doc(app.editor.document());
    assert_eq!(app.editor.document().state_id, imported.state_id);
    save(&mut app, &folder.0.join("copy.pcnd"));
    let moved = folder.0.join("moved.tif");
    fs::rename(&source, &moved).unwrap();
    let alias = folder.0.join("alias.pcnd");
    fs::hard_link(&moved, &alias).unwrap();
    assert!(app.guard_save_destination(&alias).is_err());
    assert!(app.guard_export_destination(&alias).is_err());
    assert_eq!(fs::read(&moved).unwrap(), case.bytes);
}
