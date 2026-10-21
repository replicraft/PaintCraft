//! Optimized process/statistical driver over actual desktop preview preparation.
use super::*;
use crate::{Editor, Tool};
use std::{hint::black_box, time::Duration};

fn replay(case: &str, cached: bool, verify: bool) -> u64 {
    let mut editor = Editor::new_document(512, 256).unwrap();
    editor.select_tool(Tool::Brush);
    editor.options.size = 16;
    editor.color = paintcraft_core::types::Rgba([17, 88, 156, 128]);
    let renderer = CpuRenderer::new();
    editor.pointer_down((8, 64));
    let (frames, step): (i64, i64) = if case == "tiny" { (4, 1) } else { (32, 16) };
    let mut digest = 0u64;
    for frame in 0..frames {
        for i in frame * step..(frame + 1) * step {
            let (x, y) = match case {
                "tiny" | "growing" => (8 + i, 64 + i % 32),
                "retrace" => (8 + (i % 64 - 32).abs(), 64 + (i % 32 - 16).abs()),
                _ => panic!("unknown geometry fixture"),
            };
            editor.pointer_move((x, y));
        }
        let batch = editor.preview_batch().unwrap().unwrap();
        let doc = editor.document();
        let staged = if cached {
            renderer
                .stage(doc, editor.selection(), &batch, &|| Ok(()))
                .unwrap()
        } else {
            paintcraft_editor::execute::execute_with_admission_checked(
                doc,
                editor.selection(),
                &batch,
                paintcraft_raster::Admission::new(
                    paintcraft_core::types::policy::STAGED_PIXELS_MAX,
                ),
                &|| Ok(()),
            )
            .unwrap()
        };
        let pixels = renderer.render(&staged.doc, true, || Ok(())).unwrap();
        if verify {
            let reference =
                paintcraft_editor::execute::execute(doc, editor.selection(), &batch).unwrap();
            assert_eq!(
                pixels,
                paintcraft_render::render_flat(&reference.doc).unwrap()
            );
            assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
        }
        // Consume the completed frame; all outputs and caches are destroyed in scope.
        digest = digest.wrapping_add(u64::from(pixels[(64 * 512 + 8) * 4]));
        black_box(&pixels);
    }
    black_box(digest)
}

#[test]
#[ignore = "optimized geometry replay driver; use recorded process/statistical configs"]
fn preview_geometry_process() {
    let case = std::env::var("PAINTCRAFT_GEOMETRY_CASE").unwrap_or_else(|_| "growing".into());
    match std::env::var("PAINTCRAFT_GEOMETRY_VARIANT").as_deref() {
        Ok("verify") => {
            for case in ["tiny", "growing", "retrace"] {
                assert_eq!(replay(case, true, true), replay(case, false, true));
            }
        }
        Ok("calibration") => {
            black_box(0u64);
        }
        Ok("cached") => {
            black_box(replay(&case, true, false));
        }
        Ok("baseline") => {
            black_box(replay(&case, false, false));
        }
        Ok("criterion") => {
            use criterion::{BenchmarkId, Criterion};
            let mut c = Criterion::default()
                .sample_size(20)
                .warm_up_time(Duration::from_millis(300))
                .measurement_time(Duration::from_secs(1))
                .without_plots();
            for case in ["tiny", "growing", "retrace"] {
                let mut group = c.benchmark_group(format!("preview_geometry/{case}"));
                for cached in [false, true] {
                    group.bench_function(
                        BenchmarkId::from_parameter(if cached { "cached" } else { "baseline" }),
                        |b| b.iter(|| black_box(replay(case, cached, false))),
                    );
                }
                group.finish();
            }
            c.final_summary();
        }
        _ => panic!("specify PAINTCRAFT_GEOMETRY_VARIANT"),
    }
}
