//! Familiar menus backed by the shared enabled-state and host action contracts.
use crate::{Intent, Workspace, surface::with_style};
use paintcraft_core::types::orthogonal::OrthogonalTransform as Transform;
use paintcraft_ui::{
    actions::{self, Action, Context, ImageExport, ViewAction},
    t,
};

fn item(ui: &mut egui::Ui, action: Action, context: Context, intents: &mut Vec<Intent>) {
    if actions::menu_item(ui, action, context) {
        intents.push(Intent::Action(action));
        ui.close();
    }
}

pub fn menu_bar(ui: &mut egui::Ui, workspace: &mut Workspace, context: Context) -> Vec<Intent> {
    menu_bar_inner(ui, workspace, context, true)
}
/// Browser hosts have no recent-file provider; this is an explicit capability.
pub fn menu_bar_for_browser(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    context: Context,
) -> Vec<Intent> {
    menu_bar_inner(ui, workspace, context, false)
}
fn menu_bar_inner(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    context: Context,
    include_recent: bool,
) -> Vec<Intent> {
    let mut intents = Vec::new();
    with_style(ui, |ui| {
        ui.add_enabled_ui(!context.blocked, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(t!(ui.ctx(), "File"), |ui| {
                    for action in [
                        Action::New,
                        Action::Open,
                        Action::RecentFiles,
                        Action::Save,
                        Action::SaveAs,
                    ] {
                        if action != Action::RecentFiles || include_recent {
                            item(ui, action, context, &mut intents);
                        }
                    }
                    ui.menu_button(t!(ui.ctx(), "Export"), |ui| {
                        for format in [ImageExport::Bmp, ImageExport::Png] {
                            item(ui, Action::ExportImage(format), context, &mut intents);
                        }
                    });
                    item(ui, Action::Print, context, &mut intents);
                    if context.can_exit {
                        ui.separator();
                        item(ui, Action::Exit, context, &mut intents);
                    }
                });
                ui.menu_button(t!(ui.ctx(), "Edit"), |ui| {
                    for action in [
                        Action::Undo,
                        Action::Redo,
                        Action::Cut,
                        Action::Copy,
                        Action::Paste,
                        Action::EraseSelection,
                        Action::SelectAll,
                        Action::Deselect,
                    ] {
                        item(ui, action, context, &mut intents);
                    }
                });
                ui.menu_button(t!(ui.ctx(), "View"), |ui| {
                    ui.checkbox(&mut workspace.toolbox, t!(ui.ctx(), "Tools"));
                    ui.checkbox(&mut workspace.colors, t!(ui.ctx(), "Colors"));
                    ui.checkbox(&mut workspace.status, t!(ui.ctx(), "Status bar"));
                    ui.checkbox(&mut workspace.text_toolbar, t!(ui.ctx(), "Text toolbar"));
                    ui.separator();
                    for view in [
                        ViewAction::ActualSize,
                        ViewAction::ZoomIn,
                        ViewAction::ZoomOut,
                        ViewAction::FitWindow,
                        ViewAction::FitSelection,
                        ViewAction::PixelGrid,
                        ViewAction::FullScreen,
                    ] {
                        item(ui, Action::View(view), context, &mut intents);
                    }
                });
                ui.menu_button(t!(ui.ctx(), "Image"), |ui| {
                    for transform in [
                        Transform::FlipHorizontal,
                        Transform::FlipVertical,
                        Transform::RotateRight,
                        Transform::RotateLeft,
                        Transform::Rotate180,
                    ] {
                        item(ui, Action::TransformImage(transform), context, &mut intents);
                    }
                    for action in [
                        Action::ResizeSkew,
                        Action::ImageProperties,
                        Action::Crop,
                        Action::ClearCanvas,
                    ] {
                        item(ui, action, context, &mut intents);
                    }
                });
                ui.menu_button(t!(ui.ctx(), "Colors"), |ui| {
                    item(ui, Action::EditColors, context, &mut intents);
                    item(ui, Action::EditSecondaryColors, context, &mut intents);
                });
                ui.menu_button(t!(ui.ctx(), "Help"), |ui| {
                    ui.hyperlink_to(
                        t!(ui.ctx(), "Documentation"),
                        "https://github.com/replicraft/PaintCraft#readme",
                    );
                    if ui.button(t!(ui.ctx(), "About")).clicked() {
                        intents.push(Intent::About);
                        ui.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(t!(ui.ctx(), "Settings…")).clicked() {
                        intents.push(Intent::Action(Action::Settings));
                    }
                    intents.extend(
                        paintcraft_ui::chrome::interface_changer(ui, context)
                            .into_iter()
                            .map(Intent::Action),
                    );
                    ui.weak(t!(ui.ctx(), workspace.skin.name()));
                });
            });
        });
    });
    intents
}
