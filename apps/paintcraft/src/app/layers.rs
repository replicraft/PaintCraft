//! Studio layer actions reuse typed engine transactions and stable layer IDs.
use super::*;
use paintcraft_ui::actions::LayerAction;

impl App {
    pub(super) fn dispatch_layer_action(&mut self, ctx: &egui::Context, action: LayerAction) {
        self.resolve_tab_input(ctx);
        let command = if action == LayerAction::Add {
            Some(Action::AddLayer)
        } else {
            let Some(id) = self.editor.target() else {
                return;
            };
            let doc = self.editor.document();
            let Some(index) = doc.roots.iter().position(|r| *r == id) else {
                return;
            };
            let top = doc.roots.len() - 1;
            match action {
                LayerAction::Duplicate => Some(Action::DuplicateLayer { layer: id }),
                LayerAction::Delete => Some(Action::DeleteLayer { layer: id }),
                LayerAction::ToggleVisibility => Some(Action::SetVisible {
                    layer: id,
                    visible: !doc.layers[&id].visible,
                }),
                LayerAction::MoveTop
                | LayerAction::MoveUp
                | LayerAction::MoveDown
                | LayerAction::MoveBottom => {
                    let to_index = match action {
                        LayerAction::MoveTop => top,
                        LayerAction::MoveUp => (index + 1).min(top),
                        LayerAction::MoveDown => index.saturating_sub(1),
                        _ => 0,
                    };
                    Some(Action::MoveLayer {
                        layer: id,
                        to_index,
                    })
                }
                LayerAction::GoTop
                | LayerAction::GoAbove
                | LayerAction::GoBelow
                | LayerAction::GoBottom => {
                    let next = match action {
                        LayerAction::GoTop => top,
                        LayerAction::GoAbove => (index + 1).min(top),
                        LayerAction::GoBelow => index.saturating_sub(1),
                        _ => 0,
                    };
                    let next_id = doc.roots[next];
                    self.editor.set_target(next_id);
                    self.name_edit = None;
                    self.opacity_edit = None;
                    None
                }
                LayerAction::Add => unreachable!(),
            }
        };
        if let Some(command) = command {
            self.request(
                ctx,
                paintcraft_ui::actions::Action::Layer(action).activation_code(),
                command,
            );
        }
    }
}
