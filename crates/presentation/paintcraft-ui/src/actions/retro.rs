//! Closed initial Retro presentation capabilities, independent of modern skins.
use super::*;
use paintcraft_tools::ShapePreset;

pub(super) fn available(action: Action) -> bool {
    match action {
        Action::SelectTool(tool) => matches!(
            tool,
            Tool::Pencil
                | Tool::Brush
                | Tool::Eraser
                | Tool::Fill
                | Tool::ColorPicker
                | Tool::Magnifier
                | Tool::Line
                | Tool::Rect
                | Tool::Ellipse
                | Tool::Polygon
                | Tool::Curve
                | Tool::Select
                | Tool::Text
                | Tool::Preset(ShapePreset::RoundedRectangle)
        ),
        Action::ExportImage(format) => format.available(Frontend::Retro),
        Action::View(view) => matches!(
            view,
            ViewAction::ZoomIn
                | ViewAction::ZoomOut
                | ViewAction::FitWindow
                | ViewAction::FitSelection
                | ViewAction::ActualSize
                | ViewAction::PixelGrid
                | ViewAction::StatusBar
                | ViewAction::FullScreen
                | ViewAction::Double
                | ViewAction::Quadruple
                | ViewAction::Octuple
                | ViewAction::SetZoom(_)
        ),
        Action::New
        | Action::Open
        | Action::RecentFiles
        | Action::Save
        | Action::SaveAs
        | Action::Export
        | Action::Print
        | Action::Cut
        | Action::Copy
        | Action::Paste
        | Action::Undo
        | Action::Redo
        | Action::Settings
        | Action::ChooseFrontend(_)
        | Action::CanvasSize
        | Action::ResizeSkew
        | Action::ImageProperties
        | Action::ClearCanvas
        | Action::Crop
        | Action::EraseSelection
        | Action::TransformImage(_)
        | Action::CanvasDefaults
        | Action::EditColors
        | Action::EditSecondaryColors
        | Action::EditCustomColor(_)
        | Action::EditSecondaryCustomColor(_)
        | Action::SelectAll
        | Action::InvertSelection
        | Action::Deselect
        | Action::Close
        | Action::Exit => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "retro/tests.rs"]
mod tests;
