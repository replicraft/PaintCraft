//! Typed transport/source preservation, not an independently authored wire oracle.
use paintcraft_core::types::{
    Rect,
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    resources::SharedBudget,
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use paintcraft_formats::pcnd;
use paintcraft_raster::coverage_mask::A8Mask;
fn open(bytes: &[u8]) -> paintcraft_document::Document {
    let budget = SharedBudget::new(4 << 20);
    pcnd::read_with_admission_checked(
        bytes,
        2,
        65536,
        1,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 1 << 20,
                budget: &budget,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn both_noise_modes_disabled_zero_default_and_masked_intents_keep_pcnd6_and_raw_source() {
    let raw = [13, 71, 201, 0, 250, 1, 20, 255];
    for distribution in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        for (amount, seed, mono, enabled) in [
            (0, 0, false, true),
            (10, 1, true, false),
            (100, 65535, false, true),
        ] {
            for coverage in [0, 127, 255] {
                let mut d =
                    paintcraft_document::import_image(2, 1, &raw, "noise-transport").unwrap();
                let id = d.roots[0];
                let plain = pcnd::write(&d).unwrap();
                let mask_budget = SharedBudget::new(8192);
                let mask = A8Mask::uniform(
                    Rect::new(-1, -1, 4, 3).unwrap(),
                    coverage,
                    &mask_budget,
                    Default::default(),
                    &|| Ok(()),
                )
                .unwrap();
                let descriptor = Descriptor::AddNoiseRgbV1(
                    AddNoiseRgbV1::new(amount, seed, mono, distribution).unwrap(),
                );
                d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
                    FilterInstance::new(uuid::Uuid::from_u128(13), enabled, descriptor)
                        .with_mask(Some(FilterMask::new(enabled, mask))),
                ])
                .unwrap();
                let digest = d.layers[&id].filters.digest();
                for bytes in [pcnd::write(&d).unwrap(), pcnd::write_compact(&d).unwrap()] {
                    assert_eq!(bytes[4], 6);
                    pcnd::probe(&bytes).unwrap();
                    let restored = open(&bytes);
                    assert!(d.same_semantics(&restored));
                    assert_eq!(restored.layers[&id].filters.digest(), digest);
                    assert_eq!(
                        restored.layers[&id].filters.as_slice()[0].descriptor(),
                        descriptor
                    );
                    assert_eq!(
                        (0..2)
                            .flat_map(|x| restored.layers[&id].raster.get(x, 0).0)
                            .collect::<Vec<_>>(),
                        raw
                    );
                    let requirements = pcnd::catalog_manifest(&bytes).unwrap();
                    assert_eq!(requirements.len(), 1);
                    assert_eq!(
                        (
                            requirements[0].profile_id.as_str(),
                            requirements[0].codec_id.as_str()
                        ),
                        ("add-noise-rgb-v1", "params-v1")
                    );
                    let mut removed = restored;
                    removed.layers.get_mut(&id).unwrap().filters = Default::default();
                    assert_eq!(pcnd::write(&removed).unwrap(), plain);
                }
            }
        }
    }
}
