//! Stable scrolling popup bounds, independent of egui's moving area estimate.
pub(super) fn height(ui: &mut egui::Ui, minimum: f32, maximum: f32) -> f32 {
    // A popup may be moved above its trigger while egui measures it. Deriving
    // height from that moving cursor makes the viewport shrink each frame.
    // Reserve the main menu row (32/40 points), its outer margins, the popup
    // frame and edge clearance. The heading stays reachable for menu switching.
    let screen_height = ui.ctx().content_rect().height();
    let clearance = if screen_height < 420.0 { 64.0 } else { 72.0 };
    let height = (screen_height - clearance).clamp(minimum, maximum);
    ui.set_max_height(height);
    ui.spacing_mut().scroll.floating_allocated_width = 4.0;
    ui.spacing_mut().scroll.dormant_handle_opacity = 0.6;
    height
}
