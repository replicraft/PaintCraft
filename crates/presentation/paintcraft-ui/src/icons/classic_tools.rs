//! Original tool outlines, separate from both menu symbols and colored artwork.
use super::Icon;

pub(super) fn source(icon: Icon) -> Option<&'static [u8]> {
    Some(match icon {
        Icon::Pencil => include_bytes!("../../assets/icons/classic-tools/pencil.svg"),
        Icon::Bucket => include_bytes!("../../assets/icons/classic-tools/fill.svg"),
        Icon::Text => include_bytes!("../../assets/icons/classic-tools/text.svg"),
        Icon::Eraser => include_bytes!("../../assets/icons/classic-tools/eraser.svg"),
        Icon::Eyedropper => include_bytes!("../../assets/icons/classic-tools/eyedropper.svg"),
        Icon::ZoomIn => include_bytes!("../../assets/icons/classic-tools/zoom-in.svg"),
        _ => return None,
    })
}
