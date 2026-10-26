use paintcraft_formats::{
    codec_worker as wire,
    registry::{self, Decision, Family, Frontend, Operation, Platform, Request},
};
#[test]
fn only_family_matched_explicit_native_studio_float_conversion_is_available() {
    for (family, extension, correct) in [
        (Family::Hdr, "hdr", wire::NATIVE_HDR_SDR_IMPORT_VARIANT),
        (Family::Exr, "exr", wire::NATIVE_EXR_SDR_IMPORT_VARIANT),
    ] {
        for frontend in [
            Frontend::Studio,
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            for platform in [Platform::Native, Platform::Web] {
                for operation in [
                    Operation::Open,
                    Operation::OpenRaster,
                    Operation::OpenProject,
                    Operation::ImportConvert,
                    Operation::SavePreserving,
                    Operation::SaveAsCopy,
                    Operation::ExportFlat,
                ] {
                    for variant in [
                        None,
                        Some("invented"),
                        Some(wire::NATIVE_HDR_SDR_IMPORT_VARIANT),
                        Some(wire::NATIVE_EXR_SDR_IMPORT_VARIANT),
                    ] {
                        let mut request = Request::new(frontend, operation, family, platform);
                        request.extension = Some(extension);
                        request.variant = variant;
                        let allowed = frontend == Frontend::Studio
                            && platform == Platform::Native
                            && operation == Operation::ImportConvert
                            && variant == Some(correct);
                        assert_eq!(
                            registry::admission(request).available(),
                            allowed,
                            "{request:?}"
                        );
                        if allowed {
                            assert_eq!(
                                registry::admission(request),
                                Decision::AllowedWithPreflight(
                                    registry::Reason::ConvertRequiresConsent
                                )
                            );
                            request.backend_available = false;
                            assert!(!registry::admission(request).available());
                        }
                    }
                }
            }
        }
        let mut wrong = Request::new(
            Frontend::Studio,
            Operation::ImportConvert,
            family,
            Platform::Native,
        );
        wrong.variant = Some(correct);
        wrong.extension = Some("png");
        assert!(!registry::admission(wrong).available());
    }
}
