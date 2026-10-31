//! Independent semantic-byte goldens; numerical LUTs are not document identity.
use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{ExposureSrgbV1, LevelsRgbV1},
    resources::SharedBudget,
};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
fn exposure(stops: i16) -> Descriptor {
    Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(stops).unwrap())
}
fn field(budget: &SharedBudget, coverage: u8) -> A8Mask {
    A8Mask::uniform(
        Rect::new(-1, -1, 4, 3).unwrap(),
        coverage,
        budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn independent_exposure_v3_goldens_pin_signed_identity_disabled_and_mixed_mask_bytes() {
    // Independently assembled Python struct/hashlib records from the v3 contract;
    // neither semantic_parameter_bytes nor FilterStack::digest supplied expected bytes.
    for (stops, enabled, expected) in [
        (
            -800,
            true,
            "07289d9c14e271567ac95d95c12a2a1a505e983d8819de0921b2ef5d8fb483a5",
        ),
        (
            0,
            true,
            "3423b29988097c42abfeefa1d3e10d3723385ae25cf0e3fec1d67c6d9da914d7",
        ),
        (
            800,
            true,
            "6ce79d569287fe7cea3f47c7584062d2821cdd567e428f82816bc5390109e935",
        ),
        (
            0,
            false,
            "00013619ae5a6443230fe51667663bc99a6590929fc3a987a5695ba0b5e6081a",
        ),
    ] {
        let stack = FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(1),
            enabled,
            exposure(stops),
        )])
        .unwrap();
        assert_eq!(crate::hex(&stack.digest()), expected);
    }
    let budget = SharedBudget::new(8192);
    let coverage = field(&budget, 127);
    assert_eq!(
        crate::hex(&coverage.digest()),
        "5e38cf1164b88cd9c781811765f578f0ecbb54d0cf24c32a0f5d7b9dabf9a549"
    );
    let stack = FilterStack::new(vec![
        FilterInstance::new(
            Uuid::from_u128(1),
            false,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 100, 0, 255).unwrap()),
        ),
        FilterInstance::new(Uuid::from_u128(2), true, exposure(-137))
            .with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    assert_eq!(
        crate::hex(&stack.digest()),
        "3999a1d8c9e73f2f0bc257b2bed066d7b3d0fd384af7f1de8f23b758a2f5eccd"
    );
}
#[test]
fn exposure_kind_order_parameters_and_explicit_mask_intentions_are_distinct_without_pixel_changes()
{
    let budget = SharedBudget::new(8192);
    {
        let base = FilterInstance::new(Uuid::from_u128(9), true, exposure(0));
        let digest = |entry: FilterInstance| FilterStack::new(vec![entry]).unwrap().digest();
        for changed in [
            base.with_descriptor(exposure(-1)),
            base.with_descriptor(exposure(1)),
            base.with_descriptor(Descriptor::LevelsRgbV1(
                LevelsRgbV1::new(0, 255, 100, 0, 255).unwrap(),
            )),
            base.with_enabled(false),
            base.with_id(Uuid::from_u128(10)),
            base.with_mask(Some(FilterMask::new(true, field(&budget, 0)))),
            base.with_mask(Some(FilterMask::new(false, field(&budget, 127)))),
        ] {
            assert_ne!(digest(base.clone()), digest(changed));
        }
        let coverage = field(&budget, 127);
        let usage = coverage.storage_usage().unwrap();
        let masked = base.with_mask(Some(FilterMask::new(false, coverage)));
        let active = masked.with_mask(Some(masked.mask().unwrap().with_enabled(true)));
        assert_ne!(digest(masked.clone()), digest(active.clone()));
        let update = active.with_descriptor(exposure(800));
        assert_eq!(update.mask(), active.mask());
        assert_eq!(
            update.mask().unwrap().coverage().storage_usage(),
            Some(usage)
        );
        let before = crate::import_image(
            2,
            1,
            &[13, 71, 201, 0, 250, 1, 20, 255],
            "exposure-document",
        )
        .unwrap();
        let id = before.roots[0];
        let mut after = before.clone();
        after.layers.get_mut(&id).unwrap().filters =
            FilterStack::new(vec![masked, update.with_id(Uuid::from_u128(10))]).unwrap();
        assert_eq!(after.layers[&id].raster, before.layers[&id].raster);
        assert_ne!(
            FilterStack::new(after.layers[&id].filters.iter().rev().cloned().collect())
                .unwrap()
                .digest(),
            after.layers[&id].filters.digest()
        );
        assert_eq!(budget.usage().reserved, usage.1);
    }
    assert_eq!(budget.usage().reserved, 0);
}
