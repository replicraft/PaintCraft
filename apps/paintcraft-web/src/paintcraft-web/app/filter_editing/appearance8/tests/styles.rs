use super::*;
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode, StyleDescriptor};
use paintcraft_document::{StyleInstance, StyleStack};

#[test]
fn stored_style_refuses_after_paid_root_walk_before_source_encoding_or_worker_grant() {
    for enabled in [false, true] {
        for opacity in [0, 128] {
            let (budget, factory, released) = factory(1 << 20);
            let original = factory.scope().clone();
            let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
                original.clone(),
                1 << 20,
                operation::WORK,
            );
            let mut doc = paintcraft_document::new_document(
                2,
                1,
                Some(paintcraft_core::types::Rgba::TRANSPARENT),
                1,
                paintcraft_document::POLICY,
            )
            .unwrap();
            let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
            layer.styles = StyleStack::new(vec![
                StyleInstance::new(
                    uuid::Uuid::from_u128(3),
                    enabled,
                    StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                        [220, 40, 0],
                        opacity,
                        EffectBlendMode::Normal,
                    )),
                )
                .unwrap(),
            ])
            .unwrap();
            let identity = capture::Identity8 {
                owner: uuid::Uuid::from_u128(4),
                ticket: paintcraft_editor::session::Ticket { state_id: doc.state_id, publication_epoch: 17 },
                document: Arc::new(doc), draft: uuid::Uuid::from_u128(5),
                profile: paintcraft_web_render_worker::blend_service::LayerBlendService::compiled_profile().unwrap(),
            };
            let key = capture::Key8 {
                identity,
                parent: paintcraft_render::spatial::with_workspace(
                    parent.clone(),
                    operation::Parent8::capture,
                )
                .unwrap(),
            };
            assert!(snapshot::Preparation8::new(key, factory, &|| Ok(())).is_err());
            // Existing preparation, two cancellation steps, cardinality, one
            // root and zero pairs. No substitute counter or encoded snapshot.
            assert_eq!(parent.work_used(), 81920 + 2 * 1024 + 64 + 4096);
            assert_eq!(released.get(), 1);
            assert_eq!(original.footprint().live, 0);
            drop(parent);
            drop(original);
            assert_eq!(budget.usage().reserved, 0);
        }
    }
}
