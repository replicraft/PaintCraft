//! Original theme-specific metric and zoom symbols for Classic status chrome.
use super::Icon;

pub(super) fn source(icon: Icon, dark: bool) -> Option<&'static [u8]> {
    Some(match (icon, dark) {
        (Icon::Pointer, false) => {
            include_bytes!("../../assets/icons/classic-status/pointer-light.svg")
        }
        (Icon::Pointer, true) => {
            include_bytes!("../../assets/icons/classic-status/pointer-dark.svg")
        }
        (Icon::RectangleSelect, false) => {
            include_bytes!("../../assets/icons/classic-status/selection-light.svg")
        }
        (Icon::RectangleSelect, true) => {
            include_bytes!("../../assets/icons/classic-status/selection-dark.svg")
        }
        (Icon::CanvasSize, false) => {
            include_bytes!("../../assets/icons/classic-status/canvas-size-light.svg")
        }
        (Icon::CanvasSize, true) => {
            include_bytes!("../../assets/icons/classic-status/canvas-size-dark.svg")
        }
        (Icon::FitWindow, false) => {
            include_bytes!("../../assets/icons/classic-status/fit-window-light.svg")
        }
        (Icon::FitWindow, true) => {
            include_bytes!("../../assets/icons/classic-status/fit-window-dark.svg")
        }
        (Icon::ZoomIn, false) => {
            include_bytes!("../../assets/icons/classic-status/zoom-in-light.svg")
        }
        (Icon::ZoomIn, true) => {
            include_bytes!("../../assets/icons/classic-status/zoom-in-dark.svg")
        }
        (Icon::ZoomOut, false) => {
            include_bytes!("../../assets/icons/classic-status/zoom-out-light.svg")
        }
        (Icon::ZoomOut, true) => {
            include_bytes!("../../assets/icons/classic-status/zoom-out-dark.svg")
        }
        _ => return None,
    })
}
