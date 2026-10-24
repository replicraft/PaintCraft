use super::*;

fn context(frontend: Frontend) -> Context {
    Context {
        frontend,
        document_open: true,
        can_undo: false,
        can_redo: false,
        can_recent: false,
        can_save_all: false,
        selection_active: false,
        tool: Tool::Select,
        blocked: false,
        layer_index: None,
        layer_count: 0,
        layer_visible: false,
        zoom: 1.0,
        pixel_grid: false,
        can_fit_selection: false,
        can_crop_selection: false,
        can_erase_selection: false,
        can_transform_image: true,
        status_bar: true,
        rulers: false,
        fullscreen: false,
        auto_hide_toolbar: false,
        thumbnail: false,
    }
}

#[test]
fn invert_is_shared_layer_independent_and_respects_workspace_and_modal_guards() {
    let action = Action::InvertSelection;
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let context = context(frontend);
        assert!(action.available(frontend));
        // Inactive selection means selecting the whole finite canvas, per engine policy.
        assert!(action.enabled(context));
        assert!(action.enabled(Context {
            selection_active: true,
            ..context
        }));
        assert!(!action.enabled(Context {
            document_open: false,
            ..context
        }));
        assert!(!action.enabled(Context {
            blocked: true,
            ..context
        }));
        assert_eq!(
            action.icon(context),
            Some(crate::icons::Icon::InvertSelection)
        );
        assert_eq!(action.descriptor_for(frontend).label, "Invert selection");
    }
    assert_eq!(action.activation_code(), 1026);
    assert!(EDIT.contains(&action));
    let mut identities = std::collections::HashSet::new();
    for action in FILE.iter().chain(EDIT).copied() {
        assert!(identities.insert(action.activation_code()));
    }
}

#[test]
fn invert_shortcut_matches_only_the_registered_modifiers_in_both_frontends() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        assert_eq!(
            Action::from_shortcut_for(frontend, Key::I, Modifiers::COMMAND),
            Some(Action::InvertSelection)
        );
        for modifiers in [
            Modifiers::NONE,
            Modifiers::SHIFT,
            Modifiers::ALT,
            Modifiers::COMMAND | Modifiers::SHIFT,
            Modifiers::COMMAND | Modifiers::ALT,
        ] {
            assert_eq!(Action::from_shortcut_for(frontend, Key::I, modifiers), None);
        }
    }
}

#[test]
fn unsupported_mask_transforms_stay_disabled_without_disabling_inversion() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let whole = context(frontend);
        for transform in [
            Transform::RotateRight,
            Transform::RotateLeft,
            Transform::Rotate180,
            Transform::FlipVertical,
            Transform::FlipHorizontal,
        ] {
            let action = Action::TransformImage(transform);
            assert!(action.enabled(whole));
            assert!(!action.enabled(Context {
                can_transform_image: false,
                ..whole
            }));
            let rectangle = Context {
                selection_active: true,
                can_crop_selection: true,
                can_erase_selection: true,
                layer_index: Some(0),
                ..whole
            };
            assert!(action.enabled(rectangle));
            assert!(!action.enabled(Context {
                layer_index: None,
                ..rectangle
            }));
            assert!(!action.enabled(Context {
                can_transform_image: false,
                ..rectangle
            }));
            assert!(Action::InvertSelection.enabled(Context {
                can_transform_image: false,
                ..rectangle
            }));
        }
    }
}

#[test]
fn mask_erasure_does_not_advertise_unsupported_crop_or_transforms() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let mask = Context {
            selection_active: true,
            can_crop_selection: false,
            can_erase_selection: true,
            can_transform_image: false,
            layer_index: Some(0),
            ..context(frontend)
        };
        assert!(Action::EraseSelection.enabled(mask));
        assert!(!Action::Crop.enabled(mask));
        assert!(Action::InvertSelection.enabled(mask));
        assert!(!Action::EraseSelection.enabled(Context {
            layer_index: None,
            ..mask
        }));
        assert!(!Action::EraseSelection.enabled(Context {
            can_erase_selection: false,
            ..mask
        }));
        assert!(!Action::EraseSelection.enabled(Context {
            blocked: true,
            ..mask
        }));
    }
}

#[test]
fn copy_requires_a_visible_target_and_nonempty_field_and_shares_the_command_shortcut() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let context = Context {
            layer_index: Some(0),
            layer_count: 1,
            layer_visible: true,
            ..context(frontend)
        };
        assert!(Action::Copy.enabled(context));
        assert!(!Action::Copy.enabled(Context {
            layer_visible: false,
            ..context
        }));
        assert!(!Action::Copy.enabled(Context {
            layer_index: None,
            ..context
        }));
        assert!(!Action::Copy.enabled(Context {
            blocked: true,
            ..context
        }));
        assert!(!Action::Copy.enabled(Context {
            document_open: false,
            ..context
        }));
        assert!(!Action::Copy.enabled(Context {
            selection_active: true,
            can_crop_selection: false,
            ..context
        }));
        assert!(Action::Copy.enabled(Context {
            selection_active: true,
            can_crop_selection: true,
            ..context
        }));
        assert_eq!(
            Action::from_shortcut_for(frontend, Key::C, Modifiers::COMMAND),
            Some(Action::Copy)
        );
        assert_ne!(
            Action::from_shortcut_for(frontend, Key::C, Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Action::Copy)
        );
    }
}
