//! Real browser import candidates, consent and retained project round trips.
use super::*;
use crate::{WebApp, importing::Owner};
use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::json;
use std::{path::PathBuf, sync::Arc};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/bmp-retention-fixtures")
}
fn load(files: &Files, name: &str, frontend: Frontend) -> Result<Loaded> {
    decode_input_with_metadata(
        &std::fs::read(fixtures().join(name)).unwrap(),
        name.into(),
        frontend,
        false,
        &files.metadata_budget(),
        &|| Ok(()),
    )
}
#[test]
fn browser_bmp_candidates_keep_independent_pixels_profiles_and_versioned_ids_in_all_interfaces() {
    let manifest = json::parse(&std::fs::read(fixtures().join("oracle.json")).unwrap()).unwrap();
    let mut cases = 0;
    for row in manifest.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap();
        let expected = std::fs::read(fixtures().join(format!("{name}.rgba"))).unwrap();
        let mut first = None;
        for frontend in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
            Frontend::Studio,
        ] {
            let files = Files::default();
            let loaded = load(&files, name, frontend).unwrap();
            assert!(loaded.raster_preview);
            let doc = &loaded.document;
            let raster = &doc.layers[&doc.roots[0]].raster;
            let mut pixels = Vec::new();
            for y in 0..doc.canvas.1 {
                for x in 0..doc.canvas.0 {
                    pixels.extend(raster.get(i64::from(x), i64::from(y)).0);
                }
            }
            assert_eq!(pixels, expected, "{name}, including hidden RGB");
            let has_profile = row.get("profile").unwrap().as_bool().unwrap();
            let density = row
                .get("density")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect::<Vec<_>>();
            let retained = has_profile || density != [0, 0];
            assert_eq!(loaded.document.source_metadata.is_some(), retained);
            assert_eq!(loaded.converted_copy, retained || !loaded.losses.is_empty());
            if let Some(metadata) = &loaded.document.source_metadata {
                if has_profile {
                    assert_eq!(
                        paintcraft_document::hex(&Sha256::digest(metadata.profile().unwrap())),
                        row.get("profile_sha256").unwrap().as_str().unwrap()
                    );
                }
                use paintcraft_document::source_metadata::{ResolutionUnit, SourceFormat};
                assert_eq!(metadata.descriptor().format, SourceFormat::Bmp);
                if density == [0, 0] {
                    assert!(metadata.descriptor().resolution.is_none());
                } else {
                    let resolution = metadata.descriptor().resolution.unwrap();
                    assert_eq!(resolution.unit, ResolutionUnit::Metre);
                    assert_eq!(resolution.horizontal.numerator, density[0]);
                    assert_eq!(resolution.vertical.numerator, density[1]);
                    assert_eq!(resolution.horizontal.denominator, 1);
                    assert_eq!(resolution.vertical.denominator, 1);
                    assert_eq!(
                        resolution.physical().is_some(),
                        density.iter().all(|n| *n > 0)
                    );
                }
                assert_eq!(
                    metadata.descriptor().source_sha256,
                    <[u8; 32]>::from(Sha256::digest(
                        std::fs::read(fixtures().join(name)).unwrap()
                    ))
                );
            }
            let project = pcnd::write_compact(&loaded.document).unwrap();
            if let Some(first) = &first {
                assert_eq!(&project, first);
            } else {
                first = Some(project.clone());
            }
            let reopened = decode_input_with_metadata(
                &project,
                "reopened.pcnd".into(),
                frontend,
                false,
                &files.metadata_budget(),
                &|| Ok(()),
            )
            .unwrap();
            assert!(!reopened.converted_copy);
            assert_eq!(pcnd::write_compact(&reopened.document).unwrap(), project);
            drop(reopened);
            drop(loaded);
            assert_eq!(files.metadata_budget().usage().reserved, 0);
        }
        cases += 1;
    }
    assert_eq!(cases, 13);
}

fn harness(name: &str) -> Harness<'static, WebApp> {
    let name = name.to_owned();
    Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_eframe(move |cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            let loaded = load(&app.files, &name, Frontend::ClassicV11).unwrap();
            app.import_owner = Some(Owner::capture(&app));
            app.accept_loaded(loaded, &cc.egui_ctx);
            app
        })
}
#[test]
fn browser_bmp_visible_review_requires_consent_then_installs_unsaved_retained_project() {
    for name in [
        "rgb24-v124-profile-positive.bmp",
        "rgb24-v108-density-negative.bmp",
        "indexed4-v124-profile-reserved.bmp",
    ] {
        let mut h = harness(name);
        let before = h.state().editor.session.snapshot();
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().resolve_import(&ctx, true);
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert!(h.state().pending_import.is_some());
        h.get_by_label("Accept import limitations").click();
        h.run();
        h.get_by_label("Open image").click();
        h.run();
        assert!(h.state().pending_import.is_none());
        assert!(h.state().editor.session.is_dirty());
        assert!(
            h.state()
                .editor
                .session
                .document()
                .source_metadata
                .is_some()
        );
        let project = pcnd::write_compact(h.state().editor.session.document()).unwrap();
        let loaded = decode_input_with_metadata(
            &project,
            "retained.pcnd".into(),
            Frontend::ClassicV11,
            false,
            &h.state().files.metadata_budget(),
            &|| Ok(()),
        )
        .unwrap();
        let owner = Owner::capture(h.state());
        h.state_mut().import_owner = Some(owner);
        h.state_mut().accept_loaded(loaded, &ctx);
        assert!(!h.state().editor.session.is_dirty());
        assert_eq!(
            pcnd::write_compact(h.state().editor.session.document()).unwrap(),
            project
        );
    }
}

#[test]
fn browser_bmp_cancel_stale_owner_and_quota_failure_release_retained_bytes() {
    for stale in [false, true] {
        let mut h = harness("rgb24-v124-profile-positive.bmp");
        let realm = h.state().files.metadata_budget();
        assert!(realm.usage().reserved > 0);
        let ctx = h.ctx.clone();
        if stale {
            h.state_mut().editor = crate::editor::Editor::new(8, 8).unwrap();
        }
        let before = h.state().editor.session.snapshot();
        h.state_mut().resolve_import(&ctx, true);
        if !stale {
            h.state_mut().resolve_import(&ctx, false);
        }
        assert!(h.state().pending_import.is_none());
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(realm.usage().reserved, 0);
    }
    let files = Files::with_metadata_budget(SharedBudget::new(1));
    assert_eq!(
        load(
            &files,
            "rgb24-v124-profile-positive.bmp",
            Frontend::ClassicV11
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(files.metadata_budget().usage().reserved, 0);
}

#[test]
fn browser_bmp_zero_metadata_needs_no_attachment_budget_and_cancel_never_publishes() {
    let files = Files::with_metadata_budget(SharedBudget::new(0));
    let loaded = load(&files, "rgb24-v40-no-metadata.bmp", Frontend::ClassicV11).unwrap();
    assert!(loaded.document.source_metadata.is_none());
    assert!(!loaded.converted_copy);
    assert_eq!(files.metadata_budget().usage().reserved, 0);
    let bytes = std::fs::read(fixtures().join("rgb24-v124-profile-positive.bmp")).unwrap();
    let budget = SharedBudget::new(1 << 20);
    assert_eq!(
        decode_input_with_metadata(
            &bytes,
            "source.bmp".into(),
            Frontend::Studio,
            true,
            &budget,
            &|| Err(Error::new(ErrorCode::OperationCancelled, "stop"))
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(budget.usage().reserved, 0);
}
