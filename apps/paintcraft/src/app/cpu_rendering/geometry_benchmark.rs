//! Optimized process/statistical driver over actual desktop preview preparation.
use super::*;
use crate::{Editor, Tool};
use std::{hint::black_box, time::Duration};

fn replay(case: &str, cached: bool, verify: bool) -> u64 {
    let mode = std::env::var("PAINTCRAFT_GEOMETRY_GPU").ok();
    replay_with_gpu(case, cached, verify, mode.as_deref())
}
fn replay_with_gpu(case: &str, cached: bool, verify: bool, mode: Option<&str>) -> u64 {
    let mut editor = fixture();
    editor.select_tool(Tool::Brush);
    editor.options.size = 16;
    editor.color = paintcraft_core::types::Rgba([17, 88, 156, 128]);
    let original = verify.then(|| paintcraft_render::render_flat(editor.document()).unwrap());
    let gpu = gpu_fixture(mode);
    let mut uploaded_bytes = 0u64;
    let renderer = CpuRenderer::new();
    if std::env::var("PAINTCRAFT_GEOMETRY_COMPOSITOR").as_deref() == Ok("full") {
        *renderer.0.lock().unwrap() = None;
    }
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
        let pixels = if let Some(gpu) = &gpu {
            let frame = gpu.render_with_stats(&staged.doc, || Ok(())).unwrap();
            uploaded_bytes += frame.stats.uploaded_bytes;
            frame.pixels
        } else {
            renderer.render(&staged.doc, true, || Ok(())).unwrap()
        };
        if verify {
            let reference =
                paintcraft_editor::execute::execute(doc, editor.selection(), &batch).unwrap();
            assert_eq!(
                pixels,
                paintcraft_render::render_flat(&reference.doc).unwrap()
            );
        }
        // Consume the completed frame; all outputs and caches are destroyed in scope.
        digest = digest.wrapping_add(u64::from(pixels[(64 * 512 + 8) * 4]));
        black_box(&pixels);
    }
    if let Some(original) = original {
        assert_eq!(
            paintcraft_render::render_flat(editor.document()).unwrap(),
            original
        );
    }
    if gpu.is_some() {
        eprintln!("gpu_source_upload_bytes={uploaded_bytes}");
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
        Ok("gpu-criterion") => {
            use criterion::{BenchmarkId, Criterion, SamplingMode};
            let base = std::env::var("PAINTCRAFT_GEOMETRY_BASE").unwrap();
            let mut c = Criterion::default()
                .sample_size(20)
                .warm_up_time(Duration::from_millis(300))
                .measurement_time(Duration::from_secs(1))
                .without_plots();
            let mut group = c.benchmark_group(format!("gpu_sources/{base}"));
            group.sampling_mode(SamplingMode::Flat);
            for mode in ["streamed", "resident"] {
                group.bench_function(BenchmarkId::from_parameter(mode), |b| {
                    b.iter(|| black_box(replay_with_gpu(&case, true, false, Some(mode))))
                });
            }
            group.finish();
            c.final_summary();
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

pub(crate) fn fixture() -> Editor {
    let base = std::env::var("PAINTCRAFT_GEOMETRY_BASE").unwrap_or_else(|_| "sparse1".into());
    fixture_for_base(&base)
}
pub(crate) fn fixture_for_base(base: &str) -> Editor {
    if base == "sparse1" {
        return Editor::new_document(512, 256).unwrap();
    }
    assert!(matches!(
        base,
        "sparse5" | "dense5" | "dense20" | "dense65" | "dense100"
    ));
    use paintcraft_core::types::Rgba;
    use paintcraft_raster::{Admission, RasterLayer};
    let mut doc = paintcraft_document::new_document(
        512,
        256,
        Some(Rgba([255; 4])),
        42,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.clear();
    doc.roots.clear();
    let count = match base {
        "dense20" => 20u32,
        "dense65" => 65,
        "dense100" => 100,
        _ => 5,
    };
    let mut admission = Admission::new(u64::from(count) * 4 * 65536);
    for n in 0..count {
        let id =
            paintcraft_core::types::ids::deterministic_id(&["desktop-regional", &n.to_string()]);
        let default = if n == 0 {
            Rgba([255; 4])
        } else {
            Rgba([31, 63, 127, 0])
        };
        let raster = if base.starts_with("dense") {
            let pixels: Vec<_> = (0u32..512 * 256)
                .flat_map(|p| {
                    [
                        p as u8,
                        (p / 512) as u8,
                        (p.wrapping_mul(17) + n) as u8,
                        [0, 1, 64, 128, 254, 255][p as usize % 6],
                    ]
                })
                .collect();
            RasterLayer::from_rgba(512, 256, default, &pixels, &mut admission).unwrap()
        } else {
            RasterLayer::new(512, 256, default)
        };
        doc.roots.push(id);
        doc.layers.insert(
            id,
            paintcraft_document::Layer {
                id,
                name: format!("Layer {n}"),
                origin: (0, 0),
                visible: true,
                opacity: 173,
                raster,
            },
        );
    }
    Editor::with_document(doc, None)
}

fn gpu_fixture(mode: Option<&str>) -> Option<paintcraft_render::gpu::GpuRenderer> {
    use paintcraft_render::gpu::{DeviceEpoch, GpuRenderer, SourceCacheLimits};
    let mode = mode?;
    assert!(matches!(mode, "streamed" | "resident"));
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("GPU benchmark requires an adapter");
    eprintln!("GPU preview adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let epoch = DeviceEpoch::default();
    let lost = epoch.clone();
    device.set_device_lost_callback(move |_, _| lost.retire());
    let limits = if mode == "resident" {
        SourceCacheLimits::default()
    } else {
        SourceCacheLimits {
            buffer_bytes: 0,
            metadata_bytes: 0,
            entries: 0,
        }
    };
    Some(GpuRenderer::with_source_cache(device, queue, epoch, limits).unwrap())
}
