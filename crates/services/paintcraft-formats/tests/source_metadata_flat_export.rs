use paintcraft_core::types::ErrorCode;
use paintcraft_formats::{export, image::ImageFormat};

#[test]
fn retained_source_metadata_cannot_be_implicitly_acknowledged_by_flat_export_helpers() {
    use paintcraft_document::source_metadata::{
        BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
        SourceMetadata,
    };
    let budget = paintcraft_core::types::resources::SharedBudget::new(BYTES_MAX);
    let mut document =
        paintcraft_document::new_document(8, 8, None, 0, paintcraft_document::POLICY).unwrap();
    document.source_metadata = Some(
        SourceMetadata::capture(
            SourceDescriptor {
                format: SourceFormat::Png,
                source_model: SourceColorModel::Rgb,
                source_sha256: [3; 32],
                source_bytes: 4096,
                selection: None,
                source_dimensions: (8, 8),
                output_dimensions: (8, 8),
                orientation: None,
                orientation_policy: OrientationPolicy::Unmodified,
                resolution: None,
            },
            None,
            BYTES_MAX,
            &budget,
            &|| Ok(()),
        )
        .unwrap(),
    );
    let before = document.clone();
    let allowed = export::KNOWN_LOSSES.map(str::to_owned);
    for format in [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Bmp,
        ImageFormat::Msp,
    ] {
        let options = export::FlatOptions::new(format, 90);
        let error = export::export_flat_options_with(&document, options, &allowed, |_| {
            panic!("retained metadata needs its own explicit policy before rendering")
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains("explicit source metadata"));
        assert!(document.same_semantics(&before));
    }
}
