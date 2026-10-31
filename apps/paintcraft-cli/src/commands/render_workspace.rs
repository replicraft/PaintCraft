//! Explicit CLI intermediate workspace policy, independent of output/canvas caps.
//! The provisional 64 MiB process realm is an owned-allocation declaration, not
//! an RSS or all-resource limit. Work resets per command, never per layer/render.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
use std::sync::OnceLock;
const BYTES: u64 = 64 << 20;
const WORK: u64 = 1_000_000_000;
fn realm() -> &'static SharedBudget {
    static REALM: OnceLock<SharedBudget> = OnceLock::new();
    REALM.get_or_init(|| SharedBudget::new(BYTES))
}
fn in_realm<T>(
    budget: &SharedBudget,
    work_limit: u64,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let admission = WorkspaceAdmission::new(
        budget.scope(AllocationClass::SharedBudget),
        BYTES,
        work_limit,
    );
    with_workspace(admission, work)
}
pub(super) fn run<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    in_realm(realm(), WORK, work)
}
pub(crate) fn command(json: bool, work: impl FnOnce() -> Reply) -> Reply {
    match run(|| Ok(work())) {
        Ok(reply) => reply,
        Err(error) => bare(json, Err(error)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::{
        Rect,
        filters::{BoxBlurV1, Descriptor},
    };
    use paintcraft_document::{FilterInstance, FilterStack};
    fn document() -> Document {
        let mut doc = new_document(2, 1, Some(Rgba([20, 115, 200, 255])), 18, POLICY).unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(8),
                true,
                Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
            )])
            .unwrap();
        doc
    }
    #[test]
    fn command_work_accumulates_across_renders_and_resets_only_next_command() {
        let doc = document();
        let cost = paintcraft_render::spatial::quote_layer(
            &doc.layers[&doc.roots[0]],
            Rect::new(0, 0, 2, 1).unwrap(),
            &|| Ok(()),
        )
        .unwrap()
        .work;
        let budget = SharedBudget::new(BYTES);
        let expected = [20, 115, 200, 255, 20, 115, 200, 255];
        in_realm(&budget, cost, || {
            assert_eq!(paintcraft_render::render_flat(&doc)?, expected);
            assert_eq!(
                paintcraft_render::render_flat(&doc).unwrap_err().code,
                ErrorCode::ResourceLimit
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.usage().reserved, 0);
        in_realm(&budget, cost, || {
            assert_eq!(paintcraft_render::render_flat(&doc)?, expected);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            paintcraft_render::render_flat(&doc).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    #[test]
    fn process_realm_is_shared_and_failure_releases_workspace() {
        let doc = document();
        let budget = SharedBudget::new(BYTES);
        let held = budget.reserve(BYTES).unwrap();
        let error = in_realm(&budget, WORK, || paintcraft_render::render_flat(&doc)).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(budget.usage().reserved, BYTES);
        drop(held);
        in_realm(&budget, WORK, || paintcraft_render::render_flat(&doc)).unwrap();
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(realm().usage().limit, BYTES);
    }
}
