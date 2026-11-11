//! Shared command helper uses captured selection and never edits the document.
use paintcraft_commands::{Batch, Op, ROTATION_BATCH_SCHEMA};
use paintcraft_core::types::{ErrorCode, Rect, Rgba};
use paintcraft_raster::Selection;
use paintcraft_tools::commands::{rotate_image, rotate_source};
#[test]
fn actual_target_choice_closed_batch_and_source_immutability() {
    let d = paintcraft_document::new_document(
        4,
        3,
        Some(Rgba([17, 34, 51, 0])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let before = d.clone();
    let id = d.roots[0];
    let c = (644245094, 858993459);
    for selection in [
        Selection::Inactive,
        Selection::Rect(Rect::new(1, 0, 2, 2).unwrap()),
    ] {
        let operations = rotate_image(&d, selection.clone(), Some(id), c, Rgba([255; 4])).unwrap();
        assert_eq!(operations.len(), 1);
        assert_eq!(
            Batch::schema_for_operations(&operations),
            ROTATION_BATCH_SCHEMA
        );
        let op = operations[0].get("op").unwrap().as_str().unwrap();
        assert_eq!(
            op,
            if selection == Selection::Inactive {
                "canvas.rotate"
            } else {
                "raster.rotate_selection"
            }
        );
    }
    assert_eq!(rotate_source(&d, &Selection::Empty).unwrap(), None);
    assert!(
        rotate_image(&d, Selection::Empty, None, c, Rgba::TRANSPARENT)
            .unwrap()
            .is_empty()
    );
    assert!(
        rotate_image(
            &d,
            Selection::Inactive,
            None,
            (1 << 30, 0),
            Rgba::TRANSPARENT
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        rotate_image(
            &d,
            Selection::Rect(Rect::new(0, 0, 1, 1).unwrap()),
            None,
            c,
            Rgba::TRANSPARENT
        )
        .unwrap_err()
        .code,
        ErrorCode::ReferenceUnknownId
    );
    assert_eq!(
        rotate_image(&d, Selection::Inactive, None, (0, 0), Rgba::TRANSPARENT)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert!(d.same_semantics(&before));
    // The helper exposes ordinary typed operations, not a new execution authority.
    let _typed = Op::CanvasRotate { coefficients: c };
}
