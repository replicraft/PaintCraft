//! Original accented tools, separate from menu symbols and the colored sheet.
use super::Icon;

pub(super) fn source(icon: Icon, dark: bool) -> Option<&'static [u8]> {
    Some(match (icon, dark) {
        (Icon::Pencil, false) => {
            include_bytes!("../../assets/icons/classic-tools/pencil-light.svg")
        }
        (Icon::Pencil, true) => include_bytes!("../../assets/icons/classic-tools/pencil-dark.svg"),
        (Icon::Bucket, false) => include_bytes!("../../assets/icons/classic-tools/fill-light.svg"),
        (Icon::Bucket, true) => include_bytes!("../../assets/icons/classic-tools/fill-dark.svg"),
        (Icon::Text, false) => include_bytes!("../../assets/icons/classic-tools/text-light.svg"),
        (Icon::Text, true) => include_bytes!("../../assets/icons/classic-tools/text-dark.svg"),
        (Icon::Eraser, false) => {
            include_bytes!("../../assets/icons/classic-tools/eraser-light.svg")
        }
        (Icon::Eraser, true) => include_bytes!("../../assets/icons/classic-tools/eraser-dark.svg"),
        (Icon::Eyedropper, false) => {
            include_bytes!("../../assets/icons/classic-tools/eyedropper-light.svg")
        }
        (Icon::Eyedropper, true) => {
            include_bytes!("../../assets/icons/classic-tools/eyedropper-dark.svg")
        }
        (Icon::ZoomIn, false) => {
            include_bytes!("../../assets/icons/classic-tools/zoom-in-light.svg")
        }
        (Icon::ZoomIn, true) => include_bytes!("../../assets/icons/classic-tools/zoom-in-dark.svg"),
        _ => return None,
    })
}
