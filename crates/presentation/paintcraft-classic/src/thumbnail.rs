//! Floating full-canvas overview. The host supplies prepared canvas pixels and owns jobs.
pub use crate::layers::Preview;
use egui::{Color32, Rect, Vec2};

/// Returns true when the user closes the window. This view never edits pixels.
pub fn show(ctx: &egui::Context, preview: Preview, updating: bool, enabled: bool) -> bool {
    let mut close = false;
    let max = (ctx.content_rect().size() - Vec2::splat(32.0)).max(Vec2::new(128.0, 100.0));
    egui::Window::new("Thumbnail")
        .id(egui::Id::unique("classic-thumbnail-window"))
        .title_bar(false)
        .enabled(enabled)
        .default_pos(egui::pos2(
            (ctx.content_rect().right() - 332.0).max(ctx.content_rect().left() + 16.0),
            ctx.content_rect().top() + 224.0_f32.min(max.y * 0.5),
        ))
        .default_size(Vec2::new(300.0, 236.0).min(max))
        .min_size(Vec2::new(128.0, 100.0))
        .max_size(max)
        .resizable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().interact_size.y = 40.0;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Thumbnail").size(16.0).color(ui.visuals().strong_text_color()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = paintcraft_ui::widgets::classic_menu_button(
                        ui,
                        "Close thumbnail",
                        paintcraft_ui::icons::Icon::Close,
                        false,
                        "Close thumbnail",
                    ).clicked();
                });
            });
            let available = Vec2::new(ui.available_width(), (ui.available_height() - 24.0).max(24.0));
            let (rect, response) = ui.allocate_exact_size(available, egui::Sense::hover());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, enabled, "Canvas thumbnail"));
            ui.painter().rect_filled(rect, 0, paintcraft_ui::theme::workspace(ctx.theme()));
            match preview {
                Preview::Ready { texture, aspect, uv } => {
                    let size = Vec2::new(rect.width().min(rect.height() * aspect), rect.height().min(rect.width() / aspect));
                    let image = Rect::from_center_size(rect.center(), size);
                    checkerboard(ui, image);
                    ui.painter().image(texture, image, uv, Color32::WHITE);
                }
                Preview::Pending | Preview::Unavailable => {
                    ui.painter().text(
                        rect.center(), egui::Align2::CENTER_CENTER,
                        if matches!(preview, Preview::Pending) { "Updating…" } else { "Preview unavailable" },
                        egui::FontId::proportional(14.0), ui.visuals().text_color(),
                    );
                }
            }
            ui.label(egui::RichText::new(if updating { "Updating preview…" } else { "Canvas overview" }).size(12.0)).on_hover_text("Shows the canvas and active paint, shape and text previews. Selection borders and text carets stay on the main canvas.");
        });
    close
}

fn checkerboard(ui: &egui::Ui, image: Rect) {
    // A repeating 2x2 texture keeps presentation geometry bounded as the user
    // enlarges the window. Keep at most one tiny handle per theme in this context.
    let dark = ui.visuals().dark_mode;
    let id = egui::Id::unique(("classic-thumbnail-checkerboard", dark));
    let texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let [a, b] = if dark { [65, 95] } else { [205, 245] }.map(Color32::from_gray);
            let texture = ui.ctx().load_texture(
                "thumbnail-transparency-grid",
                egui::ColorImage::new([2, 2], vec![a, b, b, a]),
                egui::TextureOptions::NEAREST_REPEAT,
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    ui.painter().image(
        texture.id(),
        image,
        Rect::from_min_size(egui::Pos2::ZERO, image.size() / 24.0),
        Color32::WHITE,
    );
}
