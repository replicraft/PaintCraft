//! Editable processing cannot silently disappear in layered interchange copies.
use paintcraft_core::types::{ErrorCode, Rgba, filters::Descriptor, ids::deterministic_id};
use paintcraft_document::{
    POLICY,
    filters::{FilterInstance, FilterStack},
    new_document,
};
use paintcraft_formats::{
    export::metadata_policy::{Intent, Policy, Target},
    ora, paint, pdn, psd,
    registry::{Family, Operation},
};

#[test]
fn layered_writers_refuse_even_disabled_filters_before_output_and_omission_cannot_override() {
    for enabled in [false, true] {
        let mut document = new_document(2, 2, Some(Rgba([12, 34, 56, 255])), 1, POLICY).unwrap();
        document.layers.get_mut(&document.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                deterministic_id(&["foreign-filter"]),
                enabled,
                Descriptor::InvertRgbV1,
            )])
            .unwrap();
        let before = document.clone();
        let pdn_options = pdn::Options {
            background: pdn::BackgroundRole::Ordinary,
            geometry: pdn::Geometry::Materialize,
        };
        assert_eq!(
            pdn::prepare_document(&document, pdn_options, &Default::default(), &[], &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let paint_options = paint::Options {
            canvas_fill: [0; 4],
            layer_names: paint::LayerNames::Omit,
        };
        assert_eq!(
            paint::prepare_document(&document, paint_options, &Default::default(), &[], &|| Ok(
                ()
            ))
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        for version in [psd::Version::Psd, psd::Version::Psb] {
            let mut sink = vec![1, 2, 3];
            assert_eq!(
                psd::write_document_to(
                    &document,
                    version,
                    &Default::default(),
                    &[],
                    &mut sink,
                    &|| Ok(())
                )
                .unwrap_err()
                .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(sink, [1, 2, 3]);
        }
        let target = Target::new(Family::Ora, Operation::SaveAsCopy);
        for intent in [Intent::Refuse, Intent::Omit] {
            let policy = Policy::capture(&document, target, intent).unwrap();
            let mut sink = vec![1, 2, 3];
            assert_eq!(
                ora::write_document_to_with_metadata_policy(
                    &document,
                    &mut sink,
                    &Default::default(),
                    &[],
                    &|| Ok(()),
                    policy
                )
                .unwrap_err()
                .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(sink, [1, 2, 3]);
        }
        assert!(document.same_semantics(&before));
        assert_eq!(
            document.layers[&document.roots[0]].raster.resident_tiles(),
            0
        );
    }
}
