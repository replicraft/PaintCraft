use super::*;

#[test]
fn native_history_capability_survives_empty_workspaces_but_respects_modals() {
    let context = Context {
        frontend: Frontend::Studio,
        document_open: false,
        can_undo: false,
        can_redo: false,
        can_paste_onto: false,
        can_save_all: false,
        can_recent: true,
        can_paste: false,
        selection_active: false,
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
        desktop_download: false,
        auto_hide_toolbar: false,
        thumbnail: false,
    };
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let context = Context {
            frontend,
            ..context
        };
        assert!(Action::RecentFiles.enabled(context));
        assert!(!Action::RecentFiles.enabled(Context {
            can_recent: false,
            can_paste: false,
            ..context
        }));
        assert!(!Action::RecentFiles.enabled(Context {
            blocked: true,
            ..context
        }));
        assert!(Action::RecentFiles.descriptor().shortcut.is_none());
    }
    assert!(FILE.contains(&Action::RecentFiles));
    assert!(STUDIO_FILE.contains(&Action::RecentFiles));
    assert_eq!(Action::RecentFiles.activation_code(), 1025);
}
