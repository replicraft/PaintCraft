use paintcraft_formats::{
    codec_worker::NATIVE_JXL_IMPORT_VARIANT,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
#[test]
fn only_exact_native_studio_import_variant_is_qualified() {
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
                Operation::ImportConvert,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                let mut request = Request::new(frontend, operation, Family::Jxl, platform);
                request.extension = Some("jxl");
                request.variant = Some(NATIVE_JXL_IMPORT_VARIANT);
                assert_eq!(
                    registry::admission(request).available(),
                    frontend == Frontend::Studio
                        && platform == Platform::Native
                        && operation == Operation::ImportConvert,
                    "{request:?}"
                );
            }
        }
    }
    for variant in [
        None,
        Some("native-jxl-srgb8-worker-v0"),
        Some("native-jxl-srgb8-worker-v2"),
    ] {
        let mut request = Request::new(
            Frontend::Studio,
            Operation::ImportConvert,
            Family::Jxl,
            Platform::Native,
        );
        request.variant = variant;
        assert!(!registry::admission(request).available());
    }
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        Family::Jxl,
        Platform::Native,
    );
    request.variant = Some(NATIVE_JXL_IMPORT_VARIANT);
    request.backend_available = false;
    assert!(!registry::admission(request).available());
    request.backend_available = true;
    request.extension = Some("png");
    assert!(!registry::admission(request).available());
}

#[test]
fn browser_jxl_requires_its_own_exact_studio_import_variant() {
    use paintcraft_formats::registry::WEB_JXL_IMPORT_VARIANT;
    for frontend in [
        Frontend::Studio,
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        for platform in [Platform::Native, Platform::Web] {
            for operation in [
                Operation::Open,
                Operation::ImportConvert,
                Operation::ExportFlat,
                Operation::SaveAsCopy,
                Operation::SavePreserving,
            ] {
                let mut request = Request::new(frontend, operation, Family::Jxl, platform);
                request.variant = Some(WEB_JXL_IMPORT_VARIANT);
                request.extension = Some("jxl");
                assert_eq!(
                    registry::admission(request).available(),
                    frontend == Frontend::Studio
                        && platform == Platform::Web
                        && operation == Operation::ImportConvert,
                    "{request:?}"
                );
            }
        }
    }
}

#[test]
fn integer_jxl_requires_explicit_native_studio_conversion_and_preflight() {
    use paintcraft_formats::codec_worker::jxl_integer::contracts::IMPORT_VARIANT;
    assert_integer_conversion_gate(IMPORT_VARIANT, Platform::Native);
}

#[test]
fn integer_jxl_browser_variant_requires_web_studio_conversion_and_preflight() {
    assert_integer_conversion_gate(registry::WEB_JXL_INTEGER_IMPORT_VARIANT, Platform::Web);
}

fn assert_integer_conversion_gate(variant: &str, admitted_platform: Platform) {
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
                Operation::ImportConvert,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                let mut request = Request::new(frontend, operation, Family::Jxl, platform);
                request.extension = Some("jxl");
                request.variant = Some(variant);
                let actual = registry::admission(request);
                let expected = frontend == Frontend::Studio
                    && platform == admitted_platform
                    && operation == Operation::ImportConvert;
                assert_eq!(actual.available(), expected, "{request:?}");
                if expected {
                    assert_eq!(
                        actual,
                        registry::Decision::AllowedWithPreflight(
                            registry::Reason::ConvertRequiresConsent
                        )
                    );
                    request.backend_available = false;
                    assert!(!registry::admission(request).available());
                    request.backend_available = true;
                    request.extension = Some("png");
                    assert!(!registry::admission(request).available());
                }
            }
        }
    }
}
