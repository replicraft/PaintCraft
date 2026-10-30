//! Explicit sealed copy authority is independent of automatically known losses.
use paintcraft_core::types::{ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_document::{
    Document, FilterInstance, FilterStack,
    source_metadata::{
        BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
        SourceMetadata,
    },
};
use paintcraft_formats::{
    dcx, dds,
    export::{self, CopyPolicies, FlatOptions, filter_bake, metadata_policy},
    icns, icons,
    image::{self, ImageFormat},
    msp, pcnd, pcx,
    registry::{Family, Operation},
};
use uuid::Uuid;
fn doc(enabled: bool, visible: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        4,
        4,
        Some(Rgba([0, 0, 0, 255])),
        7,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.visible = visible;
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(1),
        enabled,
        paintcraft_core::types::filters::Descriptor::InvertRgbV1,
    )])
    .unwrap();
    doc.source_metadata = Some(
        SourceMetadata::capture(
            SourceDescriptor {
                format: SourceFormat::Png,
                source_model: SourceColorModel::Rgb,
                source_sha256: [3; 32],
                source_bytes: 64,
                selection: None,
                source_dimensions: doc.canvas,
                output_dimensions: doc.canvas,
                orientation: Some(1),
                orientation_policy: OrientationPolicy::Unmodified,
                resolution: None,
            },
            None,
            BYTES_MAX,
            &SharedBudget::new(BYTES_MAX),
            &|| Ok(()),
        )
        .unwrap(),
    );
    doc
}
fn target(family: Family) -> metadata_policy::Target {
    metadata_policy::Target::new(
        family,
        if matches!(family, Family::Ico | Family::Cur | Family::Icns) {
            Operation::ExportAsset
        } else {
            Operation::ExportFlat
        },
    )
}
fn policies(doc: &Document, family: Family, bake: bool) -> CopyPolicies {
    let target = target(family);
    CopyPolicies {
        source_metadata: metadata_policy::Policy::capture(
            doc,
            target,
            metadata_policy::Intent::Omit,
        )
        .unwrap(),
        filter_bake: if bake {
            filter_bake::Policy::capture(doc, target, filter_bake::Intent::Bake, &|| Ok(()))
                .unwrap()
        } else {
            Default::default()
        },
    }
}
fn allowed() -> Vec<String> {
    export::KNOWN_LOSSES.iter().map(|s| s.to_string()).collect()
}
fn texture() -> dds::TextureOptions {
    dds::TextureOptions {
        encoding: dds::Encoding::Rgba8,
        transfer: dds::Transfer::Srgb,
        alpha: dds::SourceAlpha::Straight,
        mips: dds::MipCount::Levels(1),
        filter: dds::MipFilter::Nearest,
        quality: dds::TextureQuality::Normal,
    }
}
fn losses(codes: &[String]) {
    for code in [filter_bake::BAKED_REASON, metadata_policy::OMITTED_REASON] {
        assert_eq!(
            codes.iter().filter(|v| v.as_str() == code).count(),
            1,
            "{codes:?}"
        );
    }
}
#[test]
fn all_known_losses_and_metadata_omit_cannot_authorize_any_legacy_document_wrapper() {
    for (enabled, visible) in [(true, true), (false, true), (true, false)] {
        let doc = doc(enabled, visible);
        let before = pcnd::write_compact(&doc).unwrap();
        let all = allowed();
        let mut options = FlatOptions::new(ImageFormat::Png, 90);
        options.source_metadata = policies(&doc, Family::Png, false).source_metadata;
        assert_eq!(
            export::export_flat_options_with(&doc, options, &all, |_| panic!(
                "must refuse before render"
            ))
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            export::dds::prepare_document_with_metadata_policy(
                &doc,
                texture(),
                &all,
                |_, _| panic!("admission must not run"),
                |_, _| panic!("render must not run"),
                &|| Ok(()),
                policies(&doc, Family::Dds, false).source_metadata
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            export::msp::prepare_document_with_metadata_policy(
                &doc,
                msp::Version::RowRleV2,
                &all,
                |_, _| panic!("admission"),
                |_| panic!("encode admission"),
                |_, _| panic!("render"),
                &|| Ok(()),
                policies(&doc, Family::Msp, false).source_metadata
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            export::jxr::prepare_opaque_rgb_with_metadata_policy(
                &doc,
                1 << 20,
                &all,
                |_, _| panic!("admission"),
                |_, _| panic!("render"),
                &|| Ok(()),
                policies(&doc, Family::Jxr, false).source_metadata
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        let specs = [icons::EntrySpec {
            width: 4,
            height: 4,
            encoding: icons::Encoding::Png,
            hotspot: None,
        }];
        assert_eq!(
            icons::prepare_document_with_metadata_policy(
                &doc,
                icons::Kind::Icon,
                &specs,
                icons::Filter::Nearest,
                icons::WriteLimits::default(),
                &|| Ok(()),
                policies(&doc, Family::Ico, false).source_metadata
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            icns::prepare_document_with_metadata_policy(
                &doc,
                &[icns::IconType::Retina16],
                icons::Filter::Nearest,
                &|| Ok(()),
                policies(&doc, Family::Icns, false).source_metadata
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            dcx::prepare_document_with_metadata_policy(
                &doc,
                pcx::Mode::Rgb24,
                &all,
                dcx::DocumentLimits::default(),
                &|| Ok(()),
                |_| panic!("render"),
                policies(&doc, Family::Dcx, false).source_metadata
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(pcnd::write_compact(&doc).unwrap(), before);
    }
}
#[test]
fn actual_flat_copies_name_disabled_hidden_and_active_intention_loss_without_mutation() {
    for (enabled, visible, pixel) in [
        (true, true, [255; 4]),
        (false, true, [0, 0, 0, 255]),
        (true, false, [0; 4]),
    ] {
        let doc = doc(enabled, visible);
        let before = pcnd::write_compact(&doc).unwrap();
        let captured = policies(&doc, Family::Png, true);
        let mut options = FlatOptions::new(ImageFormat::Png, 90);
        options.source_metadata = captured.source_metadata;
        options.filter_bake = captured.filter_bake;
        let flat = export::export_flat_options(&doc, options, &[]).unwrap();
        losses(&flat.losses);
        assert_eq!(image::decode(&flat.bytes).unwrap().rgba, pixel.repeat(16));
        assert_eq!(pcnd::write_compact(&doc).unwrap(), before);
    }
}
#[test]
fn every_specialty_producer_uses_the_frozen_policy_and_preserves_exact_source_bytes() {
    let doc = doc(true, true);
    let before = pcnd::write_compact(&doc).unwrap();
    let render = |doc: &Document, check: &dyn Fn() -> paintcraft_core::types::Result<()>| {
        paintcraft_render::render_cancellable(doc, check)
    };
    let dds = export::dds::prepare_document_with_policies(
        &doc,
        texture(),
        &[],
        |_, _| Ok(()),
        render,
        &|| Ok(()),
        policies(&doc, Family::Dds, true),
    )
    .unwrap();
    losses(dds.losses());
    assert_eq!(
        image::decode(dds.encoded().bytes()).unwrap().rgba,
        vec![255; 64]
    );
    let msp = export::msp::prepare_document_with_policies(
        &doc,
        msp::Version::RowRleV2,
        &[],
        |_, _| Ok(()),
        |_| Ok(()),
        render,
        &|| Ok(()),
        policies(&doc, Family::Msp, true),
    )
    .unwrap();
    losses(msp.losses());
    assert_eq!(image::decode(msp.bytes()).unwrap().rgba, vec![255; 64]);
    let rgb = export::jxr::prepare_opaque_rgb_with_policies(
        &doc,
        1 << 20,
        &[],
        |_, _| Ok(()),
        render,
        &|| Ok(()),
        policies(&doc, Family::Jxr, true),
    )
    .unwrap();
    losses(rgb.losses());
    assert_eq!(rgb.rgb(), vec![255; 48]);
    for (kind, family, hotspot) in [
        (icons::Kind::Icon, Family::Ico, None),
        (icons::Kind::Cursor, Family::Cur, Some((0, 0))),
    ] {
        let specs = [icons::EntrySpec {
            width: 4,
            height: 4,
            encoding: icons::Encoding::Png,
            hotspot,
        }];
        let icon = icons::prepare_document_with_policies(
            &doc,
            kind,
            &specs,
            icons::Filter::Nearest,
            icons::WriteLimits::default(),
            &|| Ok(()),
            policies(&doc, family, true),
        )
        .unwrap();
        losses(icon.losses());
        assert_eq!(icon.entries()[0].rgba(), vec![255; 64]);
        let bytes = icon.encode(&[], &|| Ok(())).unwrap();
        assert_eq!(
            icons::decode_entry(&bytes, 0, 16).unwrap().rgba,
            vec![255; 64]
        );
    }
    let icon = icns::prepare_document_with_policies(
        &doc,
        &[icns::IconType::Retina16],
        icons::Filter::Nearest,
        &|| Ok(()),
        policies(&doc, Family::Icns, true),
    )
    .unwrap();
    losses(icon.losses());
    assert_eq!(icon.entries()[0].rgba(), vec![255; 32 * 32 * 4]);
    let bytes = icon.encode(&["icon-resized".into()], &|| Ok(())).unwrap();
    assert_eq!(
        icns::decode_entry(&bytes, 0, 1024, &|| Ok(()))
            .unwrap()
            .rgba,
        vec![255; 32 * 32 * 4]
    );
    let dcx = dcx::prepare_document_with_policies(
        &doc,
        pcx::Mode::Rgb24,
        &[],
        dcx::DocumentLimits::default(),
        &|| Ok(()),
        paintcraft_render::render_flat,
        policies(&doc, Family::Dcx, true),
    )
    .unwrap();
    losses(dcx.losses());
    assert_eq!(
        dcx::decode_page(dcx.bytes(), 0, 16).unwrap().rgba,
        vec![255; 64]
    );
    assert_eq!(pcnd::write_compact(&doc).unwrap(), before);
}
#[test]
fn bake_authority_does_not_override_other_losses_or_target_binding() {
    let mut doc = doc(true, true);
    let mut other = doc.layers[&doc.roots[0]].clone();
    other.id = Uuid::from_u128(40);
    doc.roots.push(other.id);
    doc.layers.insert(other.id, other);
    let mut options = FlatOptions::new(ImageFormat::Png, 90);
    let captured = policies(&doc, Family::Png, true);
    options.source_metadata = captured.source_metadata;
    options.filter_bake = captured.filter_bake;
    assert_eq!(
        export::export_flat_options(&doc, options, &[])
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let flat = export::export_flat_options(&doc, options, &["flatten".into()]).unwrap();
    assert!(flat.losses.iter().any(|v| v == "flatten"));
    losses(&flat.losses);
    assert_eq!(
        export::dds::prepare_document_with_policies(
            &doc,
            texture(),
            &allowed(),
            |_, _| panic!("wrong target must refuse before admission"),
            |_, _| panic!("render"),
            &|| Ok(()),
            captured
        )
        .unwrap_err()
        .code,
        ErrorCode::ConflictStaleBase
    );
}
