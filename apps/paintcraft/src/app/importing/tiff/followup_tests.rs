//! Independent profile and skinny-display follow-ups to the native consumer.
use super::*;
const SKINNY_PIXEL: [u8; 4] = [13, 71, 201, 255];
#[test]
fn independent_graya16_and_icc_pixels_match_the_review_and_exact_consented_document() {
    const GRAYA: &[u8] = include_bytes!(
        "../../../../../../crates/services/paintcraft-formats/tests/tiff-fixtures/gray-endian-packbits.tif"
    );
    const ICC: &[u8] = include_bytes!(
        "../../../../../../crates/services/paintcraft-formats/tests/tiff-fixtures/icc-srgb-rgba8.tif"
    );
    const GRAYA_PIXELS: &[u8] = &[
        63, 63, 63, 0, 82, 82, 82, 1, 101, 101, 101, 137, 100, 100, 100, 1, 119, 119, 119, 137,
        138, 138, 138, 255,
    ];
    const ICC_PIXELS: &[u8] = &[
        10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
        170, 180, 255,
    ];
    for (input, index, expected, losses) in [
        (
            GRAYA,
            1,
            GRAYA_PIXELS,
            &[
                "depth-16-reduced-to-8",
                "tiff-metadata-ignored",
                "tiff-other-pages-not-imported",
            ][..],
        ),
        (
            ICC,
            0,
            ICC_PIXELS,
            &["tiff-metadata-ignored", "icc-profile-ignored"][..],
        ),
    ] {
        let mut fixture = FileFixture::new(false);
        fixture.bytes = input.to_vec();
        fs::write(&fixture.path, input).unwrap();
        let mut app = app();
        let ctx = app.context.clone();
        let before = app.editor.document_snapshot();
        app.import_selected(fixture.path.clone());
        poll(&mut app, false);
        assert_eq!(
            source(&app).selected,
            None,
            "even a single ICC page requires explicit choice"
        );
        app.resolve_import(&ctx, true);
        assert!(source(&app).prepared.is_none());
        app.choose_tiff_page(&ctx, index);
        let mut h = Harness::builder()
            .with_size(egui::vec2(1000., 800.))
            .build_ui_state(|ui, app: &mut App| app.tiff_import_dialog(ui.ctx()), app);
        let end = Instant::now() + Duration::from_secs(10);
        while source(h.state()).prepared.is_none() {
            h.run();
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        h.run();
        let prepared = source(h.state()).prepared.as_ref().unwrap();
        assert_eq!(
            prepared.thumbnail, expected,
            "display upload uses the independently expected RGBA"
        );
        assert_eq!(
            prepared.assessment.output_sha256(),
            <[u8; 32]>::from(Sha256::digest(expected))
        );
        let actual = prepared
            .losses
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            actual, losses,
            "the displayed losses retain canonical assessment order"
        );
        assert_eq!(prepared.assessment.losses(), losses);
        assert_eq!(prepared.assessment.page_index(), index);
        assert_eq!(
            prepared.assessment.page_count(),
            if index == 1 { 2 } else { 1 }
        );
        assert_eq!(
            prepared.assessment.source_depth(),
            if index == 1 { 16 } else { 8 }
        );
        assert_eq!(
            (prepared.assessment.width(), prepared.assessment.height()),
            (3, 2)
        );
        let descriptor = prepared.assessment.descriptor();
        assert_eq!(
            descriptor.bits_per_sample.values[0],
            if index == 1 { 16 } else { 8 }
        );
        assert_eq!(descriptor.photometric, if index == 1 { 1 } else { 2 });
        assert_eq!(descriptor.samples_per_pixel, if index == 1 { 2 } else { 4 });
        assert_eq!(descriptor.extra_samples.values[0], 2);
        assert_eq!(descriptor.icc_bytes, if index == 1 { 0 } else { 588 });
        assert_eq!(descriptor.icc_sha256 == [0; 32], index == 1);
        for (n, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                prepared.document.layers[&prepared.document.roots[0]]
                    .raster
                    .get((n % 3) as i64, (n / 3) as i64)
                    .0,
                *pixel
            );
        }
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        for loss in losses {
            h.get_by_label(import_loss_description(loss).unwrap());
        }
        assert!(
            h.get_by_role_and_label(egui::Role::Button, "Open image")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label("Accept import limitations").click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, "Open image")
            .click();
        h.run();
        assert!(h.state().pending_import.is_none());
        let doc = h.state().editor.document();
        assert_eq!(doc.canvas, (3, 2));
        for (n, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                doc.layers[&doc.roots[0]]
                    .raster
                    .get((n % 3) as i64, (n / 3) as i64)
                    .0,
                *pixel
            );
        }
        assert_eq!(fs::read(&fixture.path).unwrap(), input);
    }
}
fn skinny(vertical: bool) -> Vec<u8> {
    // Extend the authored second uncompressed IFD, keeping all original offsets
    // and the first 1x1 page. Uniform opaque-color bytes are an independent display oracle.
    let mut bytes = fixture(false);
    let base = 8 + 154;
    let width = 65_537u32;
    let dimension = base + if vertical { 22 } else { 10 };
    bytes[dimension..dimension + 4].copy_from_slice(&width.to_le_bytes());
    if vertical {
        bytes[base + 94..base + 98].copy_from_slice(&width.to_le_bytes());
    }
    let strip = base + 146;
    bytes[base + 106..base + 110].copy_from_slice(&(width * 4).to_le_bytes());
    bytes.resize(strip + width as usize * 4, 0);
    for pixel in bytes[strip..].as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&SKINNY_PIXEL);
    }
    bytes
}
#[test]
fn skinny_page_above_old_65536_limit_has_a_visible_bounded_preview_and_prior_frame_consent() {
    for vertical in [false, true] {
        let mut fixture = FileFixture::new(false);
        fixture.bytes = skinny(vertical);
        fs::write(&fixture.path, &fixture.bytes).unwrap();
        let mut app = prepared(&fixture);
        let ctx = app.context.clone();
        let before = app.editor.document_snapshot();
        let Candidate::Tiff(source) = &mut app.pending_import.as_mut().unwrap().candidate else {
            panic!()
        };
        let prepared = source.prepared.as_ref().unwrap();
        let canvas = if vertical { (1, 65_537) } else { (65_537, 1) };
        assert_eq!(prepared.document.canvas, canvas);
        assert_eq!(
            prepared.thumbnail_size,
            if vertical { [1, 1024] } else { [1024, 1] }
        );
        assert!(
            prepared
                .thumbnail
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == SKINNY_PIXEL)
        );
        let review = source.review.as_mut().unwrap();
        review.consent_for_test();
        let mut rectangle = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_clip_rect(egui::Rect::NOTHING);
            rectangle = review.preview(ui);
            assert!(!review.ready(ui.ctx()));
        });
        output.textures_delta.clear();
        assert!(rectangle.height() >= 1.0);
        assert!(!review.ready(&ctx));
        let mut output = ctx.run_ui(Default::default(), |ui| {
            rectangle = review.preview(ui);
            assert!(
                !review.ready(ui.ctx()),
                "the first visible frame cannot authorize itself"
            );
            assert!(rectangle.height() >= 1.0 && rectangle.width() >= 1.0);
            assert!(rectangle.width() <= 200.0 && rectangle.height() <= 200.0);
            assert!(ui.is_rect_visible(rectangle));
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            review.preview(ui);
            assert!(review.ready(ui.ctx()));
        });
        output.textures_delta.clear();
        app.accept_tiff_import(&ctx);
        assert!(app.pending_import.is_none());
        assert!(!Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        let doc = app.editor.document();
        assert_eq!(doc.canvas, canvas);
        for n in [0, 32768, 65536] {
            let (x, y) = if vertical { (0, n) } else { (n, 0) };
            assert_eq!(doc.layers[&doc.roots[0]].raster.get(x, y).0, SKINNY_PIXEL);
        }
        assert_eq!(fs::read(&fixture.path).unwrap(), fixture.bytes);
    }
}

