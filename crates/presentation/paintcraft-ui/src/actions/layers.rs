//! Studio layer actions operate on persistent identities supplied by the shell.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerAction {
    Add,
    Delete,
    Duplicate,
    ToggleVisibility,
    GoTop,
    GoAbove,
    GoBelow,
    GoBottom,
    MoveTop,
    MoveUp,
    MoveDown,
    MoveBottom,
    Properties,
}

pub const LAYERS: &[LayerAction] = &[
    LayerAction::Add,
    LayerAction::Delete,
    LayerAction::Duplicate,
    LayerAction::ToggleVisibility,
    LayerAction::GoTop,
    LayerAction::GoAbove,
    LayerAction::GoBelow,
    LayerAction::GoBottom,
    LayerAction::MoveTop,
    LayerAction::MoveUp,
    LayerAction::MoveDown,
    LayerAction::MoveBottom,
    LayerAction::Properties,
];

impl LayerAction {
    pub(super) fn descriptor(self) -> Descriptor {
        let (label, shortcut) = match self {
            Self::Properties => ("Layer properties…", Some((Key::F4, Modifiers::NONE))),
            Self::Add => (
                "Add new layer",
                Some((Key::N, Modifiers::COMMAND | Modifiers::SHIFT)),
            ),
            Self::Delete => (
                "Delete layer",
                Some((Key::Delete, Modifiers::COMMAND | Modifiers::SHIFT)),
            ),
            Self::Duplicate => (
                "Duplicate layer",
                Some((Key::D, Modifiers::COMMAND | Modifiers::SHIFT)),
            ),
            Self::ToggleVisibility => (
                "Toggle layer visibility",
                Some((
                    Key::Comma,
                    if cfg!(target_os = "macos") {
                        Modifiers::COMMAND | Modifiers::SHIFT
                    } else {
                        Modifiers::COMMAND
                    },
                )),
            ),
            Self::GoTop => (
                "Go to top layer",
                Some((Key::PageUp, Modifiers::COMMAND | Modifiers::ALT)),
            ),
            Self::GoAbove => ("Go to layer above", Some((Key::PageUp, Modifiers::ALT))),
            Self::GoBelow => ("Go to layer below", Some((Key::PageDown, Modifiers::ALT))),
            Self::GoBottom => (
                "Go to bottom layer",
                Some((Key::PageDown, Modifiers::COMMAND | Modifiers::ALT)),
            ),
            Self::MoveTop => ("Move layer to top", None),
            Self::MoveUp => ("Move layer up", None),
            Self::MoveDown => ("Move layer down", None),
            Self::MoveBottom => ("Move layer to bottom", None),
        };
        Descriptor {
            action: Action::Layer(self),
            label,
            shortcut: shortcut.map(|(logical_key, modifiers)| KeyboardShortcut {
                logical_key,
                modifiers,
            }),
        }
    }

    pub(super) fn enabled(self, ctx: Context) -> bool {
        if !ctx.document_open {
            return false;
        }
        if self == Self::Add {
            return ctx.layer_count < paintcraft_core::types::policy::LAYER_COUNT_MAX;
        }
        let Some(index) = ctx.layer_index else {
            return false;
        };
        match self {
            Self::Duplicate => ctx.layer_count < paintcraft_core::types::policy::LAYER_COUNT_MAX,
            Self::GoTop | Self::GoAbove | Self::MoveTop | Self::MoveUp => {
                index + 1 < ctx.layer_count
            }
            Self::GoBottom | Self::GoBelow | Self::MoveBottom | Self::MoveDown => index > 0,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_and_stack_moves_have_distinct_boundary_and_frontend_states() {
        let context = Context {
            frontend: Frontend::Studio,
            document_open: true,
            can_undo: false,
            can_redo: false,
            can_save_all: false,
            selection_active: false,
            tool: Tool::Pencil,
            blocked: false,
            layer_index: Some(0),
            layer_count: 2,
            layer_visible: true,
            zoom: 1.0,
            pixel_grid: false,
            can_fit_selection: false,
            can_crop_selection: false,
            status_bar: true,
            rulers: false,
            fullscreen: false,
            auto_hide_toolbar: false,
            thumbnail: false,
        };
        assert!(Action::Layer(LayerAction::MoveTop).enabled(context));
        assert!(!Action::Layer(LayerAction::MoveBottom).enabled(context));
        assert!(!Action::Layer(LayerAction::Duplicate).enabled(Context {
            frontend: Frontend::Classic,
            ..context
        }));
        assert!(!Action::Layer(LayerAction::Duplicate).enabled(Context {
            layer_count: paintcraft_core::types::policy::LAYER_COUNT_MAX,
            ..context
        }));
        assert_eq!(
            Action::from_shortcut(Key::D, Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Action::Layer(LayerAction::Duplicate))
        );
        assert_eq!(
            Action::from_shortcut(Key::D, Modifiers::COMMAND),
            Some(Action::Deselect)
        );
        assert_eq!(
            Action::from_shortcut(Key::PageDown, Modifiers::ALT),
            Some(Action::Layer(LayerAction::GoBelow))
        );
        assert_eq!(
            Action::from_shortcut(Key::PageDown, Modifiers::ALT | Modifiers::COMMAND),
            Some(Action::Layer(LayerAction::GoBottom))
        );
        assert_eq!(
            Action::Layer(LayerAction::ToggleVisibility).checked(context),
            Some(true)
        );
    }
}
