//! Explicit backend qualification only. The ordinary ORA route remains flat.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{ora, pcnd, project_import, registry::Family};
use std::cell::Cell;

const GROUPED: &[u8] = include_bytes!("ora-group-fixtures/neutral.ora");
const FLAT: &[u8] = include_bytes!("ora-group-fixtures/flat.ora");
const MERGED: &[u8] = include_bytes!("ora-group-fixtures/merged.rgba");

#[test]
fn neutral_auto_tree_preserves_leaf_order_raw_pixels_offsets_and_effective_visibility() {
    let source = GROUPED.to_vec();
    let frozen = source.clone();
    let converted =
        project_import::decode_ora_with_neutral_groups(&source, 64, &|| Ok(())).unwrap();
    assert_eq!(converted.losses, [ora::GROUP_STRUCTURE_LOSS]);
    assert_eq!(converted.document.title.as_deref(), Some("Source title"));
    assert_eq!(converted.document.canvas, (3, 2));
    let expected = [
        (
            "Back",
            (0, 0),
            true,
            3,
            2,
            include_bytes!("ora-group-fixtures/layer-0.rgba").as_slice(),
        ),
        (
            "Mid",
            (0, 0),
            true,
            3,
            2,
            include_bytes!("ora-group-fixtures/layer-1.rgba").as_slice(),
        ),
        (
            "Child visible but ancestor hidden",
            (0, 0),
            false,
            1,
            1,
            include_bytes!("ora-group-fixtures/layer-2.rgba").as_slice(),
        ),
        (
            "Child hidden",
            (2, 1),
            false,
            1,
            1,
            include_bytes!("ora-group-fixtures/layer-3.rgba").as_slice(),
        ),
        (
            "Bright",
            (-1, 0),
            true,
            2,
            2,
            include_bytes!("ora-group-fixtures/layer-4.rgba").as_slice(),
        ),
        (
            "Fore",
            (1, -1),
            true,
            2,
            2,
            include_bytes!("ora-group-fixtures/layer-5.rgba").as_slice(),
        ),
    ];
    assert_eq!(converted.document.roots.len(), expected.len());
    for (layer, (name, origin, visible, width, height, raw)) in
        converted.document.layers_in_order().zip(expected)
    {
        assert_eq!(layer.name, name);
        assert_eq!(layer.origin, origin);
        assert_eq!(layer.visible, visible);
        assert_eq!(layer.opacity, 255);
        assert_eq!(
            (layer.raster.width(), layer.raster.height()),
            (width, height)
        );
        for (i, pixel) in raw.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                layer
                    .raster
                    .get((i % width as usize) as i64, (i / width as usize) as i64)
                    .0,
                *pixel
            );
        }
    }
    let flat = project_import::decode_ora(FLAT, 64, &|| Ok(())).unwrap();
    assert!(flat.losses.is_empty());
    assert_eq!(
        paintcraft_render::render_flat(&flat.document).unwrap(),
        MERGED
    );
    assert_eq!(
        paintcraft_render::render_flat(&converted.document).unwrap(),
        MERGED
    );
    let restored = pcnd::read(&pcnd::write_compact(&converted.document).unwrap()).unwrap();
    assert!(converted.document.same_semantics(&restored));
    assert_eq!(source, frozen);
}

#[test]
fn default_public_paths_still_refuse_groups_and_existing_flat_identity_is_unchanged() {
    assert_eq!(
        ora::decode_layers(GROUPED, &ora::Limits::default(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        project_import::decode_ora(GROUPED, 64, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        project_import::decode_layered(Family::Ora, GROUPED, 64, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(!ora::LOSSES.contains(&ora::GROUP_STRUCTURE_LOSS));
    assert!(project_import::KNOWN_LOSSES.contains(&ora::GROUP_STRUCTURE_LOSS));
    for bytes in [
        FLAT,
        include_bytes!("ora-fixtures/stored.ora"),
        include_bytes!("ora-fixtures/deflated.ora"),
    ] {
        let old = project_import::decode_ora(bytes, 100, &|| Ok(())).unwrap();
        let new = project_import::decode_ora_with_neutral_groups(bytes, 100, &|| Ok(())).unwrap();
        assert_eq!(old.losses, new.losses);
        assert!(old.document.same_semantics(&new.document));
        assert_eq!(old.document.state_id, new.document.state_id);
    }
}

#[test]
fn default_isolation_near_one_blends_offsets_and_hidden_unknown_semantics_refuse() {
    for bytes in [
        include_bytes!("ora-group-fixtures/default-isolate.ora").as_slice(),
        include_bytes!("ora-group-fixtures/explicit-isolate.ora"),
        include_bytes!("ora-group-fixtures/near-one.ora"),
        include_bytes!("ora-group-fixtures/half-opacity.ora"),
        include_bytes!("ora-group-fixtures/exponent-one.ora"),
        include_bytes!("ora-group-fixtures/group-offset.ora"),
        include_bytes!("ora-group-fixtures/group-blend.ora"),
        include_bytes!("ora-group-fixtures/leaf-blend.ora"),
        include_bytes!("ora-group-fixtures/hidden-isolate.ora"),
        include_bytes!("ora-group-fixtures/hidden-unknown.ora"),
        include_bytes!("ora-group-fixtures/old-version.ora"),
    ] {
        let error =
            project_import::decode_ora_with_neutral_groups(bytes, 64, &|| Ok(())).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported, "{error}");
    }
    // An unsupported tree refuses before any of these deliberately invalid PNG bodies are examined.
    assert_eq!(
        project_import::decode_ora_with_neutral_groups(
            include_bytes!("ora-group-fixtures/policy-before-pixels.ora"),
            64,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
fn valid_different_merged_pixels_are_an_unsupported_projection_not_corruption() {
    let error = project_import::decode_ora_with_neutral_groups(
        include_bytes!("ora-group-fixtures/different-projection.ora"),
        64,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert!(error.message.contains("rendering/projection"));
}

#[test]
fn aggregate_source_pixels_and_every_observed_cancellation_stop_private_conversion() {
    assert_eq!(
        project_import::decode_ora_with_neutral_groups(GROUPED, 21, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let calls = Cell::new(0);
    project_import::decode_ora_with_neutral_groups(GROUPED, 64, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for checkpoint in 0..calls.get() {
        let seen = Cell::new(0);
        let check = || {
            let n = seen.get();
            seen.set(n + 1);
            if n >= checkpoint {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "group conversion cancelled",
                ))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            project_import::decode_ora_with_neutral_groups(GROUPED, 64, &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled,
            "checkpoint {checkpoint}"
        );
    }
}
