//! Prebuilt optimized process driver, excluded from ordinary tests and shipping code.
use super::*;
use std::{hint::black_box, time::Instant};

fn fixture(case: &str) -> (Editor, usize, usize) {
    let mut editor = Editor::new_document(512, 512).unwrap();
    let (points, repeats, paths) = match case {
        "short" => (32, 100_000, 1),
        "long" => (65_536, 512, 1),
        "queue" => (8_192, 128, 31),
        "changing" => (65_536, 512, 1),
        _ => panic!("unknown preview benchmark case"),
    };
    if paths > 1 {
        editor.connect_editing(background::Service::new(eframe::egui::Context::default()).unwrap());
    }
    for _ in 0..paths {
        editor.pointer_down((0, 0));
        for i in 1..points {
            editor.pointer_move(((i % 512) as i64, (i / 512) as i64));
        }
        if paths > 1 {
            editor.pointer_up();
        }
    }
    (editor, points, repeats)
}
fn next(editor: &Editor, current: &mut Option<Arc<Preview>>, cached: bool) {
    let editor = black_box(editor);
    if cached {
        let candidate = editor.preview_snapshot();
        let same = match (&current, &candidate) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        // Same check as native sync_texture, including changed-input comparison.
        black_box(same || *current == candidate);
        *current = candidate;
    } else {
        // Frozen pre-a21d733 collect/compare behavior, without using the new cache.
        let candidate = editor.collect_preview();
        if current.as_deref() != candidate.as_ref() {
            *current = candidate.map(Arc::new);
        }
    }
    black_box(&current);
}
#[test]
#[ignore = "optimized preview benchmark driver; run through recorded Hyperfine configs"]
fn preview_capture_process() {
    let case = std::env::var("PAINTCRAFT_PREVIEW_CASE").unwrap();
    let variant = std::env::var("PAINTCRAFT_PREVIEW_VARIANT").unwrap();
    let cached = match variant.as_str() {
        "baseline" => false,
        "cached" => true,
        "verify" => {
            verify(&case);
            return;
        }
        _ => panic!("unknown preview variant"),
    };
    let start = Instant::now();
    let (mut editor, points, repeats) = fixture(&case);
    let mut displayed = None;
    next(&editor, &mut displayed, cached);
    for i in 0..repeats {
        if case == "changing" {
            editor.pointer_move(((points + i) as i64, 1));
        }
        next(&editor, &mut displayed, cached);
    }
    // Include snapshot/gesture/worker-pool destruction in both process variants.
    drop(displayed);
    drop(editor);
    eprintln!(
        "{case} {variant} repeats={repeats} complete_ns={}",
        start.elapsed().as_nanos()
    );
}
fn verify(case: &str) {
    let (mut editor, points, _) = fixture(case);
    let mut cached = None;
    let mut baseline = None;
    for i in 0..16 {
        next(&editor, &mut cached, true);
        next(&editor, &mut baseline, false);
        assert_eq!(cached, baseline);
        let snapshot = cached.clone().unwrap();
        assert!(Arc::ptr_eq(&snapshot, &editor.preview_snapshot().unwrap()));
        if case == "changing" {
            editor.pointer_move(((points + i) as i64, 1));
        }
    }
    // Independent checks on the generated workload, not only equality of helpers.
    match cached.unwrap().as_ref() {
        Preview::Path(path) => {
            assert_eq!(path.points[0], (0, 0));
            assert_eq!(
                path.points.len(),
                points + if case == "changing" { 15 } else { 0 }
            );
        }
        Preview::Paths(paths) => {
            assert_eq!(paths.len(), 31);
            assert!(paths.iter().all(|path| path.points.len() == points));
        }
        _ => panic!("fixture must contain paint paths"),
    }
    editor.cancel_gesture();
    editor.cancel_pending_edits();
    assert!(editor.preview_snapshot().is_none());
}
