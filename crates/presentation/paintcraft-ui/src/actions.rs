//! Semantic action metadata shared by menus, toolbars and keyboard dispatch.
use crate::t;
use egui::{Key, KeyboardShortcut, Modifiers};
use paintcraft_core::types::orthogonal::OrthogonalTransform as Transform;
use paintcraft_tools::Tool;
mod export;
pub use export::ImageExport;
mod layers;
mod view;
pub use layers::{LAYERS, LayerAction};
pub use view::{VIEW, ViewAction, WORKSPACE_VIEW, ZOOM_MAX, ZOOM_MIN, stepped_zoom};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    RecentFiles,
    Save,
    SaveAs,
    SaveAll,
    Export,
    Print,
    ExportImage(ImageExport),
    Undo,
    Redo,
    Interface,
    Settings,
    ChooseFrontend(Frontend),
    CanvasSize,
    ResizeSkew,
    ImageProperties,
    ClearCanvas,
    Crop,
    EraseSelection,
    TransformImage(Transform),
    CanvasDefaults,
    EditColors,
    EditSecondaryColors,
    /// Open the color editor at a zero-based custom palette slot.
    EditCustomColor(u8),
    /// Edit a custom palette slot while retaining Color 2 as the adoption target.
    EditSecondaryCustomColor(u8),
    SelectTool(Tool),
    SelectAll,
    InvertSelection,
    Deselect,
    Close,
    Exit,
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
    pub can_recent: bool,
    pub can_save_all: bool,
    pub selection_active: bool,
    pub tool: Tool,
    pub blocked: bool,
    pub layer_index: Option<usize>,
    pub layer_count: usize,
    pub layer_visible: bool,
    pub zoom: f32,
    pub pixel_grid: bool,
    pub can_fit_selection: bool,
    pub can_crop_selection: bool,
    pub can_erase_selection: bool,
    /// Whole-image or selected-pixel transform is implemented for this selection kind.
    pub can_transform_image: bool,
    pub status_bar: bool,
    pub rulers: bool,
    pub fullscreen: bool,
    pub auto_hide_toolbar: bool,
    pub thumbnail: bool,
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
    Action::RecentFiles,
    Action::Save,
    Action::SaveAs,
    Action::Export,
    Action::Print,
    Action::Close,
];
pub const STUDIO_FILE: &[Action] = &[
    Action::New,
    Action::Open,
    Action::RecentFiles,
    Action::Save,
    Action::SaveAs,
    Action::SaveAll,
    Action::Export,
    Action::Print,
    Action::Close,
];
pub const EDIT: &[Action] = &[
    Action::Crop,
    Action::Undo,
    Action::Redo,
    Action::SelectAll,
    Action::InvertSelection,
    Action::Deselect,
    Action::EraseSelection,
    Action::ClearCanvas,
    Action::TransformImage(Transform::RotateRight),
    Action::TransformImage(Transform::RotateLeft),
    Action::TransformImage(Transform::Rotate180),
    Action::TransformImage(Transform::FlipVertical),
    Action::TransformImage(Transform::FlipHorizontal),
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
            Self::RecentFiles => ("Recent files", None),
            Self::Save => ("Save", Some((Key::S, false))),
            Self::SaveAs => ("Save as…", Some((Key::S, true))),
            Self::SaveAll => ("Save All", None),
            Self::Export => ("Export…", None),
            Self::Print => ("Print…", Some((Key::P, false))),
            Self::ExportImage(format) => (format.label(), None),
            Self::Undo => ("Undo", Some((Key::Z, false))),
            Self::Redo => ("Redo", Some((Key::Y, false))),
            Self::ChooseFrontend(frontend) => (
                match frontend {
                    Frontend::Classic => "Classic",
                    Frontend::Studio => "Studio",
                },
                None,
            ),
            Self::Interface => ("Change Classic skin…", None),
            Self::Settings => (
                "Settings…",
                cfg!(target_os = "macos").then_some((Key::Comma, false)),
            ),
            Self::CanvasSize => ("Canvas size…", None),
            Self::ResizeSkew => ("Resize and skew", None),
            Self::ImageProperties => ("Image properties", Some((Key::E, false))),
            Self::ClearCanvas => ("Clear Canvas", None),
            Self::Crop => ("Crop", None),
            Self::EraseSelection => ("Delete", None),
            Self::TransformImage(t) => (
                match t {
                    Transform::RotateRight => "Rotate right 90°",
                    Transform::RotateLeft => "Rotate left 90°",
                    Transform::Rotate180 => "Rotate 180°",
                    Transform::FlipHorizontal => "Flip horizontal",
                    Transform::FlipVertical => "Flip vertical",
                },
                None,
            ),
            Self::CanvasDefaults => ("Default canvas size…", None),
            Self::EditColors => ("Edit colors…", None),
            Self::EditSecondaryColors => ("Edit secondary color…", None),
            Self::EditCustomColor(_) | Self::EditSecondaryCustomColor(_) => {
                ("Edit custom color…", None)
            }
            Self::SelectTool(tool) => (tool.name(), None),
            Self::SelectAll => ("Select all", Some((Key::A, false))),
            Self::InvertSelection => ("Invert selection", Some((Key::I, false))),
            Self::Deselect => ("Deselect", Some((Key::D, false))),
            Self::Close => ("Close", Some((Key::W, false))),
            Self::Exit => ("Exit", None),
        };
        Descriptor {
            action: self,
            label,
            shortcut: if self == Self::SaveAll {
                Some(KeyboardShortcut::new(
                    Modifiers::COMMAND | Modifiers::ALT,
                    Key::S,
                ))
            } else if self == Self::EraseSelection {
                Some(KeyboardShortcut::new(Modifiers::NONE, Key::Delete))
            } else {
                key.map(|(logical_key, shift)| KeyboardShortcut {
                    modifiers: Modifiers {
                        shift,
                        ..Modifiers::COMMAND
                    },
                    logical_key,
                })
            },
        }
    }

    /// Frontends may expose a shared operation under their reference UI name.
    pub fn descriptor_for(self, frontend: Frontend) -> Descriptor {
        let mut descriptor = self.descriptor();
        if frontend == Frontend::Classic && self == Self::View(ViewAction::PixelGrid) {
            descriptor.label = "Gridlines";
            descriptor.shortcut = Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::G));
        }
        if frontend == Frontend::Studio && self == Self::SelectTool(Tool::Magnifier) {
            descriptor.label = "Zoom tool";
            descriptor.shortcut = Some(KeyboardShortcut::new(Modifiers::NONE, Key::Z));
        }
        if frontend == Frontend::Studio && self == Self::SelectTool(Tool::Pan) {
            descriptor.shortcut = Some(KeyboardShortcut::new(Modifiers::NONE, Key::H));
        }
        descriptor
    }

    pub fn from_shortcut_for(frontend: Frontend, key: Key, modifiers: Modifiers) -> Option<Self> {
        if frontend == Frontend::Studio
            && key == Key::S
            && modifiers.matches_exact(Modifiers::COMMAND | Modifiers::ALT)
        {
            return Some(Self::SaveAll);
        }
        if frontend == Frontend::Studio && key == Key::Z && modifiers.matches_exact(Modifiers::NONE)
        {
            return Some(Self::SelectTool(Tool::Magnifier));
        }
        if frontend == Frontend::Studio && key == Key::H && modifiers.matches_exact(Modifiers::NONE)
        {
            return Some(Self::SelectTool(Tool::Pan));
        }
        VIEW.iter()
            .chain(WORKSPACE_VIEW)
            .copied()
            .map(Self::View)
            .filter(|action| action.available(frontend))
            .find(|action| {
                action
                    .descriptor_for(frontend)
                    .shortcut
                    .is_some_and(|shortcut| {
                        shortcut.logical_key == key && modifiers.matches_exact(shortcut.modifiers)
                    })
            })
            .or_else(|| {
                Self::from_shortcut(key, modifiers).filter(|action| action.available(frontend))
            })
    }

    pub fn available(self, frontend: Frontend) -> bool {
        match self {
            Self::SaveAll | Self::SelectTool(Tool::Pan) => frontend == Frontend::Studio,
            Self::View(
                ViewAction::StatusBar
                | ViewAction::Rulers
                | ViewAction::AlwaysShowToolbar
                | ViewAction::AutoHideToolbar
                | ViewAction::Thumbnail,
            ) => frontend == Frontend::Classic,
            Self::Interface
            | Self::ResizeSkew
            | Self::ImageProperties
            | Self::CanvasDefaults
            | Self::EditColors
            | Self::EditSecondaryColors
            | Self::EditCustomColor(_)
            | Self::EditSecondaryCustomColor(_) => frontend == Frontend::Classic,
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
            | Self::Exit
            | Self::Open
            | Self::Interface
            | Self::CanvasDefaults
            | Self::Settings
            | Self::ChooseFrontend(_) => true,
            Self::Print => {
                ctx.document_open && cfg!(target_os = "macos") && !cfg!(target_arch = "wasm32")
            }
            Self::RecentFiles => ctx.can_recent,
            Self::SaveAll => ctx.document_open && ctx.can_save_all,
            Self::Undo => ctx.document_open && ctx.can_undo,
            Self::Redo => ctx.document_open && ctx.can_redo,
            Self::Deselect => ctx.document_open && ctx.selection_active,
            Self::Crop => ctx.document_open && ctx.can_crop_selection,
            Self::EraseSelection => {
                ctx.document_open && ctx.can_erase_selection && ctx.layer_index.is_some()
            }
            Self::TransformImage(_) => {
                ctx.document_open
                    && ctx.can_transform_image
                    && (!ctx.selection_active
                        || (ctx.can_crop_selection && ctx.layer_index.is_some()))
            }
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
        if key == Key::F11 && modifiers.matches_exact(Modifiers::NONE) {
            return Some(Self::View(ViewAction::FullScreen));
        }
        if matches!(key, Key::Plus | Key::Equals) && modifiers.command && !modifiers.alt {
            return Some(Self::View(ViewAction::ZoomIn));
        }
        if key == Key::Z && modifiers.matches_exact(Modifiers::COMMAND | Modifiers::SHIFT) {
            return Some(Self::Redo);
        }
        FILE.iter()
            .chain(EDIT)
            .copied()
            .chain([Self::Settings, Self::ImageProperties])
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
            Self::RecentFiles => 1025,
            Self::Save => 1002,
            Self::SaveAs => 1003,
            Self::SaveAll => 1024,
            Self::Export => 1004,
            Self::Print => 1027,
            Self::ExportImage(format) => 1040 + format as u64,
            Self::Undo => 1005,
            Self::Redo => 1006,
            Self::Interface => 1007,
            Self::SelectAll => 1008,
            Self::InvertSelection => 1026,
            Self::Deselect => 1009,
            Self::Close => 1010,
            Self::Exit => 1019,
            Self::CanvasSize => 1011,
            Self::ResizeSkew => 1020,
            Self::ImageProperties => 1021,
            Self::ClearCanvas => 1018,
            Self::Crop => 1022,
            Self::EraseSelection => 1023,
            Self::TransformImage(t) => 1030 + t as u64,
            Self::Settings => 1015,
            Self::ChooseFrontend(frontend) => 1016 + frontend as u64,
            Self::CanvasDefaults => 1012,
            Self::EditColors => 1013,
            Self::EditSecondaryColors => 1014,
            Self::EditCustomColor(slot) => 2000 + u64::from(slot),
            Self::EditSecondaryCustomColor(slot) => 2300 + u64::from(slot),
            Self::SelectTool(tool) => 1100 + tool.action_id(),
        }
    }
}

