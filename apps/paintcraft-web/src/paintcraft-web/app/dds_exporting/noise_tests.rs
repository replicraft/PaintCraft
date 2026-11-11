use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution};
use paintcraft_document::{FilterInstance, FilterStack};
#[test]
fn main_selects8_for_disabled_default_and_both_distribution_intents() {
    for (enabled, amount, mode) in [
        (false, 0, NoiseDistribution::Uniform),
        (false, 10, NoiseDistribution::Gaussian),
        (true, 0, NoiseDistribution::Gaussian),
        (true, 10, NoiseDistribution::Uniform),
    ] {
        let mut doc =
            paintcraft_document::import_image(1, 1, &[64, 16, 128, 127], &"13".repeat(32)).unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(13),
                enabled,
                Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(amount, 65535, true, mode).unwrap()),
            )])
            .unwrap();
        let target = paintcraft_formats::export::metadata_policy::Target::new(
            paintcraft_formats::registry::Family::Dds,
            paintcraft_formats::registry::Operation::ExportFlat,
        );
        let bake = paintcraft_formats::export::filter_bake::Policy::capture(
            &doc,
            target,
            paintcraft_formats::export::filter_bake::Intent::Bake,
            &|| Ok(()),
        )
        .unwrap();
        let plan = Plan::capture(
            &doc,
            options(Settings::default()),
            paintcraft_formats::export::CopyPolicies {
                appearance_bake: paintcraft_formats::export::appearance_bake::Policy::Refuse,
                source_metadata: Default::default(),
                filter_bake: bake,
            },
        )
        .unwrap();
        assert_eq!(plan.protocol_version(), 8);
        let source = plan.snapshot(&doc).unwrap();
        let owner = paintcraft_formats::codec_worker::Owner {
            session: doc.document_id,
            state: doc.state_id,
            epoch: 1,
            frontend: 3,
        };
        let request = Request::capture_plan(&plan, &source, owner, doc.document_id).unwrap();
        assert_eq!(request.protocol_version(), 8);
        assert!(request.matches_captured(&plan));
        let response = wire::v8::execute(
            &request
                .encode()
                .into_iter()
                .chain(source)
                .collect::<Vec<_>>(),
            env!("CARGO_PKG_VERSION"),
            &|| Ok(()),
        )
        .unwrap();
        let mut validated = false;
        let prepared =
            response::validate_request(&request, &request.encode(), response, || validated = true)
                .unwrap();
        assert!(validated);
        assert_eq!(prepared.header.owner(), owner);
    }
}
