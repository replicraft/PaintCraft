//! Explicit MAIN-instance admission for remaining synchronous browser routes.
//! The persistent filter Worker has a separate source-owned reservation. These
//! limits charge transient appearance workspace, not image/GPU storage or RSS.
use paintcraft_core::types::{
    Result,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::Document;
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
use std::sync::OnceLock;

pub(crate) const BYTES_MAX: u64 = 64 << 20;
pub(crate) const WORK_MAX: u64 = 268_435_456;

pub(crate) fn with_document<T>(doc: &Document, work: impl FnOnce() -> Result<T>) -> Result<T> {
    if !doc.layers.values().any(paintcraft_render::spatial::active) {
        return work();
    }
    static MAIN: OnceLock<SharedBudget> = OnceLock::new();
    with_budget(MAIN.get_or_init(|| SharedBudget::new(BYTES_MAX)), work)
}

fn with_budget<T>(budget: &SharedBudget, work: impl FnOnce() -> Result<T>) -> Result<T> {
    // One realm across requests; only the finite work counter resets per job.
    with_workspace(
        WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            BYTES_MAX,
            WORK_MAX,
        ),
        work,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::{
        ErrorCode,
        filters::{BoxBlurV1, Descriptor},
    };
    use paintcraft_document::{FilterInstance, FilterStack};
    use uuid::Uuid;
    fn fixture() -> Document {
        let mut doc =
            paintcraft_document::import_image(1, 1, &[255, 0, 0, 128], "main-boundary").unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                Uuid::from_u128(1),
                true,
                Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
            )])
            .unwrap();
        doc
    }
    #[test]
    fn synchronous_main_routes_require_shared_workspace_release_it_and_preserve_private_text() {
        let doc = fixture();
        let mut editor = crate::editor::Editor::from_document(doc).unwrap();
        let before = editor.session.snapshot();
        let ticket = editor.session.ticket();
        editor.tools.begin_text(editor.target.unwrap(), (0, 0));
        editor.tools.type_text("still editable");
        let stamp = editor.tools.text_stamp();
        let budget = SharedBudget::new(0);
        assert_eq!(
            with_budget(&budget, || paintcraft_render::render_flat(&before))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(editor.tools.text_stamp(), stamp);
        assert_eq!(editor.session.ticket(), ticket);
        assert!(std::sync::Arc::ptr_eq(&before, &editor.session.snapshot()));
        assert_eq!(editor.session.undo_depth(), 0);
        let budget = SharedBudget::new(BYTES_MAX);
        let hold = budget.reserve(BYTES_MAX).unwrap();
        assert_eq!(
            with_budget(&budget, || paintcraft_render::render_flat(&before))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        drop(hold);
        assert_eq!(
            with_budget(&budget, || paintcraft_render::render_flat(&before)).unwrap(),
            [255, 0, 0, 128]
        );
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(
            with_document(&before, || paintcraft_render::render_flat(&before)).unwrap(),
            [255, 0, 0, 128]
        );
    }
}