pub fn menu_item(ui: &mut egui::Ui, action: Action, ctx: Context) -> bool {
    menu_item_with_label(ui, action, ctx, action.descriptor_for(ctx.frontend).label)
}

/// Frontend-specific wording retains the action's identity, shortcut and gates.
pub fn menu_item_with_label(
    ui: &mut egui::Ui,
    action: Action,
    ctx: Context,
    label: &'static str,
) -> bool {
    menu_item_with_icon_size(ui, action, ctx, label, 16.0)
}

/// Frontend icon metrics retain the same labels, gates, shortcuts and checkmarks.
pub fn menu_item_with_icon_size(
    ui: &mut egui::Ui,
    action: Action,
    ctx: Context,
    label: &'static str,
    icon_size: f32,
) -> bool {
    menu_item_with_icon_style(ui, action, ctx, label, icon_size, MenuIconStyle::Colorful)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuIconStyle {
    #[default]
    Colorful,
    ClassicOutline,
    /// Keep the action glyph visible until a toggle becomes checked.
    ClassicOutlineToggle,
}

/// Choose frontend artwork while retaining shared action metadata and routing.
pub fn menu_item_with_icon_style(
    ui: &mut egui::Ui,
    action: Action,
    ctx: Context,
    label: &'static str,
    icon_size: f32,
    icon_style: MenuIconStyle,
) -> bool {
    let descriptor = Descriptor {
        label,
        ..action.descriptor_for(ctx.frontend)
    };
    let localized_label = t!(ui.ctx(), descriptor.label);
    let checked = action.checked(ctx);
    let icon = if icon_style == MenuIconStyle::Colorful {
        action.icon(ctx)
    } else {
        action.classic_menu_icon(ctx)
    };
    // Checkable menu rows reserve an indicator column; their selected state is
    // announced to accessibility without painting a permanently selected button.
    let classic_marker =
        (checked.is_some() || icon.is_none()) && icon_style != MenuIconStyle::Colorful;
    let marker_id = egui::IdSalt::new("menu-toggle-marker");
    let title = if checked.is_some() && !classic_marker {
        format!("       {}", localized_label)
    } else {
        localized_label.clone()
    };
    let mut button = if classic_marker {
        egui::Button::new((
            egui::Atom::custom(marker_id, egui::Vec2::splat(icon_size)),
            title,
        ))
    } else {
        match (checked, icon) {
            (None, Some(icon)) => {
                let image = match icon_style {
                    MenuIconStyle::Colorful => crate::icons::image(ui.ctx(), icon, icon_size),
                    MenuIconStyle::ClassicOutline | MenuIconStyle::ClassicOutlineToggle => {
                        crate::icons::classic_menu_image(
                            ui.ctx(),
                            icon,
                            icon_size,
                            ui.visuals().strong_text_color(),
                        )
                    }
                };
                egui::Button::image_and_text(image, title)
            }
            _ => egui::Button::new(title),
        }
    };
    let shortcut = descriptor
        .shortcut
        .map(|shortcut| ui.ctx().format_shortcut(&shortcut));
    if let Some(shortcut) = &shortcut {
        button = button.shortcut_text(shortcut);
    }
    let enabled = action.enabled(ctx) && ui.is_enabled();
    let painted = ui.add_enabled_ui(enabled, |ui| button.atom_ui(ui)).inner;
    let marker_rect = painted.rect(marker_id);
    let response = painted.response;
    if let Some(checked) = checked {
        let label = shortcut.map_or_else(
            || localized_label.clone(),
            |shortcut| format!("{} {shortcut}", localized_label),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, enabled, checked, &label)
        });
        if checked {
            let center = marker_rect.map_or(response.rect.left_center(), |rect| {
                rect.center() - egui::vec2(8.0, 0.0)
            });
            let ink = if enabled {
                ui.visuals().text_color()
            } else {
                ui.visuals().weak_text_color()
            };
            let stroke = egui::Stroke::new(1.5, ink);
            let a = center + egui::vec2(4.0, 0.0);
            let b = center + egui::vec2(7.0, 3.0);
            let c = center + egui::vec2(13.0, -4.0);
            ui.painter().line_segment([a, b], stroke);
            ui.painter().line_segment([b, c], stroke);
        } else if icon_style == MenuIconStyle::ClassicOutlineToggle
            && let (Some(rect), Some(icon)) = (marker_rect, icon)
        {
            let ink = if enabled {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().weak_text_color()
            };
            crate::icons::classic_menu_image(ui.ctx(), icon, icon_size, ink).paint_at(ui, rect);
        }
    }
    response.clicked()
}

