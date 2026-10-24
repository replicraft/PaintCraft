//! Actions use only icons for operations that already have real implementations.
use super::Icon;
use crate::actions::{Action, Context, LayerAction, ViewAction};

impl Action {
    /// Classic presentation can refine a shared action's default artwork.
    pub fn classic_menu_icon(self, ctx: Context) -> Option<Icon> {
        if self == Self::SelectAll {
            Some(Icon::SelectAll)
        } else {
            self.icon(ctx)
        }
    }

    pub fn icon(self, ctx: Context) -> Option<Icon> {
        Some(match self {
            Self::Settings => Icon::Settings,
            Self::New => Icon::New,
            Self::Open => Icon::Open,
            Self::Save => Icon::Save,
            Self::SaveAs | Self::Export => Icon::SaveAs,
            Self::Close | Self::Exit => Icon::Close,
            Self::Undo => Icon::Undo,
            Self::Redo => Icon::Redo,
            Self::SelectAll | Self::Deselect => Icon::RectangleSelect,
            Self::ChooseFrontend(_)
            | Self::Interface
            | Self::CanvasDefaults
            | Self::SelectTool(_) => {
                return None;
            }
            Self::CanvasSize => Icon::CanvasSize,
            Self::ResizeSkew => Icon::Resize,
            Self::ImageProperties => Icon::ImageProperties,
            Self::ClearCanvas => Icon::ClearCanvas,
            Self::EditColors | Self::EditSecondaryColors | Self::EditCustomColor(_) => {
                Icon::EditColors
            }
            Self::View(action) => match action {
                ViewAction::ZoomIn => Icon::ZoomIn,
                ViewAction::ZoomOut => Icon::ZoomOut,
                ViewAction::FitWindow | ViewAction::FitSelection => Icon::FitWindow,
                ViewAction::ActualSize => Icon::ActualSize,
                ViewAction::PixelGrid => Icon::PixelGrid,
                ViewAction::StatusBar
                | ViewAction::AlwaysShowToolbar
                | ViewAction::AutoHideToolbar => return None,
                ViewAction::Rulers => Icon::Rulers,
                ViewAction::FullScreen => Icon::Resize,
                ViewAction::Double
                | ViewAction::Quadruple
                | ViewAction::Octuple
                | ViewAction::SetZoom(_) => Icon::ZoomIn,
            },
            Self::Layer(action) => match action {
                LayerAction::Add => Icon::AddLayer,
                LayerAction::Delete => Icon::DeleteLayer,
                LayerAction::Duplicate => Icon::DuplicateLayer,
                LayerAction::ToggleVisibility => {
                    if ctx.layer_visible {
                        Icon::HideLayer
                    } else {
                        Icon::ShowLayer
                    }
                }
                LayerAction::GoTop
                | LayerAction::GoAbove
                | LayerAction::MoveTop
                | LayerAction::MoveUp => Icon::MoveLayerUp,
                LayerAction::GoBottom
                | LayerAction::GoBelow
                | LayerAction::MoveBottom
                | LayerAction::MoveDown => Icon::MoveLayerDown,
                LayerAction::Properties => return None,
            },
        })
    }
}
