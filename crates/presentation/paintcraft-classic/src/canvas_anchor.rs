//! Scoped, bounded image-point anchors consumed before scroll layout.
use egui::{Context, Id, Pos2, Vec2};
#[derive(Clone, Copy)]
pub(crate) struct Anchor {
    pub scope: Id,
    image: Vec2,
    screen: Pos2,
    remaining: u8,
    stable: u8,
    last_frame: Option<u64>,
}
/// Anchor a logical document point at a logical screen point using the new zoom.
/// The scope describes the current document/view, including zoom if appropriate.
pub fn anchor_at(
    ctx: &Context,
    id: Id,
    scope: Id,
    image_point: Pos2,
    screen_point: Pos2,
    zoom: f32,
) {
    let image = image_point.to_vec2() * zoom;
    if !image_point.is_finite()
        || !screen_point.is_finite()
        || !zoom.is_finite()
        || zoom <= 0.0
        || !image.is_finite()
    {
        return;
    }
    ctx.data_mut(|data| {
        data.insert_temp(
            id.with("classic-anchor-request"),
            Anchor {
                scope,
                image,
                screen: screen_point,
                remaining: 32,
                stable: 0,
                last_frame: None,
            },
        )
    });
    ctx.request_repaint();
}
impl Anchor {
    pub fn offset(self, viewport: Pos2, leading: Vec2, maximum: Vec2) -> Vec2 {
        (viewport.to_vec2() + leading + self.image - self.screen.to_vec2())
            .max(Vec2::ZERO)
            .min(maximum)
    }
    /// Scrollbar appearance uses the previous frame's extent. Wait for two
    /// stable measurements, with a fixed bound even for pathological geometry.
    pub fn settle(
        mut self,
        predicted: Vec2,
        actual: Vec2,
        frame: u64,
        position_agrees: bool,
    ) -> Option<Self> {
        if self.last_frame == Some(frame) {
            if (predicted - actual).length() > 0.5 || !position_agrees {
                self.stable = 0;
            }
            return Some(self);
        }
        self.last_frame = Some(frame);
        self.remaining = self.remaining.saturating_sub(1);
        self.stable = if (predicted - actual).length() <= 0.5 && position_agrees {
            self.stable + 1
        } else {
            0
        };
        (self.remaining > 0 && self.stable < 2).then_some(self)
    }
}
