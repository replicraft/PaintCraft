use paintcraft_core::types::{Error, ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_history::Packing;
use paintcraft_raster::RasterLayer;

fn session() -> Session {
    Session::new(new_document(257, 259, Some(Rgba([37, 19, 83, 0])), 42, POLICY).unwrap())
}
fn paint(s: &mut Session, n: u32) {
    let t = s.ticket();
    let id = s.document().roots[0];
    let op = format!(
        r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.pencil_path","layer_ref":{{"id":"{id}"}},"points":[{{"x":{},"y":{}}}],"color":"#17589CCC","mode":"replace"}}]}}"##,
        t.state_id,
        t.publication_epoch,
        3 + n % 120,
        5 + n / 120
    );
    s.apply_json(op.as_bytes()).unwrap();
}
fn verify(s: &Session, count: u32) {
    let raster = &s.document().layers[&s.document().roots[0]].raster;
    for n in 0..40 {
        assert_eq!(
            raster.get(3 + n % 120, 5 + n / 120),
            if n < i64::from(count) {
                Rgba([23, 88, 156, 204])
            } else {
                Rgba([37, 19, 83, 0])
            },
            "sample {n}, count {count}"
        );
    }
    assert_eq!(raster.get(256, 258), Rgba([37, 19, 83, 0]));
}

#[test]
fn cold_records_release_actual_decoded_sources_and_keep_held_readers_visible() {
    let mut s = session();
    paint(&mut s, 0);
    let held = s.snapshot();
    let source = held.layers[&held.roots[0]]
        .raster
        .tile_versions(held.layers[&held.roots[0]].raster.bounds())
        .unwrap()
        .next()
        .unwrap()
        .1;
    let mut states = vec![s.ticket().state_id];
    for n in 1..40 {
        paint(&mut s, n);
        states.push(s.ticket().state_id);
    }
    assert_eq!(s.resident_bytes(), 40 * RasterLayer::TILE_BYTES as u64);
    let report = s.compact_history(1, 4 << 20, || Ok(())).unwrap();
    assert_eq!(report.packed_entries, 40);
    assert_eq!(report.decoded_after, 2 * RasterLayer::TILE_BYTES as u64);
    assert_eq!(
        s.history_memory().leased_decoded_bytes,
        RasterLayer::TILE_BYTES as u64
    );
    assert!(s.history_memory().encoded_bytes < 8192);
    drop(held);
    assert_eq!(
        source.resident_bytes(),
        0,
        "weak keys must not retain the original decoded Vec"
    );
    assert_eq!(s.resident_bytes(), RasterLayer::TILE_BYTES as u64);
    for count in (1..40).rev() {
        assert_eq!(
            s.undo().unwrap().unwrap().state_id,
            states[count as usize - 1]
        );
        verify(&s, count);
    }
    s.undo().unwrap().unwrap();
    verify(&s, 0);
    assert!(s.undo().unwrap().is_none());
    for count in 1..=40 {
        s.redo().unwrap().unwrap();
        verify(&s, count);
    }
    assert!(s.redo().unwrap().is_none());
    assert_eq!(s.ticket().state_id, *states.last().unwrap());
}

#[test]
fn failed_restore_or_cancelled_compaction_never_moves_publication_or_history_cursor() {
    let mut s = session();
    for n in 0..8 {
        paint(&mut s, n);
    }
    let before = s.ticket();
    let view = s.history_view().entries;
    assert_eq!(
        s.compact_history(1, 0, || Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        s.compact_history(1, 4 << 20, || Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancelled compaction"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(s.ticket(), before);
    assert_eq!(s.history_view().entries, view);
    verify(&s, 8);
    s.compact_history(1, 4 << 20, || Ok(())).unwrap();
    s.set_history_restore_budget(0);
    assert_eq!(s.undo().unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(s.ticket(), before);
    assert_eq!(s.history_view().entries, view);
    verify(&s, 8);
    s.set_history_restore_budget(65536);
    assert!(s.undo().unwrap().is_some());
    verify(&s, 7);
    // A fresh history ticket can seek without replaying commands or changing IDs.
    let view = s.history_view();
    let initial = view.entries[0].history_id;
    s.navigate_history(view.ticket, initial).unwrap().unwrap();
    verify(&s, 0);
}

#[test]
fn adaptive_pressure_retains_more_exact_history_and_shared_policy_keeps_explicit_trimming() {
    for packing in [Packing::Shared, Packing::Adaptive] {
        let mut s = session();
        s.set_history_packing(packing);
        s.set_history_budget((4 * RasterLayer::TILE_BYTES) as u64);
        for n in 0..40 {
            paint(&mut s, n);
        }
        if packing == Packing::Adaptive {
            assert_eq!(s.undo_depth(), 40);
            assert_eq!(s.trimmed_total(), 0);
            assert!(s.history_memory().packed_entries > 0);
            assert!(s.history_memory().retained_weight() < 4 * RasterLayer::TILE_BYTES as u64);
        } else {
            assert!(s.trimmed_total() > 0);
            assert!(s.undo_depth() < 40);
        }
        let depth = s.undo_depth();
        for n in 0..depth {
            s.undo().unwrap().unwrap();
            verify(&s, 39 - n as u32);
        }
    }
}

fn run(s: &mut Session, ops: &str) {
    let t = s.ticket();
    let json = format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,
        t.state_id, t.publication_epoch
    );
    s.apply_json(json.as_bytes()).unwrap();
}

#[test]
fn cold_navigation_restores_metadata_order_selection_and_off_canvas_hidden_rgb() {
    use paintcraft_raster::{Admission, BlendMode, Selection};
    let mut doc = new_document(9, 7, Some(Rgba([255; 4])), 7, POLICY).unwrap();
    doc.title = Some("Retained title".to_owned());
    let mut ink = doc.layers[&doc.roots[0]].clone();
    ink.id = paintcraft_core::types::ids::deterministic_id(&["cold-off-canvas"]);
    ink.name = "Off canvas".to_owned();
    ink.origin = (-4, 2);
    ink.opacity = 73;
    ink.raster = RasterLayer::new(17, 13, Rgba([17, 34, 51, 0]));
    ink.raster
        .blend_pixel(
            16,
            12,
            Rgba([31, 47, 89, 113]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let id = ink.id;
    doc.roots.push(id);
    doc.layers.insert(id, ink);
    let roots = doc.roots.clone();
    let mut s = Session::new(doc);
    run(
        &mut s,
        r#"[{"op":"selection.set_rect","rect":{"x":1,"y":1,"width":3,"height":2}}]"#,
    );
    let selected = s.selection();
    run(
        &mut s,
        &format!(
            r#"[{{"op":"layer.set_properties","layer_ref":{{"id":"{id}"}},"name":"Renamed","visible":false,"opacity_u8":128}}]"#
        ),
    );
    run(
        &mut s,
        r#"[{"op":"canvas.resize","width":11,"height":8,"anchor":"center","outside_content":"preserve"}]"#,
    );
    let ids = s.history_view().entries;
    s.compact_history(1, 2 << 20, || Ok(())).unwrap();
    assert_eq!(ids, s.history_view().entries);
    s.undo().unwrap().unwrap();
    assert_eq!(s.document().canvas, (9, 7));
    assert_eq!(s.selection(), selected);
    assert_eq!(s.document().layers[&id].name, "Renamed");
    assert!(!s.document().layers[&id].visible);
    s.undo().unwrap().unwrap();
    assert_eq!(s.selection(), selected);
    let ink = &s.document().layers[&id];
    assert_eq!(ink.name, "Off canvas");
    assert!(ink.visible);
    assert_eq!(ink.opacity, 73);
    assert_eq!(ink.origin, (-4, 2));
    assert_eq!(ink.raster.get(16, 12), Rgba([31, 47, 89, 113]));
    assert_eq!(ink.raster.get(0, 0), Rgba([17, 34, 51, 0]));
    assert_eq!(s.document().roots, roots);
    assert_eq!(s.document().title.as_deref(), Some("Retained title"));
    s.undo().unwrap().unwrap();
    assert_eq!(s.selection(), Selection::Inactive);
}

#[test]
fn full_extent_patterns_keep_shared_raw_payloads_without_claiming_memory_reclamation() {
    let bytes: Vec<u8> = (0..65536u32)
        .flat_map(|n| [n as u8, (n >> 8) as u8, (n.wrapping_mul(19)) as u8, 255])
        .collect();
    let doc = paintcraft_document::import_image(256, 256, &bytes, "history-raw-fixture").unwrap();
    let mut s = Session::new(doc);
    paint(&mut s, 0);
    let before = s.resident_bytes();
    let report = s.compact_history(1, 2 << 20, || Ok(())).unwrap();
    assert_eq!(report.decoded_after, before);
    assert_eq!(report.encoded_bytes, 0);
    s.set_history_restore_budget(0);
    s.undo().unwrap().unwrap();
    let raster = &s.document().layers[&s.document().roots[0]].raster;
    for (x, y) in [(0, 0), (3, 5), (255, 255)] {
        let i = (y * 256 + x) as usize * 4;
        assert_eq!(raster.get(x, y).0, &bytes[i..i + 4]);
    }
}

#[test]
fn automatic_pressure_packs_a_bounded_number_of_complete_old_states() {
    let mut s = session();
    s.set_history_budget((100 * RasterLayer::TILE_BYTES) as u64);
    for n in 0..150 {
        paint(&mut s, n);
        let memory = s.history_memory();
        if memory.packed_entries > 0 {
            assert!(memory.packed_entries <= 64);
            assert!(memory.logical_payloads <= 64);
            assert_eq!(s.trimmed_total(), 0);
            assert_eq!(s.undo_depth(), n as usize + 1);
            return;
        }
    }
    panic!("expected automatic pressure compaction");
}

#[test]
fn explicit_retention_trim_reclaims_grown_reference_tables_as_well_as_payloads() {
    let mut s = session();
    s.set_history_packing(Packing::Shared);
    for n in 0..80 {
        paint(&mut s, n);
    }
    let before = s.history_memory();
    s.set_history_budget(0);
    paint(&mut s, 80);
    let after = s.history_memory();
    assert_eq!(s.undo_depth(), 1);
    assert_eq!(after.decoded_bytes, 2 * RasterLayer::TILE_BYTES as u64);
    assert!(after.metadata_weight < before.metadata_weight / 2);
}

#[test]
fn branching_from_a_cold_state_keeps_the_previous_published_state_hot_and_undoable() {
    let mut s = session();
    for n in 0..10 {
        paint(&mut s, n);
    }
    s.compact_history(1, 2 << 20, || Ok(())).unwrap();
    for _ in 0..5 {
        s.undo().unwrap().unwrap();
    }
    let previous = s.ticket().state_id;
    s.set_history_restore_budget(0);
    paint(&mut s, 20);
    assert_eq!(s.redo_depth(), 0);
    assert_eq!(s.undo().unwrap().unwrap().state_id, previous);
    verify(&s, 5);
}

#[test]
fn compressed_recorded_timelines_release_tiles_and_keep_navigation_atomic() {
    use paintcraft_history::Compression;
    let initial: Vec<u8> = (0..65536)
        .flat_map(|i| {
            let (x, y) = ((i % 256) as u8, (i / 256) as u8);
            [x / 8, y / 8, (x / 16) ^ (y / 16), (x % 4) * 85]
        })
        .collect();
    for codec in [Compression::Lz4, Compression::Zstd1, Compression::Zstd3] {
        let doc =
            paintcraft_document::import_image(256, 256, &initial, "codec-history-fixture").unwrap();
        let mut s = Session::new(doc);
        s.set_history_packing(Packing::Shared);
        s.set_history_compression(codec);
        let mut states = vec![s.ticket().state_id];
        for n in 0..10 {
            paint(&mut s, n);
            states.push(s.ticket().state_id);
        }
        let ticket = s.ticket();
        assert_eq!(
            s.compact_history(1, 1024, || Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(s.ticket(), ticket);
        assert_eq!(s.undo_depth(), 10);
        let stats = s.compact_history(1, 16 << 20, || Ok(())).unwrap();
        assert!(stats.codec_scratch_bytes > 0);
        assert!(stats.codec_workspace_peak > 0);
        assert_eq!(
            s.history_memory().decoded_bytes,
            RasterLayer::TILE_BYTES as u64
        );
        assert_eq!(s.history_memory().compressed_payloads, 10);
        assert!(s.history_memory().compressed_bytes > 0);
        s.set_history_restore_budget(0);
        assert_eq!(s.undo().unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(s.ticket(), ticket);
        assert_eq!(s.undo_depth(), 10);
        s.set_history_restore_budget(65536);
        let verify = |s: &Session, count: usize| {
            assert_eq!(s.ticket().state_id, states[count]);
            let mut expected = initial.clone();
            for n in 0..count {
                let offset = ((5 + n / 120) * 256 + 3 + n % 120) * 4;
                expected[offset..offset + 4].copy_from_slice(&[23, 88, 156, 204]);
            }
            let doc = s.document();
            let tile = doc.layers[&doc.roots[0]]
                .raster
                .tile_leases()
                .next()
                .unwrap()
                .1;
            assert_eq!(tile.bytes(), expected);
        };
        for count in (0..10).rev() {
            s.undo().unwrap().unwrap();
            verify(&s, count);
        }
        for count in 1..=10 {
            s.redo().unwrap().unwrap();
            verify(&s, count);
        }
    }
}
