use super::*;
use paintcraft_core::types::text::TextOptions;

fn draft(e: &mut Editor) {
    e.tools.set_text_style(TextOptions {
        points: 22,
        ..Default::default()
    });
    e.tools.color = Rgba([30, 90, 180, 128]);
    e.tools.begin_text(e.target.unwrap(), (8, 8));
    e.tools.type_text("Paint café Ω");
}

#[test]
fn shaped_preview_copy_and_placement_match_without_publishing_the_draft() {
    let mut e = Editor::new(256, 128).unwrap();
    draft(&mut e);
    let ticket = e.session.ticket();
    let revision = e.tools.preview_revision();
    let mask = e
        .text_mask("Paint café Ω", e.tools.text_style.as_ref().unwrap(), (8, 8))
        .unwrap();
    let edge = mask
        .spans
        .iter()
        .find(|span| span.coverage > 0 && span.coverage < 255)
        .unwrap();
    let expected = Rgba([30, 90, 180, 128])
        .with_opacity(edge.coverage)
        .source_over(WHITE);
    let preview = e.display_document().unwrap().unwrap();
    assert_eq!(
        preview.layers[&e.target.unwrap()]
            .raster
            .get(edge.x0, edge.y),
        expected
    );
    let copy = e.copy_snapshot().unwrap();
    assert_eq!(
        paintcraft_render::render_flat(&preview).unwrap(),
        paintcraft_render::render_flat(&copy).unwrap()
    );
    assert_eq!(e.session.ticket(), ticket);
    assert_eq!(e.tools.preview_revision(), revision);
    assert_eq!(e.session.undo_depth(), 0);
    assert!(e.has_text() && e.is_dirty());
    let bytes = paintcraft_formats::pcnd::write(&copy).unwrap();
    let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
    assert!(reopened.same_semantics(&copy));
    e.place_text().unwrap();
    assert_eq!(e.session.document().state_id, copy.state_id);
    assert_eq!(
        paintcraft_render::render_flat(e.session.document()).unwrap(),
        paintcraft_render::render_flat(&copy).unwrap()
    );
    assert!(!e.tools.is_editing_text());
    assert_eq!(e.session.undo_depth(), 1);
    e.apply(Action::Undo).unwrap();
    assert!(!e.is_dirty());
    e.apply(Action::Redo).unwrap();
    assert_eq!(
        paintcraft_render::render_flat(e.session.document()).unwrap(),
        paintcraft_render::render_flat(&copy).unwrap()
    );
}

#[test]
fn denied_or_invalid_text_placement_retains_correctable_draft_and_history() {
    let mut e = Editor::new(128, 80).unwrap();
    draft(&mut e);
    let before = e.session.ticket();
    e.session.set_staged_pixel_budget(0);
    assert!(e.place_text().is_err());
    assert!(e.copy_snapshot().is_err());
    assert_eq!(e.tools.text_content(), Some("Paint café Ω"));
    assert_eq!(e.session.ticket(), before);
    assert_eq!(e.session.undo_depth(), 0);
    e.session.set_staged_pixel_budget(MAX_PIXELS * 4);
    let mut invalid = e.tools.text_style.clone().unwrap();
    invalid.points = 0;
    e.tools.set_text_style(invalid);
    assert!(e.place_text().is_err());
    assert_eq!(e.tools.text_content(), Some("Paint café Ω"));
    e.tools.set_text_style(TextOptions::default());
    e.place_text().unwrap();
    assert_eq!(e.session.undo_depth(), 1);
}

#[test]
fn an_exported_copy_stays_frozen_while_the_draft_changes() {
    let mut e = Editor::new(256, 128).unwrap();
    draft(&mut e);
    let first = e.copy_snapshot().unwrap();
    let pixels = paintcraft_render::render_flat(&first).unwrap();
    e.tools.type_text("! More");
    let second = e.copy_snapshot().unwrap();
    assert_ne!(first.state_id, second.state_id);
    assert_ne!(pixels, paintcraft_render::render_flat(&second).unwrap());
    assert_eq!(paintcraft_render::render_flat(&first).unwrap(), pixels);
    assert_eq!(e.session.undo_depth(), 0);
}

#[test]
fn selected_text_background_and_grayscale_coverage_share_the_same_alpha_rules() {
    let mut e = Editor::new(256, 128).unwrap();
    e.apply(Action::Select {
        corners: Some(((12, 10), (170, 70))),
    })
    .unwrap();
    draft(&mut e);
    let mut style = e.tools.text_style.clone().unwrap();
    style.background_fill = true;
    e.tools.secondary = Rgba([90, 50, 10, 96]);
    e.tools.set_text_style(style.clone());
    let mask = e.text_mask("Paint café Ω", &style, (8, 8)).unwrap();
    let edge = mask
        .spans
        .iter()
        .find(|span| {
            span.coverage > 0
                && span.coverage < 255
                && span.y >= 10
                && span.y <= 70
                && span.x1 >= 12
                && span.x0 <= 170
        })
        .unwrap();
    let x = edge.x0.max(12);
    let y = edge.y;
    let expected = Rgba([30, 90, 180, 128])
        .with_opacity(edge.coverage)
        .source_over(Rgba([90, 50, 10, 96]).source_over(WHITE));
    let copy = e.copy_snapshot().unwrap();
    assert_eq!(copy.layers[&e.target.unwrap()].raster.get(x, y), expected);
    assert_eq!(copy.layers[&e.target.unwrap()].raster.get(0, 0), WHITE);
    let before = e.session.ticket();
    e.place_text().unwrap();
    assert_ne!(e.session.ticket(), before);
    assert_eq!(
        paintcraft_render::render_flat(e.session.document()).unwrap(),
        paintcraft_render::render_flat(&copy).unwrap()
    );
}
