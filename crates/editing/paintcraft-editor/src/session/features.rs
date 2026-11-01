use super::*;

/// Own-layer masks require result/v5, spatial intentions v4, filter masks v3; otherwise
/// preserve the minimal v1/v2 profile.
/// Extensions disclose inherited point-filter semantics in canonical feature order.
pub fn masked_document_features(doc: &Document) -> Vec<&'static str> {
    let catalog = doc.layers.values().any(|layer| {
        layer
            .filters
            .iter()
            .any(|entry| entry.descriptor().is_catalog_only())
    });
    let masks = doc
        .layers
        .values()
        .any(|layer| layer.filters.iter().any(|entry| entry.mask().is_some()));
    let spatial = doc.layers.values().any(|layer| {
        layer
            .filters
            .iter()
            .any(|entry| entry.descriptor().is_spatial())
    });
    let layer_masks = doc.has_layer_masks();
    if !masks && !spatial && !catalog && !layer_masks {
        return Vec::new();
    }
    let has_filters = layer_masks && doc.layers.values().any(|layer| !layer.filters.is_empty());
    let mut features = if has_filters || masks || spatial || catalog {
        vec!["layer-point-filters.v1"]
    } else {
        Vec::new()
    };
    if masks {
        features.push("layer-filter-masks.v1");
    }
    if spatial {
        features.push("layer-spatial-filters.v1");
    }
    if catalog {
        features.push("layer-filter-catalog.v1");
    }
    if layer_masks {
        features.push("layer-masks.v1");
    }
    if doc.source_metadata.is_some() {
        features.push("source-metadata.v1");
    }
    features
}
