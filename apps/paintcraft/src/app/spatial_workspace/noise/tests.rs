use super::*;
use paintcraft_core::types::{
    Rgba,
    filters::{AddNoiseRgbV1, NoiseDistribution},
};
use paintcraft_document::{FilterInstance, FilterStack};
fn source() -> Document {
    paintcraft_document::import_image(
        2,
        1,
        &[30, 80, 160, 255, 70, 100, 180, 255],
        "native-noise-root-selection/v1",
    )
    .unwrap()
}
fn descriptor() -> Descriptor {
    Descriptor::AddNoiseRgbV1(
        AddNoiseRgbV1::new(0, 65535, false, NoiseDistribution::Gaussian).unwrap(),
    )
}
#[test]
fn prospective_first_noise_and_stored_disabled_intent_select_without_lut_keys() {
    let mut doc = source();
    assert_eq!(JobPolicy::capture(&doc, None, false), JobPolicy::None);
    assert_eq!(JobPolicy::capture(&doc, None, true), JobPolicy::Legacy);
    assert_eq!(
        JobPolicy::capture(&doc, Some(&descriptor()), true),
        JobPolicy::Noise
    );
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(73),
            false,
            descriptor(),
        )])
        .unwrap();
    assert_eq!(JobPolicy::capture(&doc, None, true), JobPolicy::Noise);
    let error = legacy_document(&doc, || Ok(Rgba([0; 4]))).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
}
#[test]
fn native_noise_root_shares_realm_and_never_upgrades_a_legacy_job() {
    with_admission(SharedBudget::new(BYTES), || {
        assert_eq!(
            JobPolicy::Noise.run(|| Ok(())).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        Ok(())
    })
    .unwrap();
    JobPolicy::Noise
        .run(|| JobPolicy::Noise.run(|| Ok(())))
        .unwrap();
}

#[test]
fn native_legacy_child_keeps_the_depleted_parent_instead_of_replacing_it() {
    use paintcraft_core::types::{Rect, filters::BoxBlurV1};
    let mut doc = source();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(74),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        )])
        .unwrap();
    let cost = paintcraft_render::spatial::quote_layer(
        &doc.layers[&doc.roots[0]],
        Rect::new(0, 0, 2, 1).unwrap(),
        &|| Ok(()),
    )
    .unwrap()
    .work;
    let parent_budget = SharedBudget::new(BYTES);
    let parent = WorkspaceAdmission::new(
        parent_budget.scope(AllocationClass::SharedBudget),
        BYTES,
        cost,
    );
    let observer = parent.clone();
    with_workspace(parent, || {
        JobPolicy::Legacy.run(|| paintcraft_render::render_flat(&doc))?;
        assert!(observer.work_used() > 0);
        assert_eq!(
            legacy_document(&doc, || paintcraft_render::render_flat(&doc))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(parent_budget.usage().reserved, 0);
}
