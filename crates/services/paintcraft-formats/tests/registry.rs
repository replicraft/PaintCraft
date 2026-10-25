use paintcraft_core::types::ErrorCode;
use paintcraft_formats::registry::*;

const FRONTENDS: [Frontend; 4] = [
    Frontend::ClassicV7,
    Frontend::ClassicV10,
    Frontend::ClassicV11,
    Frontend::Studio,
];

#[test]
fn qualified_discovery_is_separate_from_stock_inventory_and_backend_candidates() {
    assert_eq!(STUDIO_STOCK.len(), 13);
    assert!(!STUDIO_STOCK.contains(&Family::Pcnd));
    assert_eq!(
        enabled_extensions(Frontend::Studio, Operation::Open, Platform::Native),
        vec!["pcnd", "png", "jpg", "jpeg", "jpe", "jfif", "bmp"]
    );
    for platform in [Platform::Native, Platform::Web] {
        for frontend in FRONTENDS {
            assert_eq!(
                enabled(frontend, Operation::Open, platform),
                vec![Family::Pcnd, Family::Png, Family::Jpeg, Family::Bmp]
            );
            assert_eq!(
                enabled(frontend, Operation::ExportFlat, platform),
                vec![Family::Png, Family::Jpeg, Family::Bmp]
            );
            for family in Family::ALL {
                let qualified = capability(*family, Operation::Open, platform).qualified;
                assert_eq!(
                    qualified,
                    matches!(
                        family,
                        Family::Pcnd | Family::Png | Family::Jpeg | Family::Bmp
                    )
                );
            }
        }
    }
}

#[test]
fn classic_does_not_admit_foreign_projects_or_vectors_even_for_import() {
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        for family in [
            Family::Svg,
            Family::Pdn,
            Family::Psd,
            Family::Psb,
            Family::Ora,
            Family::Pdf,
            Family::Tga,
            Family::Dds,
            Family::Avif,
            Family::Jxl,
            Family::Jxr,
            Family::Ico,
        ] {
            for operation in [
                Operation::Open,
                Operation::OpenRaster,
                Operation::ImportConvert,
            ] {
                assert!(
                    matches!(
                        admission(Request::new(frontend, operation, family, Platform::Native)),
                        Decision::Denied(_)
                    ),
                    "{frontend:?}/{operation:?}/{family:?}"
                );
            }
        }
    }
}

#[test]
fn stock_extended_asset_and_native_project_operations_do_not_leak() {
    for family in STUDIO_STOCK {
        let decision = admission(Request::new(
            Frontend::Studio,
            Operation::Open,
            *family,
            Platform::Native,
        ));
        assert!(!matches!(
            decision,
            Decision::Denied(_) | Decision::RequiresExplicitImport
        ));
    }
    for family in [
        Family::Svg,
        Family::Paint,
        Family::Psd,
        Family::Psb,
        Family::Ora,
        Family::Qoi,
    ] {
        assert_eq!(
            admission(Request::new(
                Frontend::Studio,
                Operation::Open,
                family,
                Platform::Native
            )),
            Decision::RequiresExplicitImport
        );
        assert_eq!(
            admission(Request::new(
                Frontend::Studio,
                Operation::ImportConvert,
                family,
                Platform::Native
            )),
            Decision::TemporarilyUnavailable(Reason::BackendUnavailable)
        );
    }
    for family in [Family::Abr, Family::Ase, Family::Csh] {
        assert_eq!(
            admission(Request::new(
                Frontend::Studio,
                Operation::Open,
                family,
                Platform::Native
            )),
            Decision::Denied(Reason::UnsupportedOperation)
        );
        assert!(
            !admission(Request::new(
                Frontend::Studio,
                Operation::ImportAsset,
                family,
                Platform::Native
            ))
            .available()
        );
    }
    assert_eq!(
        admission(Request::new(
            Frontend::Studio,
            Operation::OpenRaster,
            Family::Pcnd,
            Platform::Native
        )),
        Decision::Denied(Reason::UnsupportedOperation)
    );
}

#[test]
fn hard_product_denials_win_over_backend_flags_and_variants() {
    for frontend in FRONTENDS {
        for family in [Family::Heif, Family::Svg] {
            for operation in [
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
                Operation::ExportAsset,
            ] {
                let mut request = Request::new(frontend, operation, family, Platform::Native);
                request.backend_available = true;
                request.variant = Some("installed-encoder");
                assert_eq!(
                    admission(request),
                    Decision::Denied(Reason::UnsupportedOperation)
                );
            }
        }
    }
}

#[test]
fn deployment_and_variant_denial_cannot_be_overridden_by_known_family() {
    let mut request = Request::new(
        Frontend::Studio,
        Operation::Open,
        Family::Png,
        Platform::Web,
    );
    request.backend_available = false;
    assert_eq!(
        admission(request),
        Decision::TemporarilyUnavailable(Reason::BackendUnavailable)
    );
    request.backend_available = true;
    request.variant = Some("float32");
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
    request.family = Family::Bmp;
    request.extension = Some("dib");
    assert_eq!(
        admission(request),
        Decision::TemporarilyUnavailable(Reason::VariantUnqualified)
    );
    for family in [Family::Webp, Family::Heif] {
        assert_eq!(
            admission(Request::new(
                Frontend::ClassicV11,
                Operation::Open,
                family,
                Platform::Native
            )),
            Decision::TemporarilyUnavailable(Reason::VariantUnqualified)
        );
    }
}

