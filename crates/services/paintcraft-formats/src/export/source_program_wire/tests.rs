use super::*;
use paintcraft_core::types::filters::{Descriptor, ExposureSrgbV1};
use paintcraft_document::{FilterInstance, FilterStack};
use uuid::Uuid;
fn document(instances: Vec<(bool, Descriptor)>) -> Document {
    let mut doc =
        paintcraft_document::import_image(1, 1, &[64, 16, 128, 255], "program quote").unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(
        instances
            .into_iter()
            .enumerate()
            .map(|(index, (enabled, descriptor))| {
                FilterInstance::new(Uuid::from_u128(index as u128 + 1), enabled, descriptor)
            })
            .collect(),
    )
    .unwrap();
    doc
}
#[test]
fn enabled_direct_instances_can_have_no_prepared_keys_but_disabled_requirements_persist() {
    let doc = document(vec![
        (true, Descriptor::InvertRgbV1),
        (
            false,
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(0).unwrap()),
        ),
    ]);
    let region = SourceSpatialContext::capture_programs(&doc, 0, &|| Ok(())).unwrap();
    let context = SourceProgramContext::capture(&doc, region, &|| Ok(())).unwrap();
    let bytes = context.encode();
    assert_eq!(
        &bytes[4..20],
        &[0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 0]
    );
    assert_eq!(&bytes[24..48], &[0; 24]);
    assert!(context.work() > 0);
    let decoded = SourceProgramContext::decode(&bytes).unwrap();
    decoded.validate_region(region).unwrap();
    assert_eq!(decoded, context);
    let requirements = Requirements::capture(&doc, &|| Ok(())).unwrap();
    context
        .validate_requirements(&requirements, &|| Ok(()))
        .unwrap();
    for offset in [20, 21, 22, 23] {
        let mut bad = bytes;
        bad[offset] = 1;
        assert!(SourceProgramContext::decode(&bad).is_err());
    }
}
#[test]
fn complete_parameter_keys_deduplicate_programs_without_changing_instance_counts() {
    let descriptor = Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap());
    let doc = document(vec![
        (true, descriptor),
        (true, descriptor),
        (false, descriptor),
    ]);
    let region = SourceSpatialContext::capture_programs(&doc, 0, &|| Ok(())).unwrap();
    let context = SourceProgramContext::capture(&doc, region, &|| Ok(())).unwrap();
    let bytes = context.encode();
    assert_eq!(
        &bytes[4..20],
        &[0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0, 1]
    );
    let quote = paintcraft_render::prepared::quote_document_programs(&doc, &|| Ok(())).unwrap();
    assert_eq!(context.compile_work(), quote.compile_work);
    assert_eq!(context.retained_bytes(), quote.retained_bytes);
    assert_eq!(
        context.workspace_peak(),
        quote
            .preparation_peak_bytes
            .max(quote.retained_bytes + region.workspace_bytes())
    );
    assert_eq!(context.work(), quote.compile_work + region.work());
}
