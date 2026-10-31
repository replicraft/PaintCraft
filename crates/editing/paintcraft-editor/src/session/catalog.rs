//! Requirements describe retained numerical intent, independent of file codec.
use super::*;

pub fn catalog_document_requirements(doc: &Document) -> Vec<&'static str> {
    if !doc.layers.values().any(|layer| {
        layer
            .filters
            .iter()
            .any(|filter| filter.descriptor().is_catalog_only())
    }) {
        return Vec::new();
    }
    doc.layers
        .values()
        .flat_map(|layer| layer.filters.iter().map(|filter| filter.descriptor().id()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
