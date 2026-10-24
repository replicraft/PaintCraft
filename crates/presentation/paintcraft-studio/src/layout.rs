//! Persistable Studio utility-window preferences, independent of document state.

use crate::Workspace;

const NAMES: [&str; 4] = [
    "studio-tools",
    "studio-colors",
    "studio-layers",
    "studio-history",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    pub visible: bool,
    /// Logical screen coordinates. Window constraints recover off-screen positions.
    pub position: Option<[i32; 2]>,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            visible: true,
            position: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    pub panels: [Panel; 4],
}

impl Layout {
    pub fn restore_visibility(&self, workspace: &mut Workspace) {
        [
            workspace.tools,
            workspace.colors,
            workspace.layers,
            workspace.history,
        ] = self.panels.each_ref().map(|panel| panel.visible);
    }

    /// Supply initial positions only. Existing egui drag state always takes precedence.
    pub fn install(&self, ctx: &egui::Context) {
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::unique("studio-window-layout"), self.clone())
        });
    }

    pub fn capture(&self, ctx: &egui::Context, workspace: &Workspace) -> Self {
        let mut result = self.clone();
        let visible = [
            workspace.tools,
            workspace.colors,
            workspace.layers,
            workspace.history,
        ];
        for (index, panel) in result.panels.iter_mut().enumerate() {
            panel.visible = visible[index];
            if visible[index]
                && let Some(area) = egui::AreaState::load(ctx, egui::Id::unique(NAMES[index]))
            {
                let position = area.left_top_pos();
                if position.is_finite() {
                    panel.position = Some(
                        [position.x.round() as i32, position.y.round() as i32]
                            .map(|coordinate| coordinate.clamp(-32768, 32768)),
                    );
                }
            }
        }
        result
    }
}

pub(crate) fn position(ctx: &egui::Context, name: &str, default: egui::Pos2) -> egui::Pos2 {
    ctx.data(|data| {
        let layout = data.get_temp::<Layout>(egui::Id::unique("studio-window-layout"))?;
        let index = NAMES.iter().position(|candidate| *candidate == name)?;
        let [x, y] = layout.panels[index].position?;
        Some(egui::pos2(x as f32, y as f32))
    })
    .unwrap_or(default)
}

#[cfg(test)]
mod tests;
