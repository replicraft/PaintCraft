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

#[test]
fn erasing_uses_a_replacement_command_with_actual_engine_mask_gating() {
    let doc = new_document(32, 32, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let domain = Rect::new(2, 3, 9, 11).unwrap();
    let full = Selection::Mask(BinaryMask::uniform(domain, true).unwrap());
    let empty = Selection::Mask(BinaryMask::uniform(domain, false).unwrap());
    assert!(
        commands::erase_selection(empty, None, Rgba([255; 4]))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        commands::erase_selection(full.clone(), None, Rgba([255; 4]))
            .unwrap_err()
            .code,
        ErrorCode::ReferenceUnknownId
    );
    let commands = commands::erase_selection(full, Some(id), Rgba([255; 4])).unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0].get("op").unwrap().as_str(),
        Some("raster.fill_rect")
    );
    assert_eq!(commands[0].get("mode").unwrap().as_str(), Some("replace"));
    assert_eq!(
        commands[0]
            .get("rect")
            .unwrap()
            .get("width")
            .unwrap()
            .as_i64(),
        Some(9)
    );
}
