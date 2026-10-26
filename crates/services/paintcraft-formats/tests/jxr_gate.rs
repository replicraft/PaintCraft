//! The native conversion variant never enables generic Open or browser decode.
use paintcraft_formats::{
    codec_worker,
    registry::{self, Decision, Family, Frontend, Operation, Platform, Reason, Request},
};
#[test]
fn only_explicit_native_studio_jxr_import_is_available_and_still_requires_consent() {
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
                    Some(codec_worker::NATIVE_JXR_IMPORT_VARIANT),
                    Some(codec_worker::NATIVE_JXL_IMPORT_VARIANT),
                    Some("guess-rgb"),
                ] {
                    for extension in ["jxr", "wdp", "wmp"] {
                        let mut request = Request::new(frontend, operation, Family::Jxr, platform);
                        request.variant = variant;
                        request.extension = Some(extension);
                        let expected = frontend == Frontend::Studio
                            && platform == Platform::Native
                            && operation == Operation::ImportConvert
                            && variant == Some(codec_worker::NATIVE_JXR_IMPORT_VARIANT);
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
        Family::Jxr,
        Platform::Native,
    );
    request.variant = Some(codec_worker::NATIVE_JXR_IMPORT_VARIANT);
    request.extension = Some("png");
    assert!(!registry::admission(request).available());
    request.extension = Some("jxr");
    request.backend_available = false;
    assert!(!registry::admission(request).available());
    for platform in [Platform::Native, Platform::Web] {
        assert!(
            !registry::enabled(Frontend::Studio, Operation::Open, platform).contains(&Family::Jxr)
        );
        assert!(
            !registry::enabled(Frontend::Studio, Operation::ImportConvert, platform)
                .contains(&Family::Jxr)
        );
    }
    let source = include_bytes!("jxr-fixtures/rgb8-4x3.jxr");
    assert_eq!(registry::sniff(source), Some(Family::Jxr));
    assert!(
        registry::admit_open(Frontend::Studio, "source.jxr", source, Platform::Native).is_err()
    );
    assert!(
        registry::admit_import(Frontend::Studio, "source.jxr", source, Platform::Native).is_err()
    );
}
