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
