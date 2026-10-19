//! Studio viewport actions do not publish document mutations.
use super::*;

pub const ZOOM_MIN: f32 = 1.0 / 65536.0;
pub const ZOOM_MAX: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewAction {
    ZoomIn,
    ZoomOut,
    FitWindow,
    FitSelection,
    ActualSize,
    PixelGrid,
    Double,
    Quadruple,
    Octuple,
}

pub const VIEW: &[ViewAction] = &[
    ViewAction::ZoomIn,
    ViewAction::ZoomOut,
    ViewAction::FitWindow,
    ViewAction::FitSelection,
    ViewAction::ActualSize,
    ViewAction::PixelGrid,
];

impl ViewAction {
    pub(super) fn descriptor(self) -> Descriptor {
        let (label, shortcut) = match self {
            Self::ZoomIn => ("Zoom in", Some((Key::Plus, Modifiers::COMMAND))),
            Self::ZoomOut => ("Zoom out", Some((Key::Minus, Modifiers::COMMAND))),
            Self::FitWindow => ("Zoom to window", Some((Key::B, Modifiers::COMMAND))),
            Self::FitSelection => (
                "Zoom to selection",
                Some((Key::B, Modifiers::COMMAND | Modifiers::SHIFT)),
            ),
            Self::ActualSize => ("Actual size", Some((Key::Num0, Modifiers::COMMAND))),
            Self::PixelGrid => ("Pixel grid", None),
            Self::Double => ("200%", None),
            Self::Quadruple => ("400%", None),
            Self::Octuple => ("800%", None),
        };
        Descriptor {
            action: Action::View(self),
            label,
            shortcut: shortcut.map(|(logical_key, modifiers)| KeyboardShortcut {
                logical_key,
                modifiers,
            }),
        }
    }
    pub(super) fn enabled(self, ctx: Context) -> bool {
        ctx.document_open
            && match self {
                Self::ZoomIn => ctx.zoom < ZOOM_MAX,
                Self::ZoomOut => ctx.zoom > ZOOM_MIN,
                Self::FitSelection => ctx.can_fit_selection,
                _ => true,
            }
    }
    pub(super) fn checked(self, ctx: Context) -> Option<bool> {
        match self {
            Self::PixelGrid => Some(ctx.pixel_grid),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_shortcuts_and_boundary_states_do_not_overlap_other_actions() {
        assert_eq!(
            Action::from_shortcut(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Action::View(ViewAction::ZoomIn))
        );
        assert_eq!(
            Action::from_shortcut(Key::Equals, Modifiers::COMMAND),
            Some(Action::View(ViewAction::ZoomIn))
        );
        assert_eq!(
            Action::from_shortcut(Key::B, Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Action::View(ViewAction::FitSelection))
        );
        let ctx = Context {
            frontend: Frontend::Studio,
            document_open: true,
            can_undo: false,
            can_redo: false,
            selection_active: true,
            tool: paintcraft_tools::Tool::Pencil,
            blocked: false,
            layer_index: Some(0),
            layer_count: 1,
            layer_visible: true,
            zoom: ZOOM_MAX,
            pixel_grid: true,
            can_fit_selection: false,
        };
        assert!(!Action::View(ViewAction::ZoomIn).enabled(ctx));
        assert!(!Action::View(ViewAction::FitSelection).enabled(ctx));
        assert_eq!(Action::View(ViewAction::PixelGrid).checked(ctx), Some(true));
        assert!(!Action::View(ViewAction::ZoomOut).enabled(Context {
            zoom: ZOOM_MIN,
            ..ctx
        }));
    }
}
