#[test]
fn pan_is_studio_only_and_h_has_no_modifiers() {
    use super::*;
    let pan = Action::SelectTool(Tool::Pan);
    assert!(pan.available(Frontend::Studio));
    assert!(!pan.available(Frontend::Classic));
    assert_eq!(
        Action::from_shortcut_for(Frontend::Studio, Key::H, Modifiers::NONE),
        Some(pan)
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Classic, Key::H, Modifiers::NONE),
        None
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Studio, Key::H, Modifiers::ALT),
        None
    );
}

#[test]
fn studio_zoom_shortcut_is_plain_z_and_keeps_classic_routing() {
    use super::*;
    let action = Action::SelectTool(Tool::Magnifier);
    assert_eq!(
        Action::from_shortcut_for(Frontend::Studio, Key::Z, Modifiers::NONE),
        Some(action)
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Classic, Key::Z, Modifiers::NONE),
        None
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Studio, Key::Z, Modifiers::COMMAND),
        Some(Action::Undo)
    );
    assert_eq!(action.descriptor_for(Frontend::Studio).label, "Zoom tool");
    assert_eq!(action.descriptor_for(Frontend::Classic).label, "Magnifier");
}

use super::*;

#[test]
fn shortcuts_have_distinct_save_intents_and_exact_modifiers() {
    assert_eq!(
        Action::from_shortcut(Key::S, Modifiers::COMMAND),
        Some(Action::Save)
    );
    assert_eq!(
        Action::from_shortcut(Key::S, Modifiers::COMMAND | Modifiers::SHIFT),
        Some(Action::SaveAs)
    );
    assert_eq!(
        Action::from_shortcut(Key::S, Modifiers::COMMAND | Modifiers::ALT),
        None
    );
    assert_eq!(
        Action::from_shortcut(Key::W, Modifiers::COMMAND | Modifiers::SHIFT),
        None
    );
    assert_eq!(
        Action::from_shortcut(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT),
        Some(Action::Redo)
    );
}

#[test]
fn metadata_preserves_identity_and_context_gates_actions() {
    let context = Context {
        frontend: Frontend::Studio,
        document_open: false,
        can_undo: true,
        can_redo: true,
        can_recent: false,
        can_paste: false,
        can_paste_onto: false,
        can_save_all: true,
        selection_active: true,
        tool: Tool::Brush,
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
        can_exit: true,
        desktop_download: false,
        auto_hide_toolbar: false,
        thumbnail: false,
    };
    let ids: std::collections::HashSet<_> = FILE
        .iter()
        .chain(EDIT)
        .map(|a| a.activation_code())
        .collect();
    assert_eq!(ids.len(), FILE.len() + EDIT.len());
    assert!(!ids.contains(&Action::Exit.activation_code()));
    let mut ids = ids;
    for slot in 0..=u8::MAX {
        for action in [
            Action::EditCustomColor(slot),
            Action::EditSecondaryCustomColor(slot),
        ] {
            assert!(ids.insert(action.activation_code()));
            assert!(!action.enabled(context));
            assert!(action.enabled(Context {
                frontend: Frontend::Classic,
                document_open: true,
                ..context
            }));
            assert!(!action.enabled(Context {
                frontend: Frontend::Classic,
                document_open: true,
                blocked: true,
                ..context
            }));
        }
    }
    for format in ImageExport::ALL {
        let action = Action::ExportImage(format);
        assert!(ids.insert(action.activation_code()));
        assert!(!action.enabled(context));
        assert!(action.enabled(Context {
            document_open: true,
            ..context
        }));
        assert!(!action.enabled(Context {
            document_open: true,
            blocked: true,
            ..context
        }));
    }
    assert!(
        Action::Exit.enabled(context),
        "Exit remains available without a document"
    );
    assert!(!Action::Exit.enabled(Context {
        blocked: true,
        ..context
    }));
    assert!(!Action::Exit.enabled(Context {
        can_exit: false,
        ..context
    }));
    assert!(Action::New.enabled(context));
    assert!(Action::Open.enabled(context));
    assert!(!Action::Save.enabled(context));
    assert!(!Action::Undo.enabled(context));
    assert!(!Action::Interface.enabled(context));
    let context = Context {
        document_open: true,
        ..context
    };
    assert_eq!(Action::SelectTool(Tool::Brush).checked(context), Some(true));
    assert!(!Action::Undo.enabled(Context {
        can_undo: false,
        ..context
    }));
    assert!(!Action::New.enabled(Context {
        blocked: true,
        ..context
    }));
}
