//! Semantic action metadata shared by menus, toolbars and keyboard dispatch.
use egui::{Key, KeyboardShortcut, Modifiers};
use paintcraft_tools::Tool;
mod layers;
mod view;
pub use layers::{LAYERS, LayerAction};
pub use view::{VIEW, ViewAction, ZOOM_MAX, ZOOM_MIN};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Export,
    Undo,
    Redo,
    Interface,
    Settings,
    ChooseFrontend(Frontend),
    CanvasSize,
    CanvasDefaults,
    EditColors,
    EditSecondaryColors,
    SelectTool(Tool),
    SelectAll,
    Deselect,
    Close,
    Layer(LayerAction),
    View(ViewAction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frontend {
    Classic,
    Studio,
}

#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub frontend: Frontend,
    pub document_open: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub selection_active: bool,
    pub tool: Tool,
    pub blocked: bool,
    pub layer_index: Option<usize>,
    pub layer_count: usize,
    pub layer_visible: bool,
    pub zoom: f32,
    pub pixel_grid: bool,
    pub can_fit_selection: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Descriptor {
    pub action: Action,
    pub label: &'static str,
    pub shortcut: Option<KeyboardShortcut>,
}

pub const FILE: &[Action] = &[
    Action::New,
    Action::Open,
    Action::Save,
    Action::SaveAs,
    Action::Export,
    Action::Close,
];
pub const EDIT: &[Action] = &[
    Action::Undo,
    Action::Redo,
    Action::SelectAll,
    Action::Deselect,
];

impl Action {
    pub fn descriptor(self) -> Descriptor {
        if let Self::View(action) = self {
            return action.descriptor();
        }
        if let Self::Layer(action) = self {
            return action.descriptor();
        }
        let (label, key) = match self {
            Self::Layer(_) | Self::View(_) => unreachable!(),
            Self::New => ("New", Some((Key::N, false))),
            Self::Open => ("Open…", Some((Key::O, false))),
            Self::Save => ("Save", Some((Key::S, false))),
            Self::SaveAs => ("Save as…", Some((Key::S, true))),
            Self::Export => ("Export…", None),
            Self::Undo => ("Undo", Some((Key::Z, false))),
            Self::Redo => ("Redo", Some((Key::Y, false))),
            Self::ChooseFrontend(frontend) => (
                match frontend {
                    Frontend::Classic => "Classic",
                    Frontend::Studio => "Studio",
                },
                None,
            ),
            Self::Interface => ("Change Paint version…", None),
            Self::Settings => (
                "Settings…",
                cfg!(target_os = "macos").then_some((Key::Comma, false)),
            ),
            Self::CanvasSize => ("Canvas size…", None),
            Self::CanvasDefaults => ("Default canvas size…", None),
            Self::EditColors => ("Edit colors…", None),
            Self::EditSecondaryColors => ("Edit secondary color…", None),
            Self::SelectTool(tool) => (tool.name(), None),
            Self::SelectAll => ("Select all", Some((Key::A, false))),
            Self::Deselect => ("Deselect", Some((Key::D, false))),
            Self::Close => ("Close", Some((Key::W, false))),
        };
        Descriptor {
            action: self,
            label,
            shortcut: key.map(|(logical_key, shift)| KeyboardShortcut {
                modifiers: Modifiers {
                    shift,
                    ..Modifiers::COMMAND
                },
                logical_key,
            }),
        }
    }

    pub fn available(self, frontend: Frontend) -> bool {
        match self {
            Self::Interface
            | Self::CanvasSize
            | Self::CanvasDefaults
            | Self::EditColors
            | Self::EditSecondaryColors => frontend == Frontend::Classic,
            Self::Layer(_) => frontend == Frontend::Studio,
            _ => true,
        }
    }

    pub fn enabled(self, ctx: Context) -> bool {
        if ctx.blocked || !self.available(ctx.frontend) {
            return false;
        }
        match self {
            Self::Layer(action) => action.enabled(ctx),
            Self::View(action) => action.enabled(ctx),
            Self::New
            | Self::Open
            | Self::Interface
            | Self::CanvasDefaults
            | Self::Settings
            | Self::ChooseFrontend(_) => true,
            Self::Undo => ctx.document_open && ctx.can_undo,
            Self::Redo => ctx.document_open && ctx.can_redo,
            Self::Deselect => ctx.document_open && ctx.selection_active,
            _ => ctx.document_open,
        }
    }

    pub fn checked(self, ctx: Context) -> Option<bool> {
        match self {
            Self::Layer(LayerAction::ToggleVisibility) => Some(ctx.layer_visible),
            Self::View(action) => action.checked(ctx),
            Self::ChooseFrontend(frontend) => Some(frontend == ctx.frontend),
            Self::SelectTool(tool) => Some(ctx.document_open && tool == ctx.tool),
            _ => None,
        }
    }

    /// Exact logical-key/modifier matching prevents Save As from falling through
    /// to Save and avoids treating AltGr combinations as application commands.
    pub fn from_shortcut(key: Key, modifiers: Modifiers) -> Option<Self> {
        if matches!(key, Key::Plus | Key::Equals) && modifiers.command && !modifiers.alt {
            return Some(Self::View(ViewAction::ZoomIn));
        }
        if key == Key::Z && modifiers.matches_exact(Modifiers::COMMAND | Modifiers::SHIFT) {
            return Some(Self::Redo);
        }
        FILE.iter()
            .chain(EDIT)
            .copied()
            .chain([Self::Settings])
            .chain(LAYERS.iter().copied().map(Self::Layer))
            .chain(VIEW.iter().copied().map(Self::View))
            .find(|a| {
                a.descriptor()
                    .shortcut
                    .is_some_and(|s| s.logical_key == key && modifiers.matches_exact(s.modifiers))
            })
    }

    pub fn activation_code(self) -> u64 {
        match self {
            Self::Layer(action) => 1200 + action as u64,
            Self::View(action) => action.activation_code(),
            Self::New => 1000,
            Self::Open => 1001,
            Self::Save => 1002,
            Self::SaveAs => 1003,
            Self::Export => 1004,
            Self::Undo => 1005,
            Self::Redo => 1006,
            Self::Interface => 1007,
            Self::SelectAll => 1008,
            Self::Deselect => 1009,
            Self::Close => 1010,
            Self::CanvasSize => 1011,
            Self::Settings => 1015,
            Self::ChooseFrontend(frontend) => 1016 + frontend as u64,
            Self::CanvasDefaults => 1012,
            Self::EditColors => 1013,
            Self::EditSecondaryColors => 1014,
            Self::SelectTool(tool) => 1100 + tool as u64,
        }
    }
}

pub fn menu_item(ui: &mut egui::Ui, action: Action, ctx: Context) -> bool {
    let descriptor = action.descriptor();
    let mut button = match action.icon(ctx) {
        Some(icon) => egui::Button::image_and_text(
            crate::icons::image(ui.ctx(), icon, 16.0),
            descriptor.label,
        ),
        None => egui::Button::new(descriptor.label),
    };
    if let Some(shortcut) = descriptor.shortcut {
        button = button.shortcut_text(ui.ctx().format_shortcut(&shortcut));
    }
    if let Some(checked) = action.checked(ctx) {
        button = button.selected(checked);
    }
    ui.add_enabled(action.enabled(ctx), button).clicked()
}

/// Icon controls use the same enabled state, label and shortcut as menus.
pub fn toolbar_button(
    ui: &mut egui::Ui,
    action: Action,
    icon: crate::widgets::Icon,
    ctx: Context,
) -> bool {
    let descriptor = action.descriptor();
    let label = descriptor.label.trim_end_matches('…');
    let tooltip = descriptor.shortcut.map_or_else(
        || descriptor.label.to_owned(),
        |shortcut| {
            format!(
                "{} ({})",
                descriptor.label,
                ui.ctx().format_shortcut(&shortcut)
            )
        },
    );
    ui.add_enabled_ui(action.enabled(ctx), |ui| {
        crate::widgets::icon_button_with_tooltip(
            ui,
            label,
            icon,
            action.checked(ctx).unwrap_or(false),
            false,
            tooltip,
        )
    })
    .inner
    .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_have_distinct_save_intents_and_exact_modifiers() {
        assert_eq!(
            Action::from_shortcut(Key::S, Modifiers::COMMAND),
            Some(Action::Save)
        );
        assert_eq!(
            Action::from_shortcut(Key::S, Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Action::SaveAs)
        );
        assert_eq!(
            Action::from_shortcut(Key::S, Modifiers::COMMAND | Modifiers::ALT),
            None
        );
        assert_eq!(
            Action::from_shortcut(Key::W, Modifiers::COMMAND | Modifiers::SHIFT),
            None
        );
        assert_eq!(
            Action::from_shortcut(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Action::Redo)
        );
    }

    #[test]
    fn metadata_preserves_identity_and_context_gates_actions() {
        let context = Context {
            frontend: Frontend::Studio,
            document_open: false,
            can_undo: true,
            can_redo: true,
            selection_active: true,
            tool: Tool::Brush,
            blocked: false,
            layer_index: None,
            layer_count: 0,
            layer_visible: false,
            zoom: 1.0,
            pixel_grid: false,
            can_fit_selection: false,
        };
        let ids: std::collections::HashSet<_> = FILE
            .iter()
            .chain(EDIT)
            .map(|a| a.activation_code())
            .collect();
        assert_eq!(ids.len(), FILE.len() + EDIT.len());
        assert!(Action::New.enabled(context));
        assert!(Action::Open.enabled(context));
        assert!(!Action::Save.enabled(context));
        assert!(!Action::Undo.enabled(context));
        assert!(!Action::Interface.enabled(context));
        let context = Context {
            document_open: true,
            ..context
        };
        assert_eq!(Action::SelectTool(Tool::Brush).checked(context), Some(true));
        assert!(!Action::Undo.enabled(Context {
            can_undo: false,
            ..context
        }));
        assert!(!Action::New.enabled(Context {
            blocked: true,
            ..context
        }));
    }
}
