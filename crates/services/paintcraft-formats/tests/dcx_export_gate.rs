use paintcraft_formats::{
    dcx,
    registry::{self, Decision, Family, Frontend, Operation, Platform, Request},
};
#[test]
fn only_explicit_native_studio_fresh_copy_is_qualified() {
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
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                for variant in [None, Some(dcx::EXPORT_VARIANT), Some("invented")] {
                    let mut request = Request::new(frontend, operation, Family::Dcx, platform);
                    request.extension = Some("dcx");
                    request.variant = variant;
                    let allowed = frontend == Frontend::Studio
                        && platform == Platform::Native
                        && operation == Operation::SaveAsCopy
                        && variant == Some(dcx::EXPORT_VARIANT);
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
    let mut request = Request::new(
        Frontend::Studio,
        Operation::SaveAsCopy,
        Family::Dcx,
        Platform::Native,
    );
    request.variant = Some(dcx::EXPORT_VARIANT);
    request.extension = Some("pcx");
    assert!(!registry::admission(request).available());
}
