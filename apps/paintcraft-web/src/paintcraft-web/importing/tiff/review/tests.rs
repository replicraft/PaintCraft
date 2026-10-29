use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, translate};
const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/services/paintcraft-formats/tests/tiff-fixtures/unsupported-first-rgba16.tif"
));
const EXPECTED: [u8; 16] = [
    0, 0, 1, 0, 171, 152, 118, 128, 255, 0, 127, 255, 1, 128, 255, 1,
];
fn app(ctx: &egui::Context) -> WebApp {
    let mut app = WebApp::new(ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app
}
fn prepared() -> (Rc<Held>, Prepared) {
    let value = prepared_source(SOURCE, 1);
    pixels(&value.1.document);
    value
}
fn prepared_source(input: &[u8], index: u16) -> (Rc<Held>, Prepared) {
    let inventory = tiff_pages::inspect(input, &|| Ok(())).unwrap();
    let page = &inventory.pages()[usize::from(index)];
    let decoded =
        tiff_pages::decode_page(input, index, wire::PIXELS_MAX.into(), &|| Ok(())).unwrap();
    let selection = wire::Selection {
        page: wire::Page {
            index,
            width: page.width,
            height: page.height,
            ifd_offset: page.descriptor.ifd_offset,
            supported: true,
        },
        page_count: inventory.pages().len() as u16,
        source_sha256: inventory.source_sha256(),
    };
    let document = response::assemble(
        &decoded.assessment,
        &decoded.rgba,
        paintcraft_raster::Admission::new(0).residency_budget(),
        &|| Ok(()),
    )
    .unwrap();
    let held = Rc::new(
        Held::new(Source {
            name: "pages.tif".into(),
            length: input.len() as u64,
        })
        .unwrap(),
    );
    (
        held,
        Prepared {
            document,
            report: decoded.assessment,
            selection,
        },
    )
}
fn pixels(document: &Document) {
    assert_eq!(document.canvas, (2, 2));
    let raster = &document.layers[&document.roots[0]].raster;
    for (index, expected) in EXPECTED.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            raster.get((index % 2) as i64, (index / 2) as i64).0,
            *expected
        );
    }
}
#[test]
fn small_actual_previews_upscale_without_changing_pixels_or_report_and_need_a_full_prior_frame() {
    const ONE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/tiff-fixtures/rgba16-be-strips-lzw.tif"
    ));
    const SMALL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/tiff-fixtures/icc-srgb-rgba8.tif"
    ));
    for (input, index, canvas) in [(ONE, 1, (1, 1)), (SMALL, 0, (3, 2))] {
        let ctx = egui::Context::default();
        let app = app(&ctx);
        let (held, prepared) = prepared_source(input, index);
        let digest = prepared.report.digest();
        let pixels = paintcraft_render::render_flat(&prepared.document).unwrap();
        let mut review = Review::new(&ctx, held, Owner::capture(&app), prepared).unwrap();
        review.consent = true;
        let mut rect = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_invisible();
            review.preview(ui);
        });
        output.textures_delta.clear();
        assert!(review.shown.is_none(), "hidden UI cannot authorize review");
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_clip_rect(egui::Rect::from_min_size(
                ui.cursor().min,
                egui::Vec2::splat(10.0),
            ));
            rect = review.preview(ui);
        });
        output.textures_delta.clear();
        assert!(
            review.shown.is_none(),
            "a clipped corner cannot authorize review"
        );
        assert!((rect.width() - 200.0).abs() < 0.01);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            rect = review.preview(ui);
            assert_eq!(review.shown, Some(ui.ctx().cumulative_frame_nr()));
        });
        output.textures_delta.clear();
        assert!((rect.height() - 200.0 * canvas.1 as f32 / canvas.0 as f32).abs() <= 0.5);
        let mut output = ctx.run_ui(Default::default(), |ui| assert!(review.show(ui)));
        output.textures_delta.clear();
        assert_eq!(review.prepared.document.canvas, canvas);
        assert_eq!(review.size, egui::vec2(canvas.0 as f32, canvas.1 as f32));
        assert_eq!(review.prepared.report.digest(), digest);
        assert_eq!(
            paintcraft_render::render_flat(&review.prepared.document).unwrap(),
            pixels
        );
    }
}
#[test]
fn actual_selected_tiff_preview_requires_a_visible_previous_frame() {
    let ctx = egui::Context::default();
    let app = app(&ctx);
    let (held, value) = prepared();
    let mut review = Review::new(&ctx, held, Owner::capture(&app), value).unwrap();
    review.consent = true;
    let mut ready = false;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        ui.set_clip_rect(egui::Rect::NOTHING);
        ready = review.show(ui);
    });
    output.textures_delta.clear();
    assert!(!ready);
    assert!(review.shown.is_none());
    let mut output = ctx.run_ui(Default::default(), |ui| ready = review.show(ui));
    output.textures_delta.clear();
    assert!(!ready);
    let mut output = ctx.run_ui(Default::default(), |ui| ready = review.show(ui));
    output.textures_delta.clear();
    assert!(ready);
}
#[test]
fn skinny_preview_has_a_visible_minimum_without_changing_source_dimensions() {
    for source in [
        egui::vec2(65537.0, 1.0),
        egui::vec2(1.0, 65537.0),
        egui::vec2(wire::SIDE_MAX as f32, 1.0),
    ] {
        let size = paintcraft_ui::import_preview::fitted_size(source, egui::Vec2::splat(200.0));
        assert!(size.x >= 1.0 && size.y >= 1.0);
        assert!(size.x <= 200.0 && size.y <= 200.0);
    }
    let size =
        paintcraft_ui::import_preview::fitted_size(egui::vec2(2.0, 2.0), egui::Vec2::splat(200.0));
    assert_eq!(size, egui::Vec2::splat(200.0));
}
#[test]
fn actual_wide_tiff_uses_padded_staged_admission_separately_from_decoder_pixels() {
    // Authored uncompressed classic TIFFs: one opaque blue row/column above 65536.
    for (width, height) in [(65537u32, 1u32), (1, 65537)] {
        let count = 12u16;
        let bits = 8 + 2 + u32::from(count) * 12 + 4;
        let pixels = bits + 8;
        let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
        bytes.extend_from_slice(&count.to_le_bytes());
        for (tag, kind, n, value) in [
            (256u16, 4u16, 1u32, width),
            (257, 4, 1, height),
            (258, 3, 4, bits),
            (259, 3, 1, 1),
            (262, 3, 1, 2),
            (273, 4, 1, pixels),
            (277, 3, 1, 4),
            (278, 4, 1, height),
            (279, 4, 1, width * height * 4),
            (284, 3, 1, 1),
            (338, 3, 1, 2),
            (339, 3, 1, 1),
        ] {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(&n.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend_from_slice(&8u16.to_le_bytes());
        }
        for _ in 0..width * height {
            bytes.extend_from_slice(&[0, 0, 255, 255]);
        }
        let decoded =
            tiff_pages::decode_page(&bytes, 0, wire::PIXELS_MAX.into(), &|| Ok(())).unwrap();
        assert_eq!(decoded.width, width);
        assert_eq!(decoded.rgba.len(), width as usize * height as usize * 4);
        let budget = paintcraft_core::types::resources::SharedBudget::new(96 << 20);
        let document =
            response::assemble(&decoded.assessment, &decoded.rgba, budget.clone(), &|| {
                Ok(())
            })
            .unwrap();
        assert_eq!(document.canvas, (width, height));
        let raster = &document.layers[&document.roots[0]].raster;
        assert_eq!(raster.get(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(
            raster.get(i64::from(width - 1), i64::from(height - 1)).0,
            [0, 0, 255, 255]
        );
        assert!(budget.usage().reserved > 0);
        drop(document);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn same_source_pages_have_distinct_ids_and_repeated_selection_is_deterministic() {
    const SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/tiff-fixtures/rgba8-le-strips-none.tif"
    ));
    let build = |index| {
        let decoded =
            tiff_pages::decode_page(SOURCE, index, wire::PIXELS_MAX.into(), &|| Ok(())).unwrap();
        response::assemble(
            &decoded.assessment,
            &decoded.rgba,
            paintcraft_raster::Admission::new(0).residency_budget(),
            &|| Ok(()),
        )
        .unwrap()
    };
    let first = build(0);
    let other = build(1);
    let repeat = build(0);
    assert_ne!(first.document_id, other.document_id);
    assert_ne!(first.state_id, other.state_id);
    assert_ne!(first.roots, other.roots);
    assert_eq!(first.document_id, repeat.document_id);
    assert_eq!(first.state_id, repeat.state_id);
    assert_eq!(first.roots, repeat.roots);
}
#[test]
fn report_digest_and_captured_inventory_are_both_required_for_adoption() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    for change in 0..5 {
        let owner = Owner::capture(&app);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.history_ticket();
        let (held, mut value) = prepared();
        let digest = value.report.digest();
        let consent = if change == 0 { None } else { Some(digest) };
        match change {
            1 => value.selection.source_sha256[0] ^= 1,
            2 => value.selection.page.index = 0,
            3 => value.selection.page.ifd_offset += 1,
            4 => value.selection.page_count += 1,
            _ => {}
        }
        app.install_tiff(held, owner, value, consent, &ctx);
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(ticket, app.editor.session.history_ticket());
    }
    let owner = Owner::capture(&app);
    let (held, value) = prepared();
    let digest = value.report.digest();
    app.install_tiff(held, owner, value, Some(digest), &ctx);
    pixels(app.editor.session.document());
    assert!(app.editor.session.is_dirty());
}
#[test]
fn real_en_ja_loss_controls_cancel_and_stale_owners_preserve_the_document() {
    for language in [Language::English, Language::Japanese] {
        for action in 0..3 {
            let observed = Rc::new(RefCell::new(None));
            let context = observed.clone();
            let mut h = Harness::builder()
                .with_size(egui::vec2(1360., 1000.))
                .build_ui_state(
                    move |ui, app: &mut WebApp| {
                        *context.borrow_mut() = Some(ui.ctx().clone());
                        if ui.ctx().missing_glyph_policy()
                            != egui::epaint::text::MissingGlyphPolicy::Tofu
                        {
                            paintcraft_ui::fonts::install(ui.ctx());
                            ui.ctx().set_missing_glyph_policy(
                                egui::epaint::text::MissingGlyphPolicy::Tofu,
                            );
                        }
                        paintcraft_ui::i18n::set_language(ui.ctx(), language);
                        app.tiff_dialog(ui.ctx());
                    },
                    app(&egui::Context::default()),
                );
            let ctx = observed.borrow().as_ref().unwrap().clone();
            let owner = Owner::capture(h.state());
            let before = h.state().editor.session.snapshot();
            let ticket = h.state().editor.session.history_ticket();
            let (held, value) = prepared();
            h.state_mut().tiff.review = Some(Review::new(&ctx, held, owner, value).unwrap());
            h.step();
            h.step();
            for loss in ["depth-16-reduced-to-8", tiff_pages::LOSS_OTHER_PAGES] {
                let description = paintcraft_ui::i18n::import_loss_description(loss).unwrap();
                h.get_by_label(&translate(language, description));
            }
            assert!(
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open"))
                    .accesskit_node()
                    .is_disabled()
            );
            if action == 0 {
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                    .click();
                h.step();
                h.step();
            } else if action == 1 {
                h.state_mut().frontend = paintcraft_ui::actions::Frontend::Classic;
                h.state_mut().poll_tiff(&ctx);
            } else {
                h.get_by_role_and_label(
                    egui::Role::CheckBox,
                    &translate(language, "Accept import limitations"),
                )
                .click();
                h.step();
                h.step();
                assert!(
                    !h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open"))
                        .accesskit_node()
                        .is_disabled()
                );
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open"))
                    .click();
                h.step();
                h.step();
                pixels(h.state().editor.session.document());
                assert!(h.state().editor.session.is_dirty());
                let encoded =
                    paintcraft_formats::pcnd::write(h.state().editor.session.document()).unwrap();
                pixels(&paintcraft_formats::pcnd::read(&encoded).unwrap());
            }
            assert!(h.state().tiff.review.is_none());
            if action < 2 {
                assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
                assert_eq!(ticket, h.state().editor.session.history_ticket());
            }
        }
    }
}

#[test]
#[ignore = "requires PAINTCRAFT_TIFF_PCND_DIRECTORY from an actual production browser proof"]
fn actual_browser_downloads_match_independent_libtiff_sample_oracles() {
    use paintcraft_core::types::json;
    use sha2::{Digest, Sha256};
    let root = std::path::PathBuf::from(
        std::env::var_os("PAINTCRAFT_TIFF_PCND_DIRECTORY")
            .expect("explicit actual browser artifact directory"),
    );
    let manifest = json::parse(&std::fs::read(root.join("pcnd-oracles.json")).unwrap()).unwrap();
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/tiff-fixtures");
    let oracle = json::parse(&std::fs::read(fixtures.join("oracle.json")).unwrap()).unwrap();
    let entries = manifest.as_array().unwrap();
    assert_eq!(
        entries.len(),
        12,
        "all adoption/cancel/source-change/quarantine downloads are required"
    );
    let mut identities = std::collections::BTreeMap::new();
    for entry in entries {
        let name = entry.get("name").unwrap().as_str().unwrap();
        let source = entry.get("file").unwrap().as_str().unwrap();
        let index = usize::try_from(entry.get("index").unwrap().as_i64().unwrap()).unwrap();
        let expected_source = match name {
            "selected16.pcnd" | "reopened16.pcnd" => ("unsupported-first-rgba16.tif", 1),
            "same-source-page1.pcnd" => ("rgba8-le-strips-none.tif", 1),
            "baseline8.pcnd"
            | "empty-undo8.pcnd"
            | "inventory-cancel.pcnd"
            | "preview-cancel.pcnd"
            | "changed-preserved.pcnd"
            | "hang-preserved.pcnd"
            | "stale-preserved.pcnd"
            | "trap-preserved.pcnd"
            | "malformed-preserved.pcnd" => ("rgba8-le-strips-none.tif", 0),
            _ => panic!("unexpected artifact {name}"),
        };
        assert_eq!((source, index), expected_source);
        assert!(!name.contains(['/', '\\']));
        assert!(!source.contains(['/', '\\']));
        let fixture = oracle
            .get("fixtures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value.get("file").unwrap().as_str() == Some(source))
            .unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(
                std::fs::read(fixtures.join(source)).unwrap()
            )),
            fixture.get("sha256").unwrap().as_str().unwrap()
        );
        let page = &fixture.get("pages").unwrap().as_array().unwrap()[index];
        assert_eq!(
            page.get("qualified_integer_profile").unwrap().as_bool(),
            Some(true)
        );
        let expected: Vec<u8> = page
            .get("expected_rgba8")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|value| u8::try_from(value.as_i64().unwrap()).unwrap())
            .collect();
        let bytes = std::fs::read(root.join(name)).unwrap();
        assert!(bytes.len() <= 64 << 20);
        let document = paintcraft_formats::pcnd::read_with_limits(
            &bytes,
            wire::PIXELS_MAX.into(),
            paintcraft_core::types::policy::STAGED_PIXELS_MAX,
        )
        .unwrap();
        let width = u32::try_from(page.get("width").unwrap().as_i64().unwrap()).unwrap();
        let height = u32::try_from(page.get("height").unwrap().as_i64().unwrap()).unwrap();
        assert_eq!(document.canvas, (width, height));
        assert_eq!(document.roots.len(), 1);
        let raster = &document.layers[&document.roots[0]].raster;
        for (at, value) in expected.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                raster
                    .get((at % width as usize) as i64, (at / width as usize) as i64)
                    .0,
                *value,
                "{name}: pixel{at}"
            );
        }
        assert!(
            identities
                .insert(
                    name.to_owned(),
                    (
                        document.document_id,
                        document.state_id,
                        document.roots.clone(),
                    ),
                )
                .is_none(),
            "duplicate actual artifact {name}"
        );
        println!(
            "actual TIFF browser PCND {name}: independent {width}x{height} raw RGBA matched including hidden RGB/alpha"
        );
    }
    assert_ne!(
        identities["baseline8.pcnd"],
        identities["same-source-page1.pcnd"]
    );
    assert_eq!(identities["baseline8.pcnd"], identities["empty-undo8.pcnd"]);
    assert_eq!(identities["selected16.pcnd"], identities["reopened16.pcnd"]);
}