/// Icon controls use the same enabled state, label and shortcut as menus.
pub fn toolbar_button(
    ui: &mut egui::Ui,
    action: Action,
    icon: crate::widgets::Icon,
    ctx: Context,
) -> bool {
    toolbar_button_artwork(ui, action, icon, ctx, crate::widgets::IconArtwork::Colorful)
}

/// Classic artwork shares the same action gates, selected state and tooltip.
pub fn classic_toolbar_button(
    ui: &mut egui::Ui,
    action: Action,
    icon: crate::widgets::Icon,
    ctx: Context,
) -> bool {
    toolbar_button_artwork(
        ui,
        action,
        icon,
        ctx,
        crate::widgets::IconArtwork::ClassicMenu,
    )
}

/// Classic status symbols share the same action gates, state and tooltip.
pub fn classic_status_button(
    ui: &mut egui::Ui,
    action: Action,
    icon: crate::widgets::Icon,
    ctx: Context,
) -> bool {
    toolbar_button_artwork(
        ui,
        action,
        icon,
        ctx,
        crate::widgets::IconArtwork::ClassicStatus,
    )
}

fn toolbar_button_artwork(
    ui: &mut egui::Ui,
    action: Action,
    icon: crate::widgets::Icon,
    ctx: Context,
    artwork: crate::widgets::IconArtwork,
) -> bool {
    let descriptor = action.descriptor_for(ctx.frontend);
    let localized_label = t!(ui.ctx(), descriptor.label);
    let label = localized_label.trim_end_matches('…');
    let tooltip = descriptor.shortcut.map_or_else(
        || localized_label.clone(),
        |shortcut| {
            format!(
                "{} ({})",
                localized_label,
                ui.ctx().format_shortcut(&shortcut)
            )
        },
    );
    let tooltip = if action == Action::ClearCanvas {
        "Clear Canvas - replace all layers with a blank background. Undo restores them.".to_owned()
    } else {
        tooltip
    };
    ui.add_enabled_ui(action.enabled(ctx), |ui| {
        crate::widgets::icon_button_artwork(
            ui,
            label,
            icon,
            action.checked(ctx).unwrap_or(false),
            false,
            tooltip,
            artwork,
        )
    })
    .inner
    .clicked()
}

