use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    render_flat,
};

fn cache(edge: u32, budget: usize) -> CompositeCache {
    CompositeCache::new(CacheLimits {
        tile_edge: edge,
        committed_bytes: budget,
        preview_bytes: budget,
        entries_per_domain: 512,
    })
    .unwrap()
}
fn run(s: &mut Session, ops: &str) {
    let t = s.ticket();
    s.apply_json(format!(r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,t.state_id,t.publication_epoch).as_bytes()).unwrap();
}
fn stroke(s: &mut Session, x: u32, y: u32) {
    let id = s.document().roots.last().unwrap();
    run(
        s,
        &format!(
            r##"[{{"op":"raster.pencil_path","layer_ref":{{"id":"{id}"}},"points":[{{"x":{x},"y":{y}}}],"color":"#17589CCC","mode":"source-over"}}]"##
        ),
    );
}

#[test]
fn local_edit_recomposes_one_storage_tile_while_reusing_the_rest_and_restores_undo() {
    for edge in [64, 128, 256] {
        let mut s =
            Session::new(new_document(1024, 1024, Some(Rgba([255; 4])), 1, POLICY).unwrap());
        run(
            &mut s,
            r##"[{"op":"layer.add","ref":"ink","name":"Ink","width":1024,"height":1024,"origin":{"x":0,"y":0},"default_pixel":"#79641300"}]"##,
        );
        let mut cache = cache(edge, 16 << 20);
        let region = s.document().canvas_rect();
        let before = s.snapshot();
        cache
            .render(&before, region, Domain::Committed, || Ok(()))
            .unwrap();
        stroke(&mut s, 37, 40);
        let image = cache
            .render(s.document(), region, Domain::Committed, || Ok(()))
            .unwrap();
        assert_eq!(image.pixels, render_flat(s.document()).unwrap());
        assert_eq!(image.stats.recomposed_pixels, 65536);
        assert_eq!(image.stats.prefix_hits, image.stats.recomposed_tiles);
        assert_eq!(image.stats.recomposed_tiles, u64::from(256 / edge).pow(2));
        assert_eq!(
            image.stats.cache_hits,
            u64::from(1024 / edge).pow(2) - image.stats.recomposed_tiles
        );
        let hit = cache
            .render(s.document(), region, Domain::Committed, || Ok(()))
            .unwrap();
        assert_eq!(hit.stats.recomposed_pixels, 0);
        s.undo().unwrap().unwrap();
        let undone = cache
            .render(s.document(), region, Domain::Committed, || Ok(()))
            .unwrap();
        assert_eq!(undone.pixels, render_flat(&before).unwrap());
        s.redo().unwrap().unwrap();
        assert_eq!(
            cache
                .render(s.document(), region, Domain::Committed, || Ok(()))
                .unwrap()
                .pixels,
            image.pixels
        );
    }
}

