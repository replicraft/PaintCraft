//! The native conversion variant never enables generic Open or browser decode.
use paintcraft_formats::{
    codec_worker,
    registry::{self, Decision, Family, Frontend, Operation, Platform, Reason, Request},
};
#[test]
fn only_explicit_native_studio_heif_import_is_available_and_still_requires_consent() {
    for frontend in [
        Frontend::Studio,
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        for platform in [Platform::Native, Platform::Web] {
            for operation in [
                Operation::Open,
                Operation::OpenProject,
                Operation::OpenRaster,
                Operation::ImportConvert,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                for variant in [
                    None,
                    Some(codec_worker::NATIVE_HEIF_IMPORT_VARIANT),
                    Some(codec_worker::NATIVE_JXL_IMPORT_VARIANT),
                    Some("guess-rgb"),
                ] {
                    for extension in ["heic", "heif", "hif"] {
                        let mut request = Request::new(frontend, operation, Family::Heif, platform);
                        request.variant = variant;
                        request.extension = Some(extension);
                        let expected = frontend == Frontend::Studio
                            && platform == Platform::Native
                            && operation == Operation::ImportConvert
                            && variant == Some(codec_worker::NATIVE_HEIF_IMPORT_VARIANT);
                        let decision = registry::admission(request);
                        assert_eq!(decision.available(), expected, "{request:?}");
                        if expected {
                            assert_eq!(
                                decision,
                                Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
                            );
                            assert!(decision.require_allowed(request).is_err());
                        }
                    }
                }
            }
        }
    }
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        Family::Heif,
        Platform::Native,
    );
    request.variant = Some(codec_worker::NATIVE_HEIF_IMPORT_VARIANT);
    request.extension = Some("png");
    assert!(!registry::admission(request).available());
    request.extension = Some("heic");
    request.backend_available = false;
    assert!(!registry::admission(request).available());
    for platform in [Platform::Native, Platform::Web] {
        assert!(
            !registry::enabled(Frontend::Studio, Operation::Open, platform).contains(&Family::Heif)
        );
        assert!(
            !registry::enabled(Frontend::Studio, Operation::ImportConvert, platform)
                .contains(&Family::Heif)
        );
    }
    let source = include_bytes!("heif-color-fixtures/identity444.heic");
    assert_eq!(registry::sniff(source), Some(Family::Heif));
    assert!(
        registry::admit_open(Frontend::Studio, "source.heif", source, Platform::Native).is_err()
    );
    assert!(
        registry::admit_import(Frontend::Studio, "source.heif", source, Platform::Native).is_err()
    );
}
#[test]
fn shared_mif1_is_not_a_heif_signature_without_an_explicit_hevc_brand() {
    let source = include_bytes!("heif-color-fixtures/identity444.heic");
    let extent = u32::from_be_bytes(source[..4].try_into().unwrap()) as usize;
    let mut ambiguous = source[..extent].to_vec();
    ambiguous[8..12].copy_from_slice(b"mif1");
    for brand in ambiguous[16..].as_chunks_mut::<4>().0 {
        brand.copy_from_slice(b"mif1");
    }
    assert!(!paintcraft_formats::heif::has_signature(&ambiguous));
    ambiguous[16..20].copy_from_slice(b"heic");
    assert!(paintcraft_formats::heif::has_signature(&ambiguous));
    let mut extended = vec![0, 0, 0, 1, b'f', b't', b'y', b'p'];
    extended.extend(((extent + 8) as u64).to_be_bytes());
    extended.extend(&source[8..extent]);
    assert!(paintcraft_formats::heif::has_signature(&extended));
    for end in 0..extended.len() {
        assert!(!paintcraft_formats::heif::has_signature(&extended[..end]));
    }
}
