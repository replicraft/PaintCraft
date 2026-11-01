//! One persistence policy for sessions owned by CLI commands and MCP.
use paintcraft_document::Document;
use paintcraft_editor::session::Session;
use paintcraft_formats::pcnd;

/// Check newly authored content at publication. Initial input admission remains
/// with the reader/converter; existing snapshots and history stay navigable.
pub(super) fn session(document: Document) -> Session {
    Session::new(document).with_persistent_validator(Some(pcnd::check_compact_metadata_admission))
}

#[cfg(test)]
mod layer_mask_wire_boundary_tests {
    use super::*;
    use paintcraft_core::types::{ErrorCode, Rect, Rgba, resources::SharedBudget};
    use paintcraft_document::{
        FilterInstance, FilterMask, FilterStack, LayerMask, POLICY,
        source_metadata::{BYTES_MAX, SourceMetadata, WIRE_HEADER_BYTES},
    };
    use paintcraft_raster::coverage_mask::A8Mask;
    #[test]
    fn actual_host_validator_counts_both_mask_families_at_exact_eight_mib_before_publish() {
        let source=pcnd::read(include_bytes!("../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd")).unwrap();
        let source = source.source_metadata.unwrap();
        let descriptor = *source.descriptor();
        let coverage = SharedBudget::new(8192);
        let field = A8Mask::uniform(
            Rect::new(0, 0, 2, 1).unwrap(),
            255,
            &coverage,
            Default::default(),
            &|| Ok(()),
        )
        .unwrap();
        let mut template =
            paintcraft_document::new_document(2, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
        let id = template.roots[0];
        template.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(
                uuid::Uuid::from_u128(1),
                false,
                paintcraft_core::types::filters::Descriptor::InvertRgbV1,
            )
            .with_mask(Some(FilterMask::new(false, field.clone()))),
        ])
        .unwrap();
        template.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(true, field));
        let framing = pcnd::write(&template).unwrap().len() - 32 + WIRE_HEADER_BYTES;
        let exact = (8 << 20) - framing;
        for extra in [0, 1] {
            let mut profile = source.profile().unwrap().to_vec();
            profile.resize(exact + extra, 0);
            let len = profile.len() as u32;
            profile[..4].copy_from_slice(&len.to_be_bytes());
            let realm = SharedBudget::new(BYTES_MAX + 4096);
            let metadata =
                SourceMetadata::capture(descriptor, Some(&profile), BYTES_MAX, &realm, &|| Ok(()))
                    .unwrap();
            let mut initial = template.clone();
            initial.layers.get_mut(&id).unwrap().mask = None;
            initial.source_metadata = Some(metadata);
            pcnd::check_compact_metadata_admission(&initial, &|| Ok(())).unwrap();
            let mut host = session(initial);
            let before = host.snapshot();
            let ticket = host.ticket();
            let operations = serde_json::json!({"schema":"paintcraft.batch/layer-appearance-v1","atomic":true,"policy":POLICY,
                "base":{"state_id":ticket.state_id.to_string(),"publication_epoch":ticket.publication_epoch.to_string()},
                "operations":[{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}},
                    {"op":"layer.mask.from_selection","layer_ref":{"id":id.to_string()},"enabled":true}]});
            let value =
                paintcraft_core::types::json::parse(&serde_json::to_vec(&operations).unwrap())
                    .unwrap();
            let batch = paintcraft_commands::Batch::from_generated_value(value).unwrap();
            let result = host.apply(&batch);
            if extra == 0 {
                assert!(result.unwrap().committed);
                let bytes = pcnd::write_compact(host.document()).unwrap();
                assert_eq!(bytes.len(), (8 << 20) + 32);
                assert_eq!(bytes[4], 7);
                assert_eq!(
                    pcnd::coverage_facts(host.document(), &|| Ok(()))
                        .unwrap()
                        .masks,
                    2
                );
            } else {
                assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
                assert_eq!(host.ticket(), ticket);
                assert!(std::sync::Arc::ptr_eq(&before, &host.snapshot()));
                assert!(host.document().layers[&id].mask.is_none());
                assert!(matches!(
                    host.selection(),
                    paintcraft_raster::Selection::Inactive
                ));
            }
        }
    }
}
