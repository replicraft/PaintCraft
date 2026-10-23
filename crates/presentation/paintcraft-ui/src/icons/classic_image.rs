//! Original accented ribbon controls; accent colors remain independent of ink.
use super::Icon;

pub(super) fn source(icon: Icon, dark: bool) -> Option<&'static [u8]> {
    Some(match (icon, dark) {
        (Icon::Layers, false) => {
            include_bytes!("../../assets/icons/classic-image/layers-light.svg")
        }
        (Icon::Layers, true) => include_bytes!("../../assets/icons/classic-image/layers-dark.svg"),
        (Icon::Crop, false) => include_bytes!("../../assets/icons/classic-image/crop-light.svg"),
        (Icon::Crop, true) => include_bytes!("../../assets/icons/classic-image/crop-dark.svg"),
        (Icon::RemoveBackground, false) => {
            include_bytes!("../../assets/icons/classic-image/remove-background-light.svg")
        }
        (Icon::RemoveBackground, true) => {
            include_bytes!("../../assets/icons/classic-image/remove-background-dark.svg")
        }
        (Icon::Resize, false) => {
            include_bytes!("../../assets/icons/classic-image/resize-light.svg")
        }
        (Icon::Resize, true) => include_bytes!("../../assets/icons/classic-image/resize-dark.svg"),
        (Icon::RotateClockwise, false) => {
            include_bytes!("../../assets/icons/classic-image/rotate-clockwise-light.svg")
        }
        (Icon::RotateClockwise, true) => {
            include_bytes!("../../assets/icons/classic-image/rotate-clockwise-dark.svg")
        }
        (Icon::RotateCounterclockwise, false) => {
            include_bytes!("../../assets/icons/classic-image/rotate-counterclockwise-light.svg")
        }
        (Icon::RotateCounterclockwise, true) => {
            include_bytes!("../../assets/icons/classic-image/rotate-counterclockwise-dark.svg")
        }
        (Icon::Rotate180, false) => {
            include_bytes!("../../assets/icons/classic-image/rotate-180-light.svg")
        }
        (Icon::Rotate180, true) => {
            include_bytes!("../../assets/icons/classic-image/rotate-180-dark.svg")
        }
        (Icon::RotateCustom, false) => {
            include_bytes!("../../assets/icons/classic-image/rotate-custom-light.svg")
        }
        (Icon::RotateCustom, true) => {
            include_bytes!("../../assets/icons/classic-image/rotate-custom-dark.svg")
        }
        (Icon::FlipHorizontal, false) => {
            include_bytes!("../../assets/icons/classic-image/flip-horizontal-light.svg")
        }
        (Icon::FlipHorizontal, true) => {
            include_bytes!("../../assets/icons/classic-image/flip-horizontal-dark.svg")
        }
        (Icon::FlipVertical, false) => {
            include_bytes!("../../assets/icons/classic-image/flip-vertical-light.svg")
        }
        (Icon::FlipVertical, true) => {
            include_bytes!("../../assets/icons/classic-image/flip-vertical-dark.svg")
        }
        _ => return None,
    })
}
