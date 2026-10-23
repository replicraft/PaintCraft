//! Monochrome menu variants; default colored sources remain separately available.
use super::Icon;

pub(super) fn source(icon: Icon) -> Option<&'static [u8]> {
    Some(match icon {
        Icon::New => include_bytes!("../../assets/icons/classic-menus/new.svg"),
        Icon::Open => include_bytes!("../../assets/icons/classic-menus/open.svg"),
        Icon::Save => include_bytes!("../../assets/icons/classic-menus/save.svg"),
        Icon::SaveAs => include_bytes!("../../assets/icons/classic-menus/save-as.svg"),
        Icon::Print => include_bytes!("../../assets/icons/classic-menus/print.svg"),
        Icon::Share => include_bytes!("../../assets/icons/classic-menus/share.svg"),
        Icon::CanvasSize => include_bytes!("../../assets/icons/classic-menus/canvas-size.svg"),
        Icon::Close => include_bytes!("../../assets/icons/classic-menus/close.svg"),
        Icon::Cut => include_bytes!("../../assets/icons/classic-menus/cut.svg"),
        Icon::Copy => include_bytes!("../../assets/icons/classic-menus/copy.svg"),
        Icon::Paste => include_bytes!("../../assets/icons/classic-menus/paste.svg"),
        Icon::CopyMerged => include_bytes!("../../assets/icons/classic-menus/copy-merged.svg"),
        Icon::Undo => include_bytes!("../../assets/icons/classic-menus/undo.svg"),
        Icon::Redo => include_bytes!("../../assets/icons/classic-menus/redo.svg"),
        Icon::RectangleSelect => {
            include_bytes!("../../assets/icons/classic-menus/rectangle-select.svg")
        }
        Icon::ClearCanvas => include_bytes!("../../assets/icons/classic-menus/clear-canvas.svg"),
        Icon::ZoomIn => include_bytes!("../../assets/icons/classic-menus/zoom-in.svg"),
        Icon::ZoomOut => include_bytes!("../../assets/icons/classic-menus/zoom-out.svg"),
        Icon::FitWindow => include_bytes!("../../assets/icons/classic-menus/fit-window.svg"),
        Icon::ActualSize => include_bytes!("../../assets/icons/classic-menus/actual-size.svg"),
        Icon::PixelGrid => include_bytes!("../../assets/icons/classic-menus/pixel-grid.svg"),
        Icon::Rulers => include_bytes!("../../assets/icons/classic-menus/rulers.svg"),
        Icon::Resize => include_bytes!("../../assets/icons/classic-menus/resize.svg"),
        Icon::ImportImage => include_bytes!("../../assets/icons/classic-menus/import-image.svg"),
        Icon::Recent => include_bytes!("../../assets/icons/classic-menus/recent.svg"),
        Icon::DesktopBackground => {
            include_bytes!("../../assets/icons/classic-menus/desktop-background.svg")
        }
        Icon::ImageProperties => {
            include_bytes!("../../assets/icons/classic-menus/image-properties.svg")
        }
        Icon::Thumbnail => include_bytes!("../../assets/icons/classic-menus/thumbnail.svg"),
        Icon::Pencil => include_bytes!("../../assets/icons/classic-menus/pencil.svg"),
        Icon::Bucket => include_bytes!("../../assets/icons/classic-menus/bucket.svg"),
        Icon::Brush => include_bytes!("../../assets/icons/classic-menus/brush.svg"),
        _ => return None,
    })
}
