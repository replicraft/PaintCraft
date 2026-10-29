use paintcraft_core::types::ErrorCode;
use paintcraft_formats::{registry::*, tiff_pages};

const MULTI: &[u8] = include_bytes!("tiff-fixtures/unsupported-first-rgba16.tif");

#[test]
fn selected_tiff_variant_is_studio_conversion_only_on_both_platforms() {
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
        Frontend::Studio,
    ] {
        for platform in [Platform::Native, Platform::Web] {
            for operation in [
                Operation::Open,
                Operation::OpenProject,
                Operation::OpenRaster,
                Operation::ImportConvert,
                Operation::ImportAsset,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
                Operation::ExportAsset,
            ] {
                let mut request = Request::new(frontend, operation, Family::Tiff, platform);
                request.extension = Some("tiff");
                request.variant = Some(tiff_pages::IMPORT_VARIANT);
                let expected =
                    frontend == Frontend::Studio && operation == Operation::ImportConvert;
                assert_eq!(admission(request).available(), expected, "{request:?}");
                if expected {
                    assert_eq!(
                        admission(request),
                        Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
                    );
                    assert!(admission(request).require_allowed(request).is_err());
                    request.backend_available = false;
                    assert!(!admission(request).available());
                    request.backend_available = true;
                    request.extension = Some("png");
                    assert!(!admission(request).available());
                }
            }
        }
    }
}

#[test]
fn explicit_page_admission_preserves_unsupported_first_page_and_signature_guards() {
    for platform in [Platform::Native, Platform::Web] {
        assert_eq!(
            admit_import_page(Frontend::Studio, "copy.tif", MULTI, platform, 1).unwrap(),
            Family::Tiff
        );
        assert_eq!(
            admit_import_page(Frontend::Studio, "copy.tif", MULTI, platform, 0)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            admit_import_page(Frontend::Studio, "copy.tif", MULTI, platform, 99)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            admit_import_page(Frontend::Studio, "copy.png", MULTI, platform, 1)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        for frontend in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            assert!(admit_import_page(frontend, "copy.tif", MULTI, platform, 1).is_err());
        }
    }
    // Discovery still permits the old strict single-image TIFF route. Its
    // decoder must refuse a multipage source instead of choosing page zero.
    assert_eq!(
        admit_open(Frontend::Studio, "copy.tif", MULTI, Platform::Native).unwrap(),
        Family::Tiff
    );
    assert!(paintcraft_formats::image::decode(MULTI).is_err());
}