#[test]
fn late_tile_residency_denial_keeps_nonempty_document_history_and_source() {
    let mut fixture = FileFixture::new(false);
    fixture.bytes = skinny(false);
    fs::write(&fixture.path, &fixture.bytes).unwrap();
    let mut app = app();
    let ctx = app.context.clone();
    app.editor.request(
        91234,
        Action::ClearCanvas {
            color: Rgba([17, 18, 19, 255]),
        },
    );
    finish_edits(&mut app);
    let before = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    let dirty = app.editor.is_dirty();
    let edge = u64::from(paintcraft_raster::TILE_EDGE);
    let bytes = (65_537u64.div_ceil(edge) - 1) * edge * edge * 4;
    let residency = SharedBudget::new(bytes);
    app.editing = Some(
        controller::background::Service::with_residency(ctx.clone(), residency.clone()).unwrap(),
    );
    app.import_selected(fixture.path.clone());
    poll(&mut app, false);
    app.choose_tiff_page(&ctx, 1);
    let end = Instant::now() + Duration::from_secs(10);
    while source(&app).job.is_some() {
        app.poll_tiff_import(&ctx);
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(source(&app).prepared.is_none() && source(&app).review.is_none());
    assert!(
        source(&app)
            .error
            .as_ref()
            .is_some_and(|error| error.contains("resource.limit"))
    );
    assert_eq!(
        residency.usage().reserved,
        0,
        "private partial tiles are retired on admission failure"
    );
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(history, app.editor.history_view().ticket);
    assert_eq!(dirty, app.editor.is_dirty());
    app.resolve_import(&ctx, false);
    retire(&app);
    assert_eq!(fs::read(&fixture.path).unwrap(), fixture.bytes);
}
