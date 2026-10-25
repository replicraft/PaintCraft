use super::*;

#[test]
fn save_all_has_a_studio_only_exact_shortcut_and_distinct_action_identity() {
    let modifiers = Modifiers::COMMAND | Modifiers::ALT;
    assert_eq!(
        Action::from_shortcut_for(Frontend::Studio, Key::S, modifiers),
        Some(Action::SaveAll)
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Classic, Key::S, modifiers),
        None
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Studio, Key::S, modifiers | Modifiers::SHIFT),
        None
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Studio, Key::S, Modifiers::COMMAND),
        Some(Action::Save)
    );
    assert_eq!(
        Action::from_shortcut_for(
            Frontend::Studio,
            Key::S,
            Modifiers::COMMAND | Modifiers::SHIFT
        ),
        Some(Action::SaveAs)
    );
    assert!(STUDIO_FILE.contains(&Action::SaveAll));
    assert!(!FILE.contains(&Action::SaveAll));
    assert!(
        FILE.iter()
            .chain(EDIT)
            .all(|action| action.activation_code() != Action::SaveAll.activation_code())
    );
}

#[test]
fn save_all_requires_a_modified_document_and_an_unblocked_studio_workspace() {
    let context = Context {
        frontend: Frontend::Studio,
        document_open: true,
        can_recent: false,
        can_paste: false,
        can_save_all: true,
        can_undo: false,
        can_redo: false,
        selection_active: false,
        tool: Tool::Pencil,
        blocked: false,
        layer_index: Some(0),
        layer_count: 1,
        layer_visible: true,
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
    };
    assert!(Action::SaveAll.enabled(context));
    assert!(!Action::SaveAll.enabled(Context {
        can_save_all: false,
        ..context
    }));
    assert!(!Action::SaveAll.enabled(Context {
        blocked: true,
        ..context
    }));
    assert!(!Action::SaveAll.enabled(Context {
        document_open: false,
        ..context
    }));
    assert!(!Action::SaveAll.enabled(Context {
        frontend: Frontend::Classic,
        ..context
    }));
}