#[test]
fn structural_properties_offsets_resize_and_cross_document_identity_never_reuse_stale_pixels() {
    let mut s =
        Session::new(new_document(259, 257, Some(Rgba([47, 38, 29, 255])), 3, POLICY).unwrap());
    run(
        &mut s,
        r##"[{"op":"layer.add","ref":"top","name":"Top","width":260,"height":260,"origin":{"x":-5,"y":3},"default_pixel":"#FF000080"}]"##,
    );
    let top = s.document().roots[1];
    let mut cache = cache(64, 2 << 20);
    let operations=[format!(r#"[{{"op":"layer.set_properties","layer_ref":{{"id":"{top}"}},"opacity_u8":128}}]"#),
        format!(r#"[{{"op":"layer.set_properties","layer_ref":{{"id":"{top}"}},"visible":false}}]"#),
        format!(r#"[{{"op":"layer.set_properties","layer_ref":{{"id":"{top}"}},"visible":true}}]"#),
        format!(r#"[{{"op":"layer.reorder","layer_ref":{{"id":"{top}"}},"to_index":0}}]"#),
        r#"[{"op":"canvas.resize","width":123,"height":119,"anchor":"center","outside_content":"preserve"}]"#.to_owned(),
        format!(r#"[{{"op":"layer.delete","layer_ref":{{"id":"{top}"}}}}]"#)];
    for op in operations {
        assert_eq!(
            cache
                .render(
                    s.document(),
                    s.document().canvas_rect(),
                    Domain::Committed,
                    || Ok(())
                )
                .unwrap()
                .pixels,
            render_flat(s.document()).unwrap()
        );
        run(&mut s, &op);
        assert_eq!(
            cache
                .render(
                    s.document(),
                    s.document().canvas_rect(),
                    Domain::Committed,
                    || Ok(())
                )
                .unwrap()
                .pixels,
            render_flat(s.document()).unwrap()
        );
    }
    let other = new_document(123, 119, Some(Rgba([23, 46, 79, 255])), 9, POLICY).unwrap();
    assert_eq!(
        cache
            .render(&other, other.canvas_rect(), Domain::Committed, || Ok(()))
            .unwrap()
            .pixels,
        [23, 46, 79, 255].repeat(123 * 119)
    );
}

#[test]
fn previews_and_cancellation_preserve_the_committed_cache_and_never_return_partial_frames() {
    let mut s = Session::new(new_document(512, 512, Some(Rgba([255; 4])), 4, POLICY).unwrap());
    let original = s.snapshot();
    let mut cache = cache(128, 4 << 20);
    cache
        .render(&original, original.canvas_rect(), Domain::Committed, || {
            Ok(())
        })
        .unwrap();
    let usage = cache.usage(Domain::Committed);
    stroke(&mut s, 12, 15);
    let preview = cache
        .render(
            s.document(),
            s.document().canvas_rect(),
            Domain::Preview,
            || Ok(()),
        )
        .unwrap();
    assert_eq!(preview.stats.recomposed_pixels, 65536);
    assert_eq!(cache.usage(Domain::Committed), usage);
    cache.clear_preview();
    assert_eq!(cache.usage(Domain::Preview).pixel_bytes, 0);
    let checks = std::cell::Cell::new(0);
    let error = cache
        .render(
            s.document(),
            s.document().canvas_rect(),
            Domain::Preview,
            || {
                checks.set(checks.get() + 1);
                if checks.get() > 8 {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(cache.usage(Domain::Committed), usage);
    let committed = cache
        .render(&original, original.canvas_rect(), Domain::Committed, || {
            Ok(())
        })
        .unwrap();
    assert_eq!(committed.stats.recomposed_pixels, 0);
    assert_eq!(committed.pixels, vec![255; 512 * 512 * 4]);
}

#[test]
fn tiny_budgets_eviction_and_partial_viewports_remain_bounded_and_exact() {
    let mut s =
        Session::new(new_document(1024, 1024, Some(Rgba([8, 9, 10, 255])), 5, POLICY).unwrap());
    for budget in [0, 1, 4096, 300_000] {
        let mut cache = cache(64, budget);
        for n in 0..12 {
            stroke(&mut s, n * 7, n * 13);
            let region = Rect::new(n as i64 * 11, n as i64 * 17, 117, 93).unwrap();
            let output = cache
                .render(s.document(), region, Domain::Committed, || Ok(()))
                .unwrap();
            assert_eq!(
                output.pixels,
                paintcraft_render::render_region(s.document(), region).unwrap()
            );
            assert!(cache.usage(Domain::Committed).weight() <= budget);
        }
        cache.clear();
        assert_eq!(cache.usage(Domain::Committed).pixel_bytes, 0);
    }
    assert!(
        CompositeCache::new(CacheLimits {
            tile_edge: 0,
            committed_bytes: 1,
            preview_bytes: 1,
            entries_per_domain: 1
        })
        .is_err()
    );
}

#[test]
fn changed_lower_layers_invalidate_prefixes_and_empty_plans_do_no_composition_work() {
    let mut s =
        Session::new(new_document(65, 63, Some(Rgba([11, 81, 143, 91])), 7, POLICY).unwrap());
    run(
        &mut s,
        r##"[{"op":"layer.add","ref":"middle","name":"Middle","width":65,"height":63,"origin":{"x":0,"y":0},"default_pixel":"#9A431D57"},{"op":"layer.add","ref":"top","name":"Top","width":65,"height":63,"origin":{"x":0,"y":0},"default_pixel":"#2C61E38B"}]"##,
    );
    let region = s.document().canvas_rect();
    let mut cache = cache(128, 1 << 20);
    let render = cache
        .render(s.document(), region, Domain::Committed, || Ok(()))
        .unwrap();
    let expected = Rgba([44, 97, 227, 139])
        .source_over(Rgba([154, 67, 29, 87]).source_over(Rgba([11, 81, 143, 91])));
    assert_eq!(render.pixels, expected.0.repeat(65 * 63));
    let bottom = s.document().roots[0];
    run(
        &mut s,
        &format!(
            r#"[{{"op":"layer.set_properties","layer_ref":{{"id":"{bottom}"}},"opacity_u8":73}}]"#
        ),
    );
    let changed = cache
        .render(s.document(), region, Domain::Committed, || Ok(()))
        .unwrap();
    assert_eq!(changed.stats.prefix_hits, 0);
    assert_eq!(changed.pixels, render_flat(s.document()).unwrap());
    stroke(&mut s, 2, 3);
    let changed = cache
        .render(s.document(), region, Domain::Committed, || Ok(()))
        .unwrap();
    assert_eq!(changed.stats.prefix_hits, 1);
    assert_eq!(changed.pixels, render_flat(s.document()).unwrap());
    let ops = s
        .document()
        .roots
        .iter()
        .map(|id| {
            format!(
                r#"{{"op":"layer.set_properties","layer_ref":{{"id":"{id}"}},"visible":false}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    run(&mut s, &format!("[{ops}]"));
    let empty = cache
        .render(s.document(), region, Domain::Committed, || Ok(()))
        .unwrap();
    assert_eq!(empty.pixels, vec![0; 65 * 63 * 4]);
    assert_eq!(empty.stats.recomposed_pixels, 0);
}
