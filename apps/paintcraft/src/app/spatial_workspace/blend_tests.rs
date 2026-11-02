//! Host admission prerequisite only. All frontend adoption guards remain closed.
use super::*;
use paintcraft_core::types::{
    ErrorCode, LayerBlendMode as Mode, Rgba,
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_render::spatial::capture_current_workspace;

fn scene(mode: Mode) -> Document {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = uuid::Uuid::from_u128(0x627c);
    layer.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([204, 77, 13, 255]));
    layer.blend_mode = mode;
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    doc
}
fn top(doc: &mut Document) -> &mut paintcraft_document::Layer {
    let id = *doc.roots.last().unwrap();
    doc.layers.get_mut(&id).unwrap()
}
// Independent opaque identities: Multiply(white, source)=source;
// Screen(white, source)=white. No product computes expected pixels.
fn expected(mode: Mode) -> [u8; 4] {
    match mode {
        Mode::Multiply => [204, 77, 13, 255],
        Mode::Screen => [255; 4],
        Mode::Normal => panic!("new admission cases are nonNormal"),
    }
}
fn policy(doc: &Document) -> JobPolicy {
    JobPolicy::capture(doc, None, needed(doc))
}

#[test]
fn stored_modes_select_existing_workspace_even_hidden_or_zero_opacity() {
    let normal = scene(Mode::Normal);
    assert!(!needed(&normal));
    assert_eq!(policy(&normal), JobPolicy::None);
    for mode in [Mode::Multiply, Mode::Screen] {
        for (visible, opacity) in [(true, 255), (false, 255), (true, 0), (false, 0)] {
            let mut doc = scene(mode);
            top(&mut doc).visible = visible;
            top(&mut doc).opacity = opacity;
            assert!(needed_layer(&doc.layers[doc.roots.last().unwrap()]));
            assert!(needed(&doc));
            assert_eq!(policy(&doc), JobPolicy::Legacy);
        }
    }
}

#[test]
fn cpu_native_policy_uses_actual_tight_parent_and_cannot_replace_its_counter() {
    for mode in [Mode::Multiply, Mode::Screen] {
        let doc = scene(mode);
        let source = doc.clone();
        assert_eq!(
            paintcraft_render::render_flat(&doc).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let budget = SharedBudget::new(BYTES);
        // Existing two-root setup512 + opaque-extra32, without a higher cap.
        let parent =
            WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), BYTES, 544);
        with_workspace(parent.clone(), || {
            with_job(needed(&doc), || {
                let active = capture_current_workspace().unwrap();
                assert!(active.same_job(&parent));
                assert_eq!(active.work_limit(), 544);
                assert_eq!(paintcraft_render::render_flat(&doc)?, expected(mode));
                Ok(())
            })?;
            assert_eq!(parent.work_used(), 544);
            assert_eq!(
                with_job(needed(&doc), || paintcraft_render::render_flat(&doc))
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(parent.work_used(), 544);
            Ok(())
        })
        .unwrap();
        assert!(capture_current_workspace().is_none());
        assert!(doc.same_semantics(&source));
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn gpu_native_policy_uses_the_same_tight_parent_before_real_dispatch() {
    let Some((_target, gpu)) = crate::gpu_display::tests::target() else {
        return;
    };
    for mode in [Mode::Multiply, Mode::Screen] {
        let doc = scene(mode);
        let source = doc.clone();
        let budget = SharedBudget::new(BYTES);
        let parent =
            WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), BYTES, 544);
        with_workspace(parent.clone(), || {
            let pixels = with_job(needed(&doc), || {
                let active = capture_current_workspace().unwrap();
                assert!(active.same_job(&parent));
                assert_eq!(active.work_limit(), 544);
                gpu.render_cancellable(&doc, || Ok(()))
            })?;
            assert_eq!(pixels, expected(mode));
            assert_eq!(parent.work_used(), 544);
            let before = gpu.source_cache_usage();
            assert_eq!(
                with_job(needed(&doc), || gpu.render_cancellable(&doc, || Ok(())))
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(gpu.source_cache_usage(), before);
            assert_eq!(parent.work_used(), 544);
            Ok(())
        })
        .unwrap();
        assert!(capture_current_workspace().is_none());
        assert!(doc.same_semantics(&source));
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn stored_disabled_noise_still_selects_and_retains_actual_paired_parent() {
    use paintcraft_render::noise_work::{
        NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_noise_or_current, with_noise_workspace,
    };
    let mut doc = scene(Mode::Multiply);
    top(&mut doc).filters = FilterStack::new(vec![FilterInstance::new(
        uuid::Uuid::from_u128(0x627d),
        false,
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(0, 65535, false, NoiseDistribution::Gaussian).unwrap(),
        ),
    )])
    .unwrap();
    assert_eq!(policy(&doc), JobPolicy::Noise);
    let source = doc.clone();
    let budget = SharedBudget::new(BYTES);
    let limits = NoiseLimits {
        bytes: BYTES,
        base_work: WORK,
        noise_work: 1,
    };
    let parent = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::NativeV1,
        limits,
    )
    .unwrap();
    with_noise_workspace(parent.clone(), || {
        with_job(needed(&doc), || {
            policy(&doc).run(|| {
                let active =
                    capture_noise_or_current(|| panic!("captured pair cannot fall back"), limits)?;
                assert!(active.same_job(&parent));
                assert_eq!(active.limits(), limits);
                assert_eq!(
                    paintcraft_render::render_flat(&doc)?,
                    expected(Mode::Multiply)
                );
                Ok(())
            })
        })?;
        let used = parent.used()?;
        assert!(used.base > 0);
        assert_eq!(
            used.noise, 0,
            "Disabled intent selects Noise without executing it"
        );
        parent.reserve_base_work(WORK - used.base)?;
        assert_eq!(
            with_job(needed(&doc), || policy(&doc)
                .run(|| paintcraft_render::render_flat(&doc)))
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(parent.used()?.base, WORK);
        assert_eq!(parent.used()?.noise, 0);
        Ok(())
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
    assert!(doc.same_semantics(&source));
    drop(parent);
    assert_eq!(budget.usage().reserved, 0);
}
