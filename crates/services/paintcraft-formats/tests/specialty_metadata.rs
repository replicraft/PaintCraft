//! Retained source facts cannot silently disappear through specialty exports.
use paintcraft_core::types::{Error, ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, new_document, source_metadata::*};
use paintcraft_editor::session::Session;
use paintcraft_formats::{dcx, dds, export, icns, icons, image, msp, ora, pcx};
fn document(attached: bool) -> Document {
    let doc = new_document(4, 4, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    if !attached {
        return doc;
    }
    let metadata = SourceMetadata::capture(
        SourceDescriptor {
            format: SourceFormat::Png,
            source_model: SourceColorModel::Rgb,
            source_sha256: [7; 32],
            source_bytes: 64,
            selection: None,
            source_dimensions: (4, 4),
            output_dimensions: (4, 4),
            orientation: Some(1),
            orientation_policy: OrientationPolicy::Unmodified,
            resolution: Some(Resolution {
                horizontal: Rational {
                    numerator: 300,
                    denominator: 1,
                },
                vertical: Rational {
                    numerator: 300,
                    denominator: 1,
                },
                unit: ResolutionUnit::Inch,
                raw_unit: Some(2),
            }),
        },
        None,
        BYTES_MAX,
        &SharedBudget::new(BYTES_MAX),
        &|| Ok(()),
    )
    .unwrap();
    doc.with_source_metadata_checked(Some(metadata), &|| Ok(()))
        .unwrap()
}
fn options() -> dds::TextureOptions {
    dds::TextureOptions {
        encoding: dds::Encoding::Rgba8,
        transfer: dds::Transfer::Srgb,
        alpha: dds::SourceAlpha::Straight,
        mips: dds::MipCount::Levels(1),
        filter: dds::MipFilter::Nearest,
        quality: dds::TextureQuality::Normal,
    }
}
fn unsupported(error: Error) {
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert!(error.message.to_lowercase().contains("metadata"));
}
#[test]
fn specialty_flat_routes_refuse_before_any_admission_render_or_encoder_work() {
    let doc = document(true);
    let before = doc.clone();
    let session = Session::new(doc.clone());
    let ticket = session.history_ticket();
    unsupported(
        export::dds::prepare_document_with(
            &doc,
            options(),
            &[],
            |_, _| panic!("metadata must refuse beforeDDSadmission"),
            |_, _| panic!("metadata must refuse beforeDDSrender"),
            &|| Ok(()),
        )
        .unwrap_err(),
    );
    unsupported(
        export::msp::prepare_document_with(
            &doc,
            msp::Version::RowRleV2,
            &[],
            |_, _| panic!("beforeMSPrenderadmission"),
            |_| panic!("beforeMSPencodeadmission"),
            |_, _| panic!("beforeMSPrender"),
            &|| Ok(()),
        )
        .unwrap_err(),
    );
    unsupported(
        export::jxr::prepare_opaque_rgb_checked(
            &doc,
            1024,
            &[],
            |_, _| panic!("beforeJXRhelper/RGBadmission"),
            |_, _| panic!("beforeJXRrender"),
            &|| Ok(()),
        )
        .unwrap_err(),
    );
    unsupported(
        dcx::prepare_document_with(
            &doc,
            pcx::Mode::Rgb24,
            &[],
            dcx::DocumentLimits::default(),
            &|| Ok(()),
            |_| panic!("beforeDCXrender"),
        )
        .unwrap_err(),
    );
    assert!(doc.same_semantics(&before));
    assert_eq!(session.history_ticket(), ticket);
    assert_eq!(
        doc.source_metadata.as_ref().unwrap().digest(),
        before.source_metadata.as_ref().unwrap().digest()
    );
}
#[test]
fn icon_entry_preparation_and_browser_worker_snapshots_cannot_drop_the_attachment() {
    let doc = document(true);
    for kind in [icons::Kind::Icon, icons::Kind::Cursor] {
        unsupported(
            icons::prepare_document(
                &doc,
                kind,
                &[],
                icons::Filter::Nearest,
                icons::WriteLimits::default(),
                &|| Ok(()),
            )
            .unwrap_err(),
        );
    }
    unsupported(icns::prepare_document(&doc, &[], icons::Filter::Nearest, &|| Ok(())).unwrap_err());
    unsupported(dds::web_export::SourcePlan::capture(&doc, options()).unwrap_err());
    unsupported(msp::web_export::SourcePlan::capture(&doc).unwrap_err());
    let plain = document(false);
    let dds = dds::web_export::SourcePlan::capture(&plain, options()).unwrap();
    let msp = msp::web_export::SourcePlan::capture(&plain).unwrap();
    unsupported(dds.snapshot(&doc).unwrap_err());
    unsupported(msp.snapshot(&doc).unwrap_err());
}
#[test]
fn openraster_refuses_before_writing_even_a_zip_header() {
    let doc = document(true);
    let mut sink = b"existing private destination".to_vec();
    let old = sink.clone();
    unsupported(
        ora::write_document_to(&doc, &mut sink, &ora::Limits::default(), &[], &|| Ok(()))
            .unwrap_err(),
    );
    assert_eq!(sink, old);
    unsupported(ora::encode_document(&doc, &ora::Limits::default(), &[], &|| Ok(())).unwrap_err());
}
#[test]
fn no_attachment_baselines_raw_encoders_and_canvas_previews_still_work() {
    let plain = document(false);
    let attached = document(true);
    let pixels = paintcraft_render::render_flat(&attached).unwrap();
    assert_eq!(pixels, paintcraft_render::render_flat(&plain).unwrap());
    assert!(image::encode(image::ImageFormat::Png, 4, 4, &pixels, 90).is_ok());
    assert!(msp::encode_v1_rgba_exact(4, 4, &pixels, &|| Ok(())).is_ok());
    let dds =
        export::dds::prepare_document(&plain, options(), &[], |_, _| Ok(()), &|| Ok(())).unwrap();
    assert!(!dds.encoded().bytes().is_empty());
    let msp = export::msp::prepare_document_with(
        &plain,
        msp::Version::RowRleV2,
        &[],
        |_, _| Ok(()),
        |_| Ok(()),
        |doc, check| paintcraft_render::render_cancellable(doc, check),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!msp.bytes().is_empty());
    assert!(
        dcx::prepare_document(
            &plain,
            pcx::Mode::Rgb24,
            &[],
            dcx::DocumentLimits::default(),
            &|| Ok(())
        )
        .is_ok()
    );
    assert!(ora::encode_document(&plain, &ora::Limits::default(), &[], &|| Ok(())).is_ok());
    let prepared = icons::prepare_document(
        &plain,
        icons::Kind::Icon,
        &[icons::EntrySpec {
            width: 4,
            height: 4,
            encoding: icons::Encoding::Png,
            hotspot: None,
        }],
        icons::Filter::Nearest,
        icons::WriteLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!prepared.encode(&[], &|| Ok(())).unwrap().is_empty());
    assert!(
        icns::prepare_document(
            &plain,
            &[icns::IconType::Retina16],
            icons::Filter::Nearest,
            &|| Ok(())
        )
        .is_ok()
    );
}
