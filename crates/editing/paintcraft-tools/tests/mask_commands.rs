use paintcraft_core::types::{ErrorCode, Rect, Rgba, orthogonal::OrthogonalTransform};
use paintcraft_document::{POLICY, new_document};
use paintcraft_raster::{Selection, selection::BinaryMask};
use paintcraft_tools::commands;

#[test]
fn mask_commands_distinguish_empty_crop_from_exact_full_bounds() {
    let document = new_document(32, 32, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    for selected in [false, true] {
        let mask = BinaryMask::uniform(Rect::new(2, 3, 9, 11).unwrap(), selected).unwrap();
        let selection = Selection::Mask(mask);
        let crop = commands::crop_to_selection(&document, selection.clone());
        if selected {
            let crop = crop.unwrap();
            let rect = crop.get("rect").unwrap();
            assert_eq!(rect.get("x").unwrap().as_i64(), Some(2));
            assert_eq!(rect.get("y").unwrap().as_i64(), Some(3));
            assert_eq!(rect.get("width").unwrap().as_i64(), Some(9));
            assert_eq!(rect.get("height").unwrap().as_i64(), Some(11));
        } else {
            assert_eq!(crop.unwrap_err().code, ErrorCode::InputSchema);
        }
        let missing = commands::transform_image(
            selection.clone(),
            None,
            OrthogonalTransform::RotateRight,
            Rgba([0; 4]),
        )
        .unwrap_err();
        assert_eq!(missing.code, ErrorCode::InputSchema);
        let transform = commands::transform_image(
            selection.clone(),
            document.roots.first().copied(),
            OrthogonalTransform::RotateRight,
            Rgba([0; 4]),
        )
        .unwrap();
        assert_eq!(
            transform.get("op").unwrap().as_str(),
            Some("raster.transform_selection")
        );
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

#[test]
fn irregular_crop_uses_sampled_bounds_and_clips_to_the_canvas() {
    use paintcraft_core::types::resources::SharedBudget;
    use paintcraft_raster::selection::MaskLimits;
    let document = new_document(12, 10, None, 0, POLICY).unwrap();
    let budget = SharedBudget::new(1 << 20);
    let mask = BinaryMask::polygon(
        Rect::new(-10, -10, 50, 50).unwrap(),
        &[(-2, 2), (8, 2), (0, 8)],
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    for mask in [mask.clone(), mask.inverted()] {
        let mut points = Vec::new();
        for y in 0..10 {
            for x in 0..12 {
                if mask.coverage(x, y) == 255 {
                    points.push((x, y));
                }
            }
        }
        let xmin = points.iter().map(|p| p.0).min().unwrap();
        let xmax = points.iter().map(|p| p.0).max().unwrap();
        let ymin = points.iter().map(|p| p.1).min().unwrap();
        let ymax = points.iter().map(|p| p.1).max().unwrap();
        let crop = commands::crop_to_selection(&document, Selection::Mask(mask)).unwrap();
        let rect = crop.get("rect").unwrap();
        for (key, expected) in [
            ("x", xmin),
            ("y", ymin),
            ("width", xmax - xmin + 1),
            ("height", ymax - ymin + 1),
        ] {
            assert_eq!(rect.get(key).unwrap().as_i64(), Some(expected));
        }
    }
}

#[test]
fn resolved_mask_default_json_is_unchanged_and_transparent_replacement_is_not_dropped() {
    use paintcraft_core::types::policy::MaskMode;
    use paintcraft_core::types::text::MaskSpan;
    let id = uuid::Uuid::from_u128(1);
    let spans = [MaskSpan {
        x0: 0,
        y: 0,
        x1: 1,
        coverage: 127,
    }];
    let default = commands::mask_operations(id, Rgba([2, 3, 4, 255]), &spans, &|| Ok(())).unwrap();
    let explicit = commands::mask_operations_with_mode(
        id,
        Rgba([2, 3, 4, 255]),
        &spans,
        MaskMode::SourceOver,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(default, explicit);
    assert!(default[0].get("mode").is_none());
    assert!(
        commands::mask_operations(id, Rgba::TRANSPARENT, &spans, &|| Ok(()))
            .unwrap()
            .is_empty()
    );
    let erase = commands::mask_operations_with_mode(
        id,
        Rgba::TRANSPARENT,
        &spans,
        MaskMode::Replace,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(erase.len(), 1);
    assert_eq!(erase[0].get("mode").unwrap().as_str(), Some("replace"));
}

#[test]
fn destination_out_publisher_retains_opaque_eraser_coverage_and_drops_zero_alpha() {
    use paintcraft_core::types::{policy::MaskMode, text::MaskSpan};
    let id = uuid::Uuid::from_u128(1);
    let spans = [MaskSpan {
        x0: 0,
        y: 0,
        x1: 1,
        coverage: 127,
    }];
    let erase = commands::mask_operations_with_mode(
        id,
        Rgba([0, 0, 0, 255]),
        &spans,
        MaskMode::DestinationOut,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        erase[0].get("mode").unwrap().as_str(),
        Some("destination-out")
    );
    assert!(
        commands::mask_operations_with_mode(
            id,
            Rgba::TRANSPARENT,
            &spans,
            MaskMode::DestinationOut,
            &|| Ok(())
        )
        .unwrap()
        .is_empty()
    );
}
