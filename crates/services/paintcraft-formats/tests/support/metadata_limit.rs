// Shared test-only capacity specimen, not an original-image provenance oracle.
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, SourceMetadata, source_metadata::BYTES_MAX};

pub fn exact_doc(side: u32, with_layer: bool) -> Document {
    assert!((1..=127).contains(&side));
    let mut document = paintcraft_document::new_document(
        side,
        side,
        with_layer.then_some(Rgba([255; 4])),
        7,
        POLICY,
    )
    .unwrap();
    for layer in document.layers.values_mut() {
        layer.name = "x".into();
    }
    assert!(
        document
            .layers
            .values()
            .all(|layer| layer.raster.resident_tiles() == 0)
    );
    // No raster payload is present: every byte except the trailing checksum
    // counts as metadata. PCND2 adds its fixed source header and original ICC.
    // Independent canonical framing: magic+IDs40, dimensions2, title marker1,
    // root count1, and optional layer29 (one-byte name/origin/dimension fields).
    let overhead = 44 + if with_layer { 29 } else { 0 };
    assert_eq!(
        paintcraft_formats::pcnd::write_compact(&document)
            .unwrap()
            .len(),
        overhead + 32
    );
    let specimen = paintcraft_formats::pcnd::read(include_bytes!(
        "../pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap()
    .source_metadata
    .unwrap();
    let mut profile = specimen.profile().unwrap().to_vec();
    let size = BYTES_MAX as usize - overhead - 192;
    assert!(size >= profile.len());
    profile.resize(size, 0);
    profile[..4].copy_from_slice(&(size as u32).to_be_bytes());
    let mut descriptor = *specimen.descriptor();
    descriptor.source_dimensions = (side, side);
    descriptor.output_dimensions = (side, side);
    let metadata = SourceMetadata::capture(
        descriptor,
        Some(&profile),
        BYTES_MAX,
        &SharedBudget::new(BYTES_MAX + 4096),
        &|| Ok(()),
    )
    .unwrap();
    let document = document
        .with_source_metadata_checked(Some(metadata), &|| Ok(()))
        .unwrap();
    let actual = paintcraft_formats::pcnd::write_compact(&document).unwrap();
    assert_eq!(actual.len(), BYTES_MAX as usize + 32);
    assert!(
        paintcraft_formats::pcnd::read(&actual)
            .unwrap()
            .same_semantics(&document)
    );
    document
}