#[test]
fn flat_writes_require_semantic_preflight_without_mutating_source() {
    for family in [Family::Png, Family::Bmp, Family::Jpeg] {
        let request = Request::new(
            Frontend::Studio,
            Operation::SaveAsCopy,
            family,
            Platform::Native,
        );
        let decision = admission(request);
        assert_eq!(
            decision,
            Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
        );
        assert!(decision.available());
        assert_eq!(
            decision.require_allowed(request).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let request = Request::new(
        Frontend::Studio,
        Operation::SavePreserving,
        Family::Pcnd,
        Platform::Native,
    );
    assert_eq!(admission(request), Decision::Allowed);
}

#[test]
fn signatures_and_suffixes_never_silently_fall_back() {
    let png = b"\x89PNG\r\n\x1a\n";
    for frontend in FRONTENDS {
        assert_eq!(
            admit_open(frontend, "image.PNG", png, Platform::Native).unwrap(),
            Family::Png
        );
        assert_eq!(
            admit_open(frontend, "image", png, Platform::Web).unwrap(),
            Family::Png
        );
        assert_eq!(
            admit_open(frontend, "image.jpg", png, Platform::Native)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert!(admit_open(frontend, "image.unknown", png, Platform::Native).is_err());
        assert!(admit_open(frontend, "image.png", b"unknown", Platform::Native).is_err());
        assert!(admit_open(frontend, "image.zip", b"PK\x03\x04", Platform::Native).is_err());
        assert_eq!(
            admit_open(frontend, "image.pcnd", b"PK\x03\x04", Platform::Native).unwrap(),
            Family::Pcnd
        );
        assert!(admit_open(frontend, "image.pcnd", b"PK", Platform::Native).is_err());
        // A candidate must still pass the strict decoder; admission is not validation.
        assert!(paintcraft_formats::pcnd::read(b"PK\x03\x04").is_err());
    }
    assert_eq!(
        admit_open(Frontend::ClassicV11, "fake.svg", png, Platform::Native)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        admit_open(
            Frontend::Studio,
            "fake.psd",
            b"8BPS\0\x01",
            Platform::Native
        )
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
fn approved_family_metadata_and_aliases_are_unique_not_support_claims() {
    let mut aliases = std::collections::HashSet::new();
    let mut ids = std::collections::HashSet::new();
    for family in Family::ALL {
        assert!(ids.insert(family.id()));
        for extension in family.extensions() {
            assert!(aliases.insert(*extension));
            assert_eq!(Family::from_extension(extension), Some(*family));
            assert_eq!(
                Family::from_extension(&format!(".{}", extension.to_ascii_uppercase())),
                Some(*family)
            );
        }
    }
    for suffix in [
        "paint", "pdn", "svgz", "psb", "heic", "abr", "ff", "apng", "dcx", "jxr", "jxl",
    ] {
        assert!(Family::from_extension(suffix).is_some());
    }
}

#[test]
fn every_qualified_raster_dispatches_to_an_actual_bounded_adapter() {
    use paintcraft_formats::image::{self, ImageFormat};
    for (family, format, suffix) in [
        (Family::Png, ImageFormat::Png, "png"),
        (Family::Bmp, ImageFormat::Bmp, "bmp"),
        (Family::Jpeg, ImageFormat::Jpeg, "jpg"),
    ] {
        let pixels = [255, 20, 70, 255, 0, 0, 0, 255];
        let bytes = image::encode(format, 2, 1, &pixels, 95).unwrap();
        for frontend in FRONTENDS {
            let candidate = admit_open(
                frontend,
                &format!("actual.{suffix}"),
                &bytes,
                Platform::Native,
            )
            .unwrap();
            assert_eq!(candidate, family);
            let decoded = image::decode_with_pixel_limit(&bytes, 2).unwrap();
            assert_eq!((decoded.width, decoded.height), (2, 1));
            if family != Family::Jpeg {
                assert_eq!(decoded.rgba, pixels);
            }
            assert!(image::decode_with_pixel_limit(&bytes, 1).is_err());
        }
    }
}

#[test]
fn future_studio_workflows_are_explicit_and_unavailable_rather_than_stock_formats() {
    for family in [
        Family::Pnm,
        Family::Farbfeld,
        Family::Icns,
        Family::Raw,
        Family::Pdf,
        Family::Kra,
        Family::Xcf,
        Family::Animation,
    ] {
        assert_eq!(
            admission(Request::new(
                Frontend::Studio,
                Operation::Open,
                family,
                Platform::Native
            )),
            Decision::RequiresExplicitImport
        );
        assert_eq!(
            admission(Request::new(
                Frontend::Studio,
                Operation::ImportConvert,
                family,
                Platform::Native
            )),
            Decision::TemporarilyUnavailable(Reason::BackendUnavailable)
        );
        assert!(
            !enabled(Frontend::Studio, Operation::ImportConvert, Platform::Native)
                .contains(&family)
        );
    }
}

#[test]
fn future_read_only_projects_and_assets_do_not_acquire_unsupported_writers() {
    for family in [Family::Raw, Family::Pdf, Family::Kra, Family::Xcf] {
        assert_eq!(
            admission(Request::new(
                Frontend::Studio,
                Operation::SaveAsCopy,
                family,
                Platform::Native
            )),
            Decision::Denied(Reason::UnsupportedOperation)
        );
    }
    assert_eq!(
        admission(Request::new(
            Frontend::Studio,
            Operation::ImportAsset,
            Family::Icns,
            Platform::Native
        )),
        Decision::Denied(Reason::UnsupportedOperation)
    );
}
