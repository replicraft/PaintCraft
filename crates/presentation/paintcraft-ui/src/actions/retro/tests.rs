use super::*;
fn context() -> Context {
    Context {
        frontend: Frontend::Retro,
        document_open: true,
        can_undo: true,
        can_redo: true,
        can_recent: true,
        can_paste: true,
        can_paste_onto: true,
        can_save_all: true,
        selection_active: true,
        tool: Tool::Brush,
        blocked: false,
        layer_index: Some(0),
        layer_count: 2,
        can_flatten: true,
        layer_visible: true,
        zoom: 1.0,
        pixel_grid: false,
        can_fit_selection: true,
        can_crop_selection: true,
        can_erase_selection: true,
        can_transform_image: true,
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
fn third_interface_keeps_old_ids_and_never_collides_with_clear_canvas() {
    assert_eq!(
        [
            Frontend::Classic as u8,
            Frontend::Studio as u8,
            Frontend::Retro as u8
        ],
        [0, 1, 2]
    );
    assert_eq!(
        Action::ChooseFrontend(Frontend::Classic).activation_code(),
        1016
    );
    assert_eq!(
        Action::ChooseFrontend(Frontend::Studio).activation_code(),
        1017
    );
    assert_eq!(Action::ClearCanvas.activation_code(), 1018);
    let retro = Action::ChooseFrontend(Frontend::Retro);
    assert_eq!(retro.activation_code(), 5000);
    assert_eq!(retro.descriptor().label, "Retro");
    for action in FILE
        .iter()
        .chain(STUDIO_FILE)
        .chain(EDIT)
        .copied()
        .chain(ImageExport::ALL.into_iter().map(Action::ExportImage))
        .chain(VIEW.iter().chain(WORKSPACE_VIEW).copied().map(Action::View))
        .chain(LAYERS.iter().copied().map(Action::Layer))
    {
        assert_ne!(action.activation_code(), retro.activation_code());
    }
    let mut ctx = context();
    for frontend in [Frontend::Classic, Frontend::Studio, Frontend::Retro] {
        let action = Action::ChooseFrontend(frontend);
        assert!(action.enabled(ctx));
        assert_eq!(action.checked(ctx), Some(frontend == Frontend::Retro));
    }
    ctx.blocked = true;
    assert!(!retro.enabled(ctx));
}
#[test]
fn retro_is_closed_to_supported_tools_formats_and_paint_actions() {
    let ctx = context();
    for tool in [
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Fill,
        Tool::ColorPicker,
        Tool::Magnifier,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Polygon,
        Tool::Curve,
        Tool::Select,
        Tool::Text,
        Tool::Preset(ShapePreset::RoundedRectangle),
    ] {
        assert!(Action::SelectTool(tool).enabled(ctx));
    }
    assert!(!Action::SelectTool(Tool::Pan).enabled(ctx));
    for preset in ShapePreset::ALL {
        assert_eq!(
            Action::SelectTool(Tool::Preset(preset)).available(Frontend::Retro),
            preset == ShapePreset::RoundedRectangle
        );
    }
    for format in ImageExport::ALL {
        assert_eq!(
            format.available(Frontend::Retro),
            matches!(format, ImageExport::Png | ImageExport::Bmp)
        );
    }
    for action in [
        Action::New,
        Action::Open,
        Action::Save,
        Action::SaveAs,
        Action::Print,
        Action::CanvasSize,
        Action::ResizeSkew,
        Action::ImageProperties,
        Action::EditColors,
        Action::EditSecondaryColors,
        Action::View(ViewAction::StatusBar),
    ] {
        assert!(action.enabled(ctx));
    }
    for action in [
        Action::SaveAll,
        Action::FlattenImage,
        Action::ImportImage,
        Action::CopyVisible,
        Action::PasteLayer,
        Action::Interface,
        Action::ExportIcons,
        Action::ExportIcns,
        Action::Palettes,
        Action::BrushResources,
        Action::ShapeResources,
        Action::Layer(LayerAction::Add),
        Action::View(ViewAction::Rulers),
        Action::View(ViewAction::Thumbnail),
        Action::View(ViewAction::AutoHideToolbar),
        Action::View(ViewAction::AlwaysShowToolbar),
    ] {
        assert!(!action.available(Frontend::Retro));
    }
}
#[test]
fn existing_profiles_and_shortcuts_keep_their_distinct_contracts() {
    assert!(Action::Layer(LayerAction::Add).available(Frontend::Studio));
    assert!(!Action::Layer(LayerAction::Add).available(Frontend::Classic));
    assert!(ImageExport::Jpeg.available(Frontend::Classic));
    assert!(ImageExport::Jxl.available(Frontend::Studio));
    assert!(!ImageExport::Jxl.available(Frontend::Classic));
    assert_eq!(
        Action::from_shortcut_for(Frontend::Retro, Key::S, Modifiers::COMMAND),
        Some(Action::Save)
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Retro, Key::H, Modifiers::NONE),
        None
    );
    assert_eq!(
        Action::from_shortcut_for(Frontend::Retro, Key::Z, Modifiers::NONE),
        None
    );
}
