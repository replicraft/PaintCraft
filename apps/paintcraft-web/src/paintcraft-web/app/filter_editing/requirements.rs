//! Exact profile requirements without cloning pixels or staging a document.
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use paintcraft_formats::export::prepared_capabilities::Requirements;

pub(super) struct Bindings {
    pub source: Requirements,
    pub candidate: Requirements,
    pub union: Requirements,
}

pub(super) fn capture(doc: &Document, operation: Option<&FilterOp>) -> Result<Bindings> {
    let source = Requirements::capture(doc, &|| Ok(()))?;
    let Some(operation) = operation else {
        return Ok(Bindings {
            candidate: source.clone(),
            union: source.clone(),
            source,
        });
    };
    let (layer, filter) = operation.stable_uuid_refs().ok_or_else(stale)?;
    let target = doc.layers.get(&layer).ok_or_else(stale)?;
    if filter.is_some_and(|id| !target.filters.iter().any(|item| item.id() == id)) {
        return Err(stale());
    }
    let replaces = matches!(operation, FilterOp::Update { .. } | FilterOp::Remove { .. });
    let mut candidate = Requirements::default();
    for (id, value) in &doc.layers {
        for item in value.filters.iter() {
            if replaces && *id == layer && filter == Some(item.id()) {
                continue;
            }
            candidate.insert(item.descriptor().id(), "params-v1")?;
        }
    }
    if let FilterOp::Add { descriptor, .. } | FilterOp::Update { descriptor, .. } = operation {
        // Decode the actual canonical parameters before negotiating execution.
        let registry = paintcraft_formats::export::prepared_capabilities::resolve(
            descriptor.id(),
            "params-v1",
        )?;
        let (bytes, length) = descriptor.semantic_parameter_bytes();
        if registry.decode(&bytes[..length])? != *descriptor {
            return Err(stale());
        }
        candidate.insert(descriptor.id(), "params-v1")?;
    }
    let union = source.union(&candidate, &|| Ok(()))?;
    Ok(Bindings {
        source,
        candidate,
        union,
    })
}

fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "Filter requirement target changed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_commands::{FilterRef, LayerRef};
    use paintcraft_core::types::filters::{Descriptor, ExposureSrgbV1};
    use paintcraft_document::{FilterInstance, FilterStack};
    use uuid::Uuid;

    #[test]
    fn removing_last_disabled_default_keeps_immutable_source_requirement() {
        let mut doc =
            paintcraft_document::import_image(1, 1, &[1, 2, 3, 255], "requirements").unwrap();
        let layer = doc.roots[0];
        let id = Uuid::from_u128(1);
        doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
            id,
            false,
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(0).unwrap()),
        )])
        .unwrap();
        let before = paintcraft_formats::pcnd::write(&doc).unwrap();
        let bindings = capture(
            &doc,
            Some(&FilterOp::Remove {
                layer: LayerRef::Id(layer),
                filter: FilterRef::Id(id),
            }),
        )
        .unwrap();
        assert_eq!(bindings.source.len(), 1);
        assert_eq!(bindings.candidate.len(), 0);
        assert_eq!(bindings.union.len(), 1);
        assert_eq!(
            bindings.source.digests(&|| Ok(())).unwrap(),
            bindings.union.digests(&|| Ok(())).unwrap()
        );
        assert_eq!(paintcraft_formats::pcnd::write(&doc).unwrap(), before);
    }

    #[test]
    fn update_drops_only_last_old_kind_and_rejects_changing_target() {
        let mut doc =
            paintcraft_document::import_image(1, 1, &[1, 2, 3, 255], "requirements").unwrap();
        let layer = doc.roots[0];
        let id = Uuid::from_u128(1);
        doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(id, true, Descriptor::InvertRgbV1),
            FilterInstance::new(Uuid::from_u128(2), false, Descriptor::InvertRgbV1),
        ])
        .unwrap();
        let operation = FilterOp::Update {
            layer: LayerRef::Id(layer),
            filter: FilterRef::Id(id),
            descriptor: Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(-800).unwrap()),
        };
        let binding = capture(&doc, Some(&operation)).unwrap();
        assert_eq!(
            binding.candidate.pairs().collect::<Vec<_>>(),
            vec![
                ("exposure-srgb-v1", "params-v1"),
                ("invert-rgb-v1", "params-v1"),
            ]
        );
        assert_eq!(binding.union.len(), 2);
        let mut operation = operation;
        if let FilterOp::Update { filter, .. } = &mut operation {
            *filter = FilterRef::Id(Uuid::from_u128(999));
        }
        assert_eq!(
            capture(&doc, Some(&operation)).err().unwrap().code,
            ErrorCode::ConflictStaleBase
        );
    }
}
