//! The host supplies projection truth separately from its accurate layer count.
use super::*;
fn context(layer_count: usize, can_flatten: bool) -> Context {
    Context {
        frontend: Frontend::Studio,
        document_open: true,
        can_undo: false,
        can_redo: false,
        can_recent: false,
        can_paste: false,
        can_paste_onto: false,
        can_save_all: false,
        selection_active: false,
        tool: Tool::Pencil,
        blocked: false,
        layer_index: (layer_count > 0).then_some(0),
        layer_count,
        can_flatten,
        layer_visible: layer_count > 0,
        zoom: 1.0,
        pixel_grid: false,
        can_fit_selection: false,
        can_crop_selection: false,
        can_erase_selection: false,
        can_transform_image: false,
        can_rotate_custom: false,
        status_bar: true,
        rulers: false,
        fullscreen: false,
        can_exit: true,
        desktop_download: false,
        auto_hide_toolbar: false,
        thumbnail: false,
    }
}
#[test]
fn one_rich_layer_is_enabled_without_enabling_plain_or_empty_documents() {
    let rich = context(1, true);
    assert!(Action::FlattenImage.enabled(rich));
    assert_eq!(rich.layer_count, 1);
    assert!(!Action::FlattenImage.enabled(context(1, false)));
    assert!(!Action::FlattenImage.enabled(context(0, false)));
    assert!(Action::FlattenImage.enabled(context(2, true)));
}
#[test]
fn blocking_document_and_classic_availability_guards_still_apply() {
    let rich = context(1, true);
    assert!(!Action::FlattenImage.enabled(Context {
        blocked: true,
        ..rich
    }));
    assert!(!Action::FlattenImage.enabled(Context {
        document_open: false,
        ..rich
    }));
    assert!(!Action::FlattenImage.enabled(Context {
        frontend: Frontend::Classic,
        ..rich
    }));
    assert!(Action::FlattenImage.available(Frontend::Studio));
    assert!(!Action::FlattenImage.available(Frontend::Classic));
    assert_eq!(
        Action::from_shortcut_for(
            Frontend::Studio,
            Key::F,
            Modifiers::COMMAND | Modifiers::SHIFT
        ),
        Some(Action::FlattenImage)
    );
    assert_eq!(
        Action::from_shortcut_for(
            Frontend::Classic,
            Key::F,
            Modifiers::COMMAND | Modifiers::SHIFT
        ),
        None
    );
}
