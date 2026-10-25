//! Display actions change presentation, never document publication or history.
use super::*;

#[cfg(test)]
mod tests;

impl WebApp {
    pub(crate) fn rulers_visible(&self) -> bool {
        self.frontend == Frontend::Classic && self.preferences.rulers
    }

    pub(super) fn view_action(&mut self, ctx: &egui::Context, view: ViewAction) {
        let mut persist = false;
        match view {
            ViewAction::PixelGrid => self.canvas.pixel_grid = !self.canvas.pixel_grid,
            ViewAction::Thumbnail => self.workspace.thumbnail = !self.workspace.thumbnail,
            ViewAction::Rulers => {
                self.editor.tools.cancel_gesture();
                self.preferences.rulers = !self.preferences.rulers;
                persist = true;
            }
            ViewAction::StatusBar => {
                self.preferences.status_bar = !self.preferences.status_bar;
                persist = true;
            }
            ViewAction::AlwaysShowToolbar | ViewAction::AutoHideToolbar => {
                self.preferences.auto_hide_toolbar = view == ViewAction::AutoHideToolbar;
                persist = true;
            }
            ViewAction::SetZoom(percent) => self.zoom = percent as f32 / 10_000.0,
            ViewAction::ZoomIn => self.zoom *= 1.25,
            ViewAction::ZoomOut => self.zoom /= 1.25,
            ViewAction::ActualSize => self.zoom = 1.0,
            ViewAction::Double => self.zoom = 2.0,
            ViewAction::Quadruple => self.zoom = 4.0,
            ViewAction::Octuple => self.zoom = 8.0,
            ViewAction::FitWindow => self.fit_canvas(),
            ViewAction::FitSelection => self.fit_selection(),
            ViewAction::FullScreen => {
                self.editor.cancel_floating();
                self.editor.tools.cancel_gesture();
                self.fullscreen.toggle(ctx);
            }
        }
        if persist {
            self.persist_preferences();
        }
        ctx.request_repaint();
    }

    pub(super) fn show_thumbnail(&mut self, ctx: &egui::Context) {
        if !self.workspace.thumbnail {
            return;
        }
        let (w, h) = self.editor.session.document().canvas;
        let preview = self.canvas.texture.as_ref().map_or(
            paintcraft_classic::thumbnail::Preview::Unavailable,
            |texture| paintcraft_classic::thumbnail::Preview::Ready {
                texture: texture.id(),
                aspect: w as f32 / h as f32,
                uv: egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            },
        );
        if paintcraft_classic::thumbnail::show(ctx, preview, false, !self.blocked()) {
            self.workspace.thumbnail = false;
        }
    }
}