#[cfg(test)]
mod tests {
    #[test]
    fn pan_is_studio_only_and_h_has_no_modifiers() {
        use super::*;
        let pan = Action::SelectTool(Tool::Pan);
        assert!(pan.available(Frontend::Studio));
        assert!(!pan.available(Frontend::Classic));
        assert_eq!(
            Action::from_shortcut_for(Frontend::Studio, Key::H, Modifiers::NONE),
            Some(pan)
        );
        assert_eq!(
            Action::from_shortcut_for(Frontend::Classic, Key::H, Modifiers::NONE),
            None
        );
        assert_eq!(
            Action::from_shortcut_for(Frontend::Studio, Key::H, Modifiers::ALT),
            None
        );
    }

    #[test]
    fn studio_zoom_shortcut_is_plain_z_and_keeps_classic_routing() {
        use super::*;
        let action = Action::SelectTool(Tool::Magnifier);
        assert_eq!(
            Action::from_shortcut_for(Frontend::Studio, Key::Z, Modifiers::NONE),
            Some(action)
        );
        assert_eq!(
            Action::from_shortcut_for(Frontend::Classic, Key::Z, Modifiers::NONE),
            None
        );
        assert_eq!(
            Action::from_shortcut_for(Frontend::Studio, Key::Z, Modifiers::COMMAND),
            Some(Action::Undo)
        );
        assert_eq!(action.descriptor_for(Frontend::Studio).label, "Zoom tool");
        assert_eq!(action.descriptor_for(Frontend::Classic).label, "Magnifier");
    }

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
            can_recent: false,
            can_save_all: true,
            selection_active: true,
            tool: Tool::Brush,
            blocked: false,
            layer_index: None,
            layer_count: 0,
            layer_visible: false,
            zoom: 1.0,
            pixel_grid: false,
            can_fit_selection: false,
            can_crop_selection: false,
            can_erase_selection: false,
            can_transform_image: true,
            status_bar: true,
            rulers: false,
            fullscreen: false,
            auto_hide_toolbar: false,
            thumbnail: false,
        };
        let ids: std::collections::HashSet<_> = FILE
            .iter()
            .chain(EDIT)
            .map(|a| a.activation_code())
            .collect();
        assert_eq!(ids.len(), FILE.len() + EDIT.len());
        assert!(!ids.contains(&Action::Exit.activation_code()));
        let mut ids = ids;
        for slot in 0..=u8::MAX {
            for action in [
                Action::EditCustomColor(slot),
                Action::EditSecondaryCustomColor(slot),
            ] {
                assert!(ids.insert(action.activation_code()));
                assert!(!action.enabled(context));
                assert!(action.enabled(Context {
                    frontend: Frontend::Classic,
                    document_open: true,
                    ..context
                }));
                assert!(!action.enabled(Context {
                    frontend: Frontend::Classic,
                    document_open: true,
                    blocked: true,
                    ..context
                }));
            }
        }
        for format in ImageExport::ALL {
            let action = Action::ExportImage(format);
            assert!(ids.insert(action.activation_code()));
            assert!(!action.enabled(context));
            assert!(action.enabled(Context {
                document_open: true,
                ..context
            }));
            assert!(!action.enabled(Context {
                document_open: true,
                blocked: true,
                ..context
            }));
        }
        assert!(
            Action::Exit.enabled(context),
            "Exit remains available without a document"
        );
        assert!(!Action::Exit.enabled(Context {
            blocked: true,
            ..context
        }));
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

#[cfg(test)]
mod recent_tests;
#[cfg(test)]
mod save_all_tests;

#[cfg(test)]
mod selection_tests;
