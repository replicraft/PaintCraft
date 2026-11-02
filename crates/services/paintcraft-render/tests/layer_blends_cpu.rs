//! Literal-first serial CPU qualification; no GPU/browser blend claim.
use paintcraft_core::types::{
    ErrorCode, LayerBlendMode as Mode, Rect, Rgba,
    ids::deterministic_id,
    json::{self, Value},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack, LayerMask, POLICY};
use paintcraft_raster::{RasterLayer, coverage_mask::A8Mask};
use paintcraft_render::{
    blend, render_flat,
    spatial::{WorkspaceAdmission, with_workspace},
};
fn workspace(limit: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(
        SharedBudget::new(32 << 20).scope(AllocationClass::SharedBudget),
        16 << 20,
        limit,
    )
}
fn scene(back: Rgba, front: Rgba, mode: Mode, width: u32) -> Document {
    let mut d = paintcraft_document::new_document(width, 1, Some(back), 0, POLICY).unwrap();
    let mut l = d.layers[&d.roots[0]].clone();
    l.id = deterministic_id(&["cpu-blend-front"]);
    l.name = "Front".into();
    l.blend_mode = mode;
    l.raster = RasterLayer::new(width, 1, front);
    d.roots.push(l.id);
    d.layers.insert(l.id, l);
    d
}
fn field(d: &mut Document, area: Rect) {
    let id = d.roots[1];
    d.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(
        true,
        A8Mask::uniform(
            area,
            128,
            &SharedBudget::new(8192),
            Default::default(),
            &|| Ok(()),
        )
        .unwrap(),
    ));
    d.layers.get_mut(&id).unwrap().opacity = 128;
}
fn render(d: &Document) -> Vec<u8> {
    with_workspace(workspace(1_000_000), || render_flat(d)).unwrap()
}
#[test]
fn opaque_two_stage_alpha_order_and_general_backdrop_are_independent_literals() {
    let mut d = scene(
        Rgba([255, 255, 255, 255]),
        Rgba([204, 77, 13, 128]),
        Mode::Multiply,
        2,
    );
    field(&mut d, Rect::new(0, 0, 2, 1).unwrap());
    assert_eq!(render(&d), [249, 233, 225, 255].repeat(2));
    let mut l = d.layers[&d.roots[1]].clone();
    l.id = deterministic_id(&["cpu-blend-second"]);
    l.blend_mode = Mode::Screen;
    l.raster = RasterLayer::new(2, 1, Rgba([40, 190, 230, 128]));
    d.roots.push(l.id);
    d.layers.insert(l.id, l);
    assert_eq!(render(&d), [249, 235, 228, 255].repeat(2));
    let q = blend::quote_document(&d, d.canvas_rect()).unwrap();
    assert_eq!(
        (q.opaque_samples, q.general_samples, q.extra_work),
        (4, 0, 128)
    );
    let d = scene(
        Rgba([10, 240, 80, 128]),
        Rgba([200, 30, 180, 128]),
        Mode::Multiply,
        1,
    );
    assert_eq!(render(&d), [72, 99, 105, 192]);
    assert_eq!(
        blend::quote_document(&d, d.canvas_rect())
            .unwrap()
            .general_samples,
        1
    );
}
fn rgba(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as u8)
        .collect()
}
#[test]
fn full_box_prefix_precedes_mask_opacity_blend_and_native_lod() {
    use paintcraft_core::types::filters::{BoxBlurV1, Descriptor};
    use paintcraft_render::display::{DisplayRequest, render_display_cancellable};
    let data = json::parse(include_bytes!("layer-blend-cpu-fixtures/oracle.json")).unwrap();
    for row in data.get("spatial").unwrap().as_array().unwrap() {
        let mode = if row.get("mode").and_then(Value::as_str) == Some("multiply") {
            Mode::Multiply
        } else {
            Mode::Screen
        };
        let mut d = scene(Rgba([31, 127, 241, 128]), Rgba([0, 0, 0, 0]), mode, 2);
        let id = d.roots[1];
        d.layers.get_mut(&id).unwrap().raster = RasterLayer::from_rgba(
            2,
            1,
            Rgba([0, 0, 0, 0]),
            &[255, 0, 0, 255, 0, 0, 255, 255],
            &mut paintcraft_raster::Admission::new(65536),
        )
        .unwrap();
        d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["cpu-box"]),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        )])
        .unwrap();
        field(&mut d, Rect::new(0, 0, 1, 1).unwrap());
        let expected = row
            .get("native")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .flat_map(rgba)
            .collect::<Vec<_>>();
        assert_eq!(render(&d), expected);
        let request = DisplayRequest::new(&d, d.canvas_rect(), 1).unwrap();
        let lod = with_workspace(workspace(1_000_000), || {
            render_display_cancellable(&d, request, || Ok(()))
        })
        .unwrap();
        assert_eq!(lod.pixels, rgba(row.get("lod1").unwrap()));
        let source = d.layers[&id].raster.clone();
        d.layers.get_mut(&id).unwrap().origin = (1, 0);
        d.canvas = (3, 1);
        d.layers.get_mut(&d.roots[0]).unwrap().raster =
            RasterLayer::new(3, 1, Rgba([31, 127, 241, 128]));
        let shifted = with_workspace(workspace(1_000_000), || {
            paintcraft_render::render_region(&d, Rect::new(1, 0, 2, 1).unwrap())
        })
        .unwrap();
        assert_eq!(shifted, expected);
        assert_eq!(d.layers[&id].raster, source);
    }
}
#[test]
fn same_captured_owner_and_short_preflight_preserve_source() {
    let mut d = scene(
        Rgba([255, 255, 255, 255]),
        Rgba([204, 77, 13, 128]),
        Mode::Multiply,
        2,
    );
    d.layers.get_mut(&d.roots[1]).unwrap().opacity = 64;
    let before = d.clone();
    let short = workspace(575);
    assert_eq!(
        with_workspace(short.clone(), || render_flat(&d))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        short.work_used(),
        512,
        "one-unit-short aggregate refuses before any blend debit"
    );
    let zero = WorkspaceAdmission::new(
        SharedBudget::new(32 << 20).scope(AllocationClass::SharedBudget),
        0,
        1_000_000,
    );
    assert_eq!(
        with_workspace(zero.clone(), || render_flat(&d))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        zero.work_used(),
        0,
        "fixed active controls must fit before setup"
    );
    let a = workspace(1151);
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&d)).unwrap(),
        [249, 233, 225, 255].repeat(2)
    );
    assert_eq!(a.work_used(), 576);
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&d))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        a.work_used(),
        1088,
        "second setup stays paid, no fresh execution root"
    );
    assert!(d.same_semantics(&before));
    assert_eq!(
        paintcraft_render::prepared::quote_document_programs(&d, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported,
        "CPU scope restores on quota error"
    );
    let cancelled = with_workspace(workspace(1_000_000), || {
        paintcraft_render::render_cancellable(&d, || {
            Err(paintcraft_core::types::Error::new(
                ErrorCode::OperationCancelled,
                "stop",
            ))
        })
    });
    assert_eq!(cancelled.unwrap_err().code, ErrorCode::OperationCancelled);
    assert!(d.same_semantics(&before));
}
#[test]
fn cpu_cache_mode_identity_is_causal_and_paired_base_debits_keep_noise_owner() {
    use paintcraft_render::{
        cache::{CacheLimits, CompositeCache, Domain},
        noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace},
    };
    let mut d = scene(
        Rgba([255, 255, 255, 255]),
        Rgba([204, 77, 13, 128]),
        Mode::Multiply,
        2,
    );
    field(&mut d, Rect::new(0, 0, 2, 1).unwrap());
    let mut cache = CompositeCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 16,
    })
    .unwrap();
    let first = with_workspace(workspace(1_000_000), || {
        cache.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(first.pixels, [249, 233, 225, 255].repeat(2));
    let id = d.roots[1];
    d.layers.get_mut(&id).unwrap().blend_mode = Mode::Screen;
    let after = with_workspace(workspace(1_000_000), || {
        cache.render(&d, d.canvas_rect(), Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(after.pixels, [255, 255, 255, 255].repeat(2));
    assert_eq!(after.stats.cache_hits, 0);
    let budget = SharedBudget::new(32 << 20);
    let a = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        NoiseLimits {
            bytes: 16 << 20,
            base_work: 1_000_000,
            noise_work: 0,
        },
    )
    .unwrap();
    with_noise_workspace(a.clone(), || render_flat(&d)).unwrap();
    let used = a.used().unwrap();
    assert!(used.base > 0);
    assert_eq!(used.noise, 0);
    assert_eq!(a.workspace().work_used(), used.base);
}

#[test]
fn visible_clipboard_remains_closed_before_output_reservation() {
    let d = scene(
        Rgba([255, 255, 255, 255]),
        Rgba([1, 2, 3, 128]),
        Mode::Multiply,
        2,
    );
    let budget = SharedBudget::new(8192);
    let result = paintcraft_render::clipboard::capture_visible(
        &d,
        &paintcraft_raster::Selection::Inactive,
        &budget,
        &|| Ok(()),
    );
    assert_eq!(result.err().unwrap().code, ErrorCode::CapabilityUnsupported);
    assert_eq!(budget.usage().reserved, 0);
}

#[path = "layer_blends_cpu/request_bounds.rs"]
mod request_bounds;
