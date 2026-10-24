//! Document viewport actions do not publish document mutations.
use super::*;

pub const ZOOM_MIN: f32 = 1.0 / 65536.0;
pub const ZOOM_MAX: f32 = 64.0;

/// Adjacent power-of-two scale for toolbar and canvas magnification.
pub fn stepped_zoom(zoom: f32, increase: bool) -> f32 {
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    };
    let exponent = if increase {
        (zoom.log2() + 1e-5).floor() + 1.0
    } else {
        (zoom.log2() - 1e-5).ceil() - 1.0
    };
    2.0_f32.powf(exponent).clamp(ZOOM_MIN, ZOOM_MAX)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewAction {
    ZoomIn,
    ZoomOut,
    FitWindow,
    FitSelection,
    ActualSize,
    PixelGrid,
    StatusBar,
    AlwaysShowToolbar,
    AutoHideToolbar,
    Rulers,
    FullScreen,
    Thumbnail,
    Double,
    Quadruple,
    Octuple,
    /// Hundredths of one percent; 10000 is actual size.
    SetZoom(u32),
}

pub const WORKSPACE_VIEW: &[ViewAction] = &[
    ViewAction::Rulers,
    ViewAction::StatusBar,
    ViewAction::AlwaysShowToolbar,
    ViewAction::AutoHideToolbar,
    ViewAction::FullScreen,
    ViewAction::Thumbnail,
];

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
            Self::StatusBar => ("Status bar", None),
            Self::AlwaysShowToolbar => ("Always show toolbar", None),
            Self::AutoHideToolbar => ("Automatically hide toolbar", None),
            Self::Rulers => ("Rulers", Some((Key::R, Modifiers::COMMAND))),
            Self::FullScreen => ("Full screen", Some((Key::F11, Modifiers::NONE))),
            Self::Thumbnail => ("Thumbnail", None),
            Self::Double => ("200%", None),
            Self::Quadruple => ("400%", None),
            Self::Octuple => ("800%", None),
            Self::SetZoom(_) => ("Zoom", None),
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
        if matches!(
            self,
            Self::StatusBar
                | Self::FullScreen
                | Self::Rulers
                | Self::AlwaysShowToolbar
                | Self::AutoHideToolbar
        ) {
            return true;
        }
        ctx.document_open
            && match self {
                Self::ZoomIn => ctx.zoom < ZOOM_MAX,
                Self::ZoomOut => ctx.zoom > ZOOM_MIN,
                Self::FitSelection => ctx.can_fit_selection,
                Self::SetZoom(value) => value > 0 && value <= (ZOOM_MAX * 10000.0) as u32,
                _ => true,
            }
    }
    pub(super) fn activation_code(self) -> u64 {
        match self {
            Self::Thumbnail => 1315,
            Self::ZoomIn => 1300,
            Self::ZoomOut => 1301,
            Self::FitWindow => 1302,
            Self::FitSelection => 1303,
            Self::ActualSize => 1304,
            Self::PixelGrid => 1305,
            Self::StatusBar => 1310,
            Self::AlwaysShowToolbar => 1313,
            Self::AutoHideToolbar => 1314,
            Self::Rulers => 1312,
            Self::FullScreen => 1311,
            Self::Double => 1306,
            Self::Quadruple => 1307,
            Self::Octuple => 1308,
            Self::SetZoom(_) => 1309,
        }
    }
    pub(super) fn checked(self, ctx: Context) -> Option<bool> {
        match self {
            Self::Thumbnail => Some(ctx.thumbnail),
            Self::PixelGrid => Some(ctx.pixel_grid),
            Self::StatusBar => Some(ctx.status_bar),
            Self::AlwaysShowToolbar => Some(!ctx.auto_hide_toolbar),
            Self::AutoHideToolbar => Some(ctx.auto_hide_toolbar),
            Self::Rulers => Some(ctx.rulers),
            Self::FullScreen => Some(ctx.fullscreen),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn zoom_steps_choose_adjacent_powers_and_bound_extreme_input() {
        use super::{ZOOM_MAX, ZOOM_MIN, stepped_zoom};
        for (scale, down, up) in [
            (1.0, 0.5, 2.0),
            (0.75, 0.5, 1.0),
            (3.0, 2.0, 4.0),
            (0.125, 0.0625, 0.25),
        ] {
            assert_eq!(stepped_zoom(scale, false), down);
            assert_eq!(stepped_zoom(scale, true), up);
        }
        assert_eq!(stepped_zoom(ZOOM_MAX, true), ZOOM_MAX);
        assert_eq!(stepped_zoom(ZOOM_MIN, false), ZOOM_MIN);
        assert_eq!(stepped_zoom(f32::NAN, true), 2.0);
        assert_eq!(stepped_zoom(0.0, false), 0.5);
    }

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
            can_crop_selection: false,
            status_bar: true,
            rulers: false,
            fullscreen: false,
            auto_hide_toolbar: false,
            thumbnail: false,
        };
        assert_eq!(
            Action::from_shortcut(Key::F11, Modifiers::NONE),
            Some(Action::View(ViewAction::FullScreen))
        );
        assert_eq!(Action::from_shortcut(Key::F11, Modifiers::SHIFT), None);
        assert!(Action::View(ViewAction::FullScreen).enabled(Context {
            document_open: false,
            ..ctx
        }));
        assert!(!Action::View(ViewAction::StatusBar).enabled(ctx));
        let classic = Context {
            frontend: Frontend::Classic,
            document_open: false,
            ..ctx
        };
        let thumbnail = Action::View(ViewAction::Thumbnail);
        assert!(!thumbnail.enabled(ctx)); // Studio does not own the window.
        assert!(!thumbnail.enabled(classic)); // A closed document has no overview.
        let overview = Context {
            document_open: true,
            thumbnail: true,
            ..classic
        };
        assert!(thumbnail.enabled(overview));
        assert_eq!(thumbnail.checked(overview), Some(true));
        assert!(!thumbnail.enabled(Context {
            blocked: true,
            ..overview
        }));
        let ids: std::collections::HashSet<_> = WORKSPACE_VIEW
            .iter()
            .map(|view| Action::View(*view).activation_code())
            .collect();
        assert_eq!(ids.len(), WORKSPACE_VIEW.len());
        let show = Action::View(ViewAction::AlwaysShowToolbar);
        let hide = Action::View(ViewAction::AutoHideToolbar);
        assert!(!show.enabled(ctx) && !hide.enabled(ctx));
        assert_eq!(show.checked(classic), Some(true));
        assert_eq!(hide.checked(classic), Some(false));
        let hidden = Context {
            auto_hide_toolbar: true,
            thumbnail: false,
            ..classic
        };
        assert!(hide.enabled(hidden) && show.enabled(hidden));
        assert_eq!(show.checked(hidden), Some(false));
        assert_eq!(hide.checked(hidden), Some(true));
        assert!(!hide.enabled(Context {
            blocked: true,
            ..hidden
        }));
        assert_ne!(hide.activation_code(), show.activation_code());
        assert!(Action::View(ViewAction::StatusBar).enabled(classic));
        assert_eq!(
            Action::View(ViewAction::StatusBar).checked(classic),
            Some(true)
        );
        assert!(!Action::View(ViewAction::FullScreen).enabled(Context {
            blocked: true,
            ..classic
        }));
        assert!(!Action::View(ViewAction::ZoomIn).enabled(ctx));
        assert!(!Action::View(ViewAction::FitSelection).enabled(ctx));
        assert_eq!(Action::View(ViewAction::PixelGrid).checked(ctx), Some(true));
        assert!(!Action::View(ViewAction::ZoomOut).enabled(Context {
            zoom: ZOOM_MIN,
            ..ctx
        }));
    }
    #[test]
    fn grid_alias_and_accelerator_belong_to_classic() {
        let action = Action::View(ViewAction::PixelGrid);
        assert_eq!(action.descriptor_for(Frontend::Classic).label, "Gridlines");
        assert_eq!(action.descriptor_for(Frontend::Studio).label, "Pixel grid");
        assert_eq!(
            Action::from_shortcut_for(Frontend::Classic, Key::G, Modifiers::COMMAND),
            Some(action)
        );
        assert_eq!(
            Action::from_shortcut_for(Frontend::Studio, Key::G, Modifiers::COMMAND),
            None
        );
        assert_eq!(
            Action::from_shortcut_for(
                Frontend::Classic,
                Key::G,
                Modifiers::COMMAND | Modifiers::SHIFT
            ),
            None
        );
    }
    #[test]
    fn ruler_binding_belongs_to_classic_and_requires_exact_modifiers() {
        assert_eq!(
            Action::from_shortcut_for(Frontend::Classic, Key::R, Modifiers::COMMAND),
            Some(Action::View(ViewAction::Rulers))
        );
        assert_eq!(
            Action::from_shortcut_for(Frontend::Studio, Key::R, Modifiers::COMMAND),
            None
        );
        assert_eq!(
            Action::from_shortcut_for(
                Frontend::Classic,
                Key::R,
                Modifiers::COMMAND | Modifiers::ALT
            ),
            None
        );
    }
}
