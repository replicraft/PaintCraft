use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{BoxBlurV1, LevelsRgbV1},
    resources::SharedBudget,
};
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};

fn levels(values: [i16; 5]) -> Descriptor {
    Descriptor::LevelsRgbV1(
        LevelsRgbV1::new(values[0], values[1], values[2], values[3], values[4]).unwrap(),
    )
}
fn one(values: [i16; 5], enabled: bool) -> FilterStack {
    FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(1),
        enabled,
        levels(values),
    )])
    .unwrap()
}

#[test]
fn independent_catalog_semantic_goldens_pin_parameters_enabled_and_mask_presence() {
    // Constants come from the independent planned-PCND6 Python fixture writer,
    // not the product document hash or a transport codec implementation.
    for (values, enabled, digest) in [
        (
            [0, 255, 100, 0, 255],
            true,
            "208631f0adacb457c55ad4f5c7003753fb4313fee9e370bd93b63192f69f6036",
        ),
        (
            [0, 255, 100, 0, 255],
            false,
            "69ed6040643db113ad93957b941a20b7c54965ad97c45e9fa077d448b8a159dd",
        ),
        (
            [0, 255, 10, 0, 255],
            true,
            "f49e7ab6f951c8e52b629476565a74170005e6fc7c0e4326d1ce75f9d5511276",
        ),
        (
            [17, 201, 999, 240, 16],
            true,
            "90519f654593e78faf91fecfe8b153c1fd2ef67ee6637e5a63234153e207cb4f",
        ),
        (
            [64, 192, 200, 73, 73],
            true,
            "a7bdc97dcc9bd7bef29a80b98ba035eb4f6e8c9b6976710291d2612a6b6d040e",
        ),
    ] {
        assert_eq!(crate::hex(&one(values, enabled).digest()), digest);
    }
    let realm = SharedBudget::new(8192);
    let runs = [
        Run {
            x0: -1,
            x1: 1,
            coverage: 128,
        },
        Run {
            x0: 1,
            x1: 2,
            coverage: 255,
        },
    ];
    let field = A8Mask::from_bands(
        Rect::new(-1, 0, 3, 1).unwrap(),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        }],
        &realm,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let entries = vec![
        FilterInstance::new(
            Uuid::from_u128(1),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ),
        FilterInstance::new(Uuid::from_u128(2), true, levels([0, 255, 200, 0, 255]))
            .with_mask(Some(FilterMask::new(true, field))),
    ];
    let stack = FilterStack::new(entries).unwrap();
    assert_eq!(
        crate::hex(&stack.digest()),
        "bf3101747ebbdac3267cfa724556d8823fc446c30362e5661a5ac990522a9050"
    );
}

#[test]
fn catalog_intentions_preserve_raw_source_mask_ownership_and_distinct_identity() {
    let realm = SharedBudget::new(8192);
    {
        let field = A8Mask::uniform(
            Rect::new(-1, -1, 4, 3).unwrap(),
            127,
            &realm,
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        let usage = field.storage_usage().unwrap();
        let original = FilterInstance::new(Uuid::from_u128(9), false, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(false, field)));
        let changed = original.with_descriptor(levels([0, 255, 100, 0, 255]));
        let duplicate = changed.with_id(Uuid::from_u128(10)).with_enabled(true);
        assert_eq!(changed.id(), original.id());
        assert!(!changed.enabled());
        for entry in [&changed, &duplicate] {
            assert_eq!(entry.mask(), original.mask());
            assert_eq!(
                entry.mask().unwrap().coverage().storage_usage().unwrap(),
                usage
            );
        }
        let before =
            crate::import_image(2, 1, &[13, 71, 201, 0, 250, 1, 20, 255], "catalog-document")
                .unwrap();
        let id = before.roots[0];
        let mut after = before.clone();
        after.layers.get_mut(&id).unwrap().filters =
            FilterStack::new(vec![changed, duplicate]).unwrap();
        assert_eq!(after.layers[&id].raster, before.layers[&id].raster);
        let a = one([0, 255, 999, 73, 73], true);
        let b = one([0, 255, 10, 73, 73], true);
        assert_ne!(
            a.digest(),
            b.digest(),
            "Equal rendered constant tables do not erase gamma intent"
        );
        let reverse =
            FilterStack::new(after.layers[&id].filters.iter().rev().cloned().collect()).unwrap();
        assert_ne!(reverse.digest(), after.layers[&id].filters.digest());
        assert_eq!(realm.usage().reserved, usage.1);
    }
    assert_eq!(realm.usage().reserved, 0);
}
