use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_formats::{
    dds::{
        self,
        web_export::{self as wire, v8},
    },
    export,
};
#[test]
fn actual_dds_noise_entry_routes_both_policies_without_legacy_or_rgba_conversion() {
    for (mode, expected) in [
        (NoiseDistribution::Uniform, [86, 38, 150, 127]),
        (NoiseDistribution::Gaussian, [70, 22, 134, 127]),
    ] {
        let mut doc =
            paintcraft_document::import_image(1, 1, &[64, 16, 128, 127], &"13".repeat(32)).unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(3),
                true,
                Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(10, 1, true, mode).unwrap()),
            )])
            .unwrap();
        let target = export::metadata_policy::Target::new(
            paintcraft_formats::registry::Family::Dds,
            paintcraft_formats::registry::Operation::ExportFlat,
        );
        let bake = export::filter_bake::Policy::capture(
            &doc,
            target,
            export::filter_bake::Intent::Bake,
            &|| Ok(()),
        )
        .unwrap();
        let settings = dds::TextureOptions {
            encoding: dds::Encoding::Rgba8,
            transfer: dds::Transfer::Srgb,
            alpha: dds::SourceAlpha::Straight,
            mips: dds::MipCount::Levels(1),
            filter: dds::MipFilter::Nearest,
            quality: dds::TextureQuality::Normal,
        };
        // Explicit browser endpoint policy; no extra Render dependency.
        let workload = v8::default_workload().unwrap();
        let plan = v8::NoiseSourcePlan::capture_with_policies_checked(
            &doc,
            settings,
            export::CopyPolicies {
                source_metadata: Default::default(),
                filter_bake: bake,
            },
            workload,
            &|| Ok(()),
        )
        .unwrap();
        let source = plan.snapshot(&doc).unwrap();
        let owner = paintcraft_formats::codec_worker::Owner {
            session: doc.document_id,
            state: doc.state_id,
            epoch: 1,
            frontend: 3,
        };
        let h = v8::Header::capture(&plan, &source, owner, doc.document_id).unwrap();
        let mut req = h.encode().unwrap().to_vec();
        req.extend(source);
        let response = super::dds(&req).unwrap();
        let v8::Frame::Success { preview, .. } =
            v8::decode_response(&response, &h, owner, env!("CARGO_PKG_VERSION")).unwrap()
        else {
            panic!("expected real thin binding success")
        };
        assert_eq!(preview, expected);
        assert!(wire::v7::Header::decode(&req[..v8::REQUEST_HEADER]).is_err());
    }
}
