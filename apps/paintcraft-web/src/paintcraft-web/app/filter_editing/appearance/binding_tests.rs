//! Actual bound Render owners constrain both source and real staged candidates.
use super::*;
use crate::app::filter_editing::render::Capture;
use paintcraft_commands::{FilterOp, FilterRef, LayerRef};
use paintcraft_core::types::{
    ErrorCode,
    filters::{AddNoiseRgbV1, Descriptor, LevelsRgbV1},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack, LayerMask};
use paintcraft_editor::session::Ticket;
use paintcraft_raster::{
    Admission,
    coverage_mask::{A8Mask, Limits},
};
use paintcraft_render::{
    noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace},
    spatial::{WorkspaceAdmission, with_workspace},
};
use std::sync::Arc;
use uuid::Uuid;
const LEVEL: Uuid = Uuid::from_u128(303);
fn levels(gamma: i16) -> Descriptor {
    Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, gamma, 0, 255).unwrap())
}
fn set_filters(document: &mut Document, paired: bool, gamma: i16) {
    let mut filters = vec![FilterInstance::new(LEVEL, true, levels(gamma))];
    if paired {
        filters.push(FilterInstance::new(
            Uuid::from_u128(304),
            false,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        ));
    }
    document.layers.get_mut(&document.roots[0]).unwrap().filters =
        FilterStack::new(filters).unwrap();
}
fn source(paired: bool) -> Arc<Document> {
    let mut document =
        paintcraft_document::import_image(2, 1, &[17, 31, 47, 0, 64, 16, 128, 255], "bound7")
            .unwrap();
    let mask = A8Mask::uniform(
        document.canvas_rect(),
        127,
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    document.layers.get_mut(&document.roots[0]).unwrap().mask = Some(LayerMask::new(false, mask));
    set_filters(&mut document, paired, 100);
    Arc::new(document)
}
enum Frozen {
    Base(WorkspaceAdmission),
    Noise(NoiseAdmission),
}
impl Frozen {
    fn new(document: &Document, budget: &SharedBudget, paired: bool) -> Self {
        let scope = budget.scope(AllocationClass::SharedBudget);
        if paired {
            let root = NoiseAdmission::new_root(
                scope,
                NoiseJobPolicy::BrowserV1,
                NoiseLimits {
                    bytes: 8 << 20,
                    base_work: 268_435_456,
                    noise_work: 9_000_000,
                },
            )
            .unwrap();
            Self::Noise(root.prepare_document(document, &|| Ok(())).unwrap())
        } else {
            Self::Base(
                paintcraft_render::prepared::prepare_document_programs(
                    document,
                    WorkspaceAdmission::new(scope, 8 << 20, 268_435_456),
                    &|| Ok(()),
                )
                .unwrap(),
            )
        }
    }
    fn key(&self, document: Arc<Document>, operation: Option<&FilterOp>) -> key::Key7 {
        let capture = Capture {
            owner: Uuid::from_u128(1),
            ticket: Ticket {
                state_id: document.state_id,
                publication_epoch: 0,
            },
            draft: Uuid::from_u128(2),
            document,
            publication: None,
            spatial: false,
            catalog: true,
            programs: false,
            scoped: matches!(self, Self::Noise(_)),
        };
        match self {
            Self::Base(a) => {
                with_workspace(a.clone(), || key::Key7::new(capture, operation, None)).unwrap()
            }
            Self::Noise(a) => {
                with_noise_workspace(a.clone(), || key::Key7::new(capture, operation, None))
                    .unwrap()
            }
        }
    }
    fn workspace(&self) -> &WorkspaceAdmission {
        match self {
            Self::Base(a) => a,
            Self::Noise(a) => a.workspace(),
        }
    }
}
#[test]
fn frozen_owner_refuses_changed_source_and_actual_staged_missing_key_before_transfer() {
    for paired in [false, true] {
        for changed_source in [false, true] {
            let source = source(paired);
            let render_budget = SharedBudget::new(32 << 20);
            let frozen = Frozen::new(&source, &render_budget, paired);
            assert!(frozen.workspace().has_prepared_programs());
            let storage = render_budget.usage().reserved;
            let used = frozen.workspace().work_used();
            let mut submitted = (*source).clone();
            if changed_source {
                set_filters(&mut submitted, paired, 200);
            }
            let operation = FilterOp::Update {
                layer: LayerRef::Id(source.roots[0]),
                filter: FilterRef::Id(LEVEL),
                descriptor: levels(200),
            };
            let key = frozen.key(
                Arc::new(submitted),
                if changed_source {
                    None
                } else {
                    Some(&operation)
                },
            );
            // Real Some has exited TLS; a copied numerical ceiling cannot replace it.
            let (capture, prefix) = key.prepare_capture(&|| Ok(())).unwrap();
            let metadata = SharedBudget::new(64 << 20);
            let scope = metadata.scope(AllocationClass::PaintMetadata);
            let result = prepare::prepare(
                capture,
                prefix,
                Admission::new(0).with_paint_metadata_scope(scope.clone()),
                &scope,
                &|| Ok(()),
                None,
            );
            assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
            assert_eq!(metadata.usage().reserved, 0);
            assert_eq!(render_budget.usage().reserved, storage); // no remote scratch admission
            assert!(frozen.workspace().work_used() > used); // validation uses captured prefix
            assert!(frozen.workspace().work_used() < frozen.workspace().work_limit()); // no transferred remainder
            assert_eq!(
                source.layers[&source.roots[0]].filters.as_slice()[0].descriptor(),
                levels(100)
            );
            if let Frozen::Noise(a) = &frozen {
                assert_eq!(a.used().unwrap().noise, 0);
            }
        }
    }
}
#[test]
fn known_frozen_key_allows_actual_staging_and_keeps_original_binding_owner() {
    for paired in [false, true] {
        let source = source(paired);
        let render_budget = SharedBudget::new(32 << 20);
        let frozen = Frozen::new(&source, &render_budget, paired);
        let storage = render_budget.usage().reserved;
        let operation = FilterOp::Update {
            layer: LayerRef::Id(source.roots[0]),
            filter: FilterRef::Id(LEVEL),
            descriptor: levels(100),
        };
        let key = frozen.key(source.clone(), Some(&operation));
        let (capture, prefix) = key.prepare_capture(&|| Ok(())).unwrap();
        let metadata = SharedBudget::new(64 << 20);
        let scope = metadata.scope(AllocationClass::PaintMetadata);
        let prepared = prepare::prepare(
            capture,
            prefix,
            Admission::new(0).with_paint_metadata_scope(scope.clone()),
            &scope,
            &|| Ok(()),
            None,
        )
        .unwrap();
        assert_eq!(
            frozen.workspace().work_used(),
            frozen.workspace().work_limit()
        );
        assert!(frozen.workspace().has_prepared_programs());
        assert!(render_budget.usage().reserved > storage);
        drop(prepared);
        assert_eq!(render_budget.usage().reserved, storage);
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(
            source.layers[&source.roots[0]].filters.as_slice()[0].descriptor(),
            levels(100)
        );
    }
}
