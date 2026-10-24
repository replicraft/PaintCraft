use paintcraft_core::types::{ErrorCode, Rect, Rgba, orthogonal::OrthogonalTransform};
use paintcraft_document::{POLICY, new_document};
use paintcraft_raster::{Selection, selection::BinaryMask};
use paintcraft_tools::commands;

#[test]
fn mask_commands_refuse_unsupported_operations_instead_of_using_bounds_or_deselecting() {
    let document = new_document(32, 32, None, 0, POLICY).unwrap();
    for selected in [false, true] {
        let mask = BinaryMask::uniform(Rect::new(2, 3, 9, 11).unwrap(), selected).unwrap();
        let selection = Selection::Mask(mask);
        assert_eq!(
            commands::crop_to_selection(&document, selection.clone())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        for target in [None, document.roots.first().copied()] {
            assert_eq!(
                commands::erase_selection(selection.clone(), target, Rgba([255; 4]))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
            assert_eq!(
                commands::transform_image(
                    selection.clone(),
                    target,
                    OrthogonalTransform::RotateRight,
                    Rgba([0; 4]),
                )
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
}
