use super::*;
use paintcraft_core::types::{
    ErrorCode,
    filters::{AddNoiseRgbV1, NoiseDistribution},
};
#[test]
fn noise_registration_is_one_umbrella_with_both_strict_priced_policy_branches() {
    let e = resolve("add-noise-rgb-v1", "params-v1").unwrap();
    assert_eq!(e.execution(), Execution::DirectNoise);
    assert_eq!(
        e.resource_contract(),
        ("direct-source-noise-rgba8-u512-g4096-v1", 1)
    );
    assert_eq!(registrations()[11].profile(), "hue-saturation-rgb-v1");
    assert_eq!(registrations()[12].profile(), "add-noise-rgb-v1");
    for distribution in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        for amount in [0, 10, 100] {
            let p = AddNoiseRgbV1::new(amount, 65535, false, distribution).unwrap();
            let d = Descriptor::AddNoiseRgbV1(p);
            assert_eq!(e.decode(&p.semantic_bytes()).unwrap(), d);
            assert!(descriptor_requires_generic(d).unwrap());
        }
    }
    for bytes in [
        &[][..],
        &[0; 5][..],
        &[0; 7][..],
        &[0, 101, 0, 0, 0, 0][..],
        &[0, 0, 0, 0, 2, 0][..],
        &[0, 0, 0, 0, 0, 2][..],
    ] {
        assert_eq!(e.decode(bytes).unwrap_err().code, ErrorCode::CodecFailed);
    }
    assert_eq!(
        resolve("add-noise-rgb-v1", "params-v2").unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn noise_subset_does_not_mutate_old_twelve_and_disabled_default_still_requires_it() {
    let mut old = Requirements::default();
    for e in &registrations()[..12] {
        old.insert(e.profile(), e.codec()).unwrap();
    }
    let before = old.digests(&|| Ok(())).unwrap();
    let mut noise = Requirements::default();
    noise.insert("add-noise-rgb-v1", "params-v1").unwrap();
    let union = old.union(&noise, &|| Ok(())).unwrap();
    assert_ne!(union.digests(&|| Ok(())).unwrap(), before);
    assert_eq!(old.digests(&|| Ok(())).unwrap(), before);
    let mut d =
        paintcraft_document::import_image(1, 1, &[10, 11, 12, 0], "noise-requirement").unwrap();
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            uuid::Uuid::from_u128(13),
            false,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    assert!(requires_generic(&d, &|| Ok(())).unwrap());
    assert_eq!(
        Requirements::capture(&d, &|| Ok(()))
            .unwrap()
            .pairs()
            .collect::<Vec<_>>(),
        [("add-noise-rgb-v1", "params-v1")]
    );
}
