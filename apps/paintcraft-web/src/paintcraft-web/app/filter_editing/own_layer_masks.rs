//! Own-mask actions publish on Main, then reopen ordinary Source7 snapshots.
//! They never enter the numeric filter-preview operation protocol.
use super::*;
use paintcraft_core::types::ErrorCode;
use paintcraft_tools::LayerMaskEdit;
impl WebApp {
    pub(crate) fn apply_own_layer_mask(&mut self, ctx: &egui::Context, edit: LayerMaskEdit) {
        if self.frontend != Frontend::Studio || self.blocked() || self.filter_private_input() {
            self.message = t!(
                ctx,
                "Finish or cancel the current action before editing layer masks"
            );
            return;
        }
        let before = self.editor.session.ticket();
        match self.editor.apply(Edit::LayerMask(edit)) {
            Ok(()) => {
                if self.editor.session.ticket() != before {
                    // reset only retires desired output. Existing transport
                    // retains its source/Main credits until normal close ACK.
                    self.filter_display.renderer.reset();
                    self.filter_display.source = None;
                    self.filter_display.committed = None;
                    self.filter_display.texture = None;
                    self.filter_display.footprint = None;
                    self.filter_display.view = None;
                    self.canvas.texture = None;
                    self.canvas.filter_footprint = None;
                    self.canvas.invalidate();
                    self.message = t!(ctx, "Layer mask updated");
                    ctx.request_repaint();
                } else {
                    self.message = t!(ctx, "Layer mask unchanged");
                }
            }
            Err(error) => {
                let source = if error.code == ErrorCode::ResourceLimit {
                    "Layer mask limits were exceeded"
                } else {
                    error.message.as_str()
                };
                self.message = t!(ctx, "Edit failed: { $detail }", detail = t!(ctx, source));
            }
        }
    }
}
#[cfg(test)]
mod tests;
