//! Initial Retro policy is distinct from historical parity or backend presence.
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::{
    image::{self, ImageFormat},
    pcnd,
    registry::*,
};
const RGBA: [u8; 16] = [
    12, 34, 56, 255, 78, 90, 123, 255, 222, 111, 7, 255, 19, 23, 29, 255,
];
const OPERATIONS: [Operation; 9] = [
    Operation::Open,
    Operation::OpenProject,
    Operation::OpenRaster,
    Operation::ImportConvert,
    Operation::ImportAsset,
    Operation::SavePreserving,
    Operation::SaveAsCopy,
    Operation::ExportFlat,
    Operation::ExportAsset,
];

#[test]
fn exact_initial_inventory_does_not_inherit_classic_or_studio_modes() {
    assert_eq!(Frontend::RetroWin95.id(), "retro-win95");
    let expected: [&[Family]; 9] = [
        &[Family::Pcnd, Family::Png, Family::Bmp],
        &[Family::Pcnd],
        &[Family::Png, Family::Bmp],
        &[Family::Png, Family::Bmp],
        &[],
        &[Family::Pcnd, Family::Png, Family::Bmp],
        &[Family::Pcnd, Family::Png, Family::Bmp],
        &[Family::Png, Family::Bmp],
        &[],
    ];
    for platform in [Platform::Native, Platform::Web] {
        for (operation, expected) in OPERATIONS.into_iter().zip(expected) {
            assert_eq!(
                enabled(Frontend::RetroWin95, operation, platform),
                expected,
                "{operation:?}/{platform:?}"
            );
            for family in Family::ALL {
                let request = Request::new(Frontend::RetroWin95, operation, *family, platform);
                assert_eq!(
                    admission(request).available(),
                    expected.contains(family),
                    "{operation:?}/{platform:?}/{family:?}"
                );
                if ![Family::Pcnd, Family::Png, Family::Bmp].contains(family) {
                    assert_eq!(
                        admission(request),
                        Decision::Denied(Reason::UnsupportedFrontendFormat)
                    );
                    let mut named = request;
                    named.variant = Some("cannot-bypass-profile");
                    assert_eq!(
                        admission(named),
                        Decision::Denied(Reason::UnsupportedFrontendFormat)
                    );
                }
            }
        }
        assert_eq!(
            enabled_extensions(Frontend::RetroWin95, Operation::Open, platform),
            ["pcnd", "png", "bmp"]
        );
    }
    for operation in [
        Operation::ImportConvert,
        Operation::OpenRaster,
        Operation::ExportFlat,
    ] {
        assert_eq!(
            admission(Request::new(
                Frontend::RetroWin95,
                operation,
                Family::Pcnd,
                Platform::Native
            )),
            Decision::Denied(Reason::UnsupportedOperation)
        );
    }
}

#[test]
fn typed_refusals_and_existing_codec_preflight_are_preserved() {
    let mut request = Request::new(
        Frontend::RetroWin95,
        Operation::ExportFlat,
        Family::Png,
        Platform::Native,
    );
    assert_eq!(
        admission(request),
        Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
    );
    assert_eq!(
        admission(request)
            .require_allowed(request)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    request.backend_available = false;
    assert_eq!(
        admission(request),
        Decision::TemporarilyUnavailable(Reason::BackendUnavailable)
    );
    request.backend_available = true;
    request.variant = Some("not-qualified");
    assert_eq!(
        admission(request),
        Decision::TemporarilyUnavailable(Reason::VariantUnqualified)
    );
    request.variant = None;
    request.extension = Some("jpg");
    assert_eq!(
        admission(request),
        Decision::Denied(Reason::ExtensionMismatch)
    );
    request.extension = None;
    request.operation = Operation::Open;
    request.needs_preflight = true;
    assert_eq!(
        admission(request),
        Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
    );
    let denied = Request::new(
        Frontend::RetroWin95,
        Operation::Open,
        Family::Jpeg,
        Platform::Native,
    );
    let error = admission(denied).require_allowed(denied).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert!(error.to_string().contains("unsupported_frontend_format"));
}

#[test]
fn actual_pcnd_png_and_bmp_bytes_reopen_with_exact_opaque_pixels() {
    let document = paintcraft_document::import_image(2, 2, &RGBA, "retro-profile-golden").unwrap();
    for operation in [Operation::SavePreserving, Operation::SaveAsCopy] {
        admission(Request::new(
            Frontend::RetroWin95,
            operation,
            Family::Pcnd,
            Platform::Native,
        ))
        .require_allowed(Request::new(
            Frontend::RetroWin95,
            operation,
            Family::Pcnd,
            Platform::Native,
        ))
        .unwrap();
    }
    let bytes = pcnd::write_compact(&document).unwrap();
    assert_eq!(
        admit_open(Frontend::RetroWin95, "image.pcnd", &bytes, Platform::Native).unwrap(),
        Family::Pcnd
    );
    let reopened = pcnd::read(&bytes).unwrap();
    assert!(document.same_semantics(&reopened));
    for (format, filename, family) in [
        (ImageFormat::Png, "image.png", Family::Png),
        (ImageFormat::Bmp, "image.bmp", Family::Bmp),
    ] {
        let encoded = image::encode(format, 2, 2, &RGBA, 90).unwrap();
        for platform in [Platform::Native, Platform::Web] {
            assert_eq!(
                admit_open(Frontend::RetroWin95, filename, &encoded, platform).unwrap(),
                family
            );
        }
        let decoded = image::decode(&encoded).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(decoded.rgba, RGBA);
        let converted =
            paintcraft_document::import_image(2, 2, &decoded.rgba, "retro-reopened").unwrap();
        let native = pcnd::write_compact(&converted).unwrap();
        let native = pcnd::read(&native).unwrap();
        let layer = &native.layers[&native.roots[0]];
        for y in 0..2 {
            for x in 0..2 {
                let i = ((y * 2 + x) * 4) as usize;
                assert_eq!(layer.raster.get(x, y).0.as_slice(), &RGBA[i..i + 4]);
            }
        }
    }
}
