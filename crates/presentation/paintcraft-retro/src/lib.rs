//! Historical presentation that emits shared editor intentions, never pixels.
mod anchor;
mod canvas;
mod menus;
mod palette;
mod status;
mod surface;
mod tools;
mod workspace;

pub use anchor::anchor_at;
pub use canvas::{canvas, pan_by};
pub use menus::{menu_bar, menu_bar_for_browser};
pub use palette::{PALETTE, palette};
pub use status::{Status, status};
pub use surface::{frame, with_style, workspace_color};
pub use tools::{ToolChoice, toolbox};
pub use workspace::{Skin, Workspace};

/// An actual host operation; the host validates its current document before execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Action(paintcraft_ui::actions::Action),
    /// Apply only after the host accepts text, floating selection and tool transition.
    ChooseTool(ToolChoice),
    /// Open the host's real About settings page.
    About,
}

#[cfg(test)]
mod tests;
