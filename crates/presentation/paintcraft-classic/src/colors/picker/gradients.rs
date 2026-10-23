//! Bounded gradient textures let egui antialias rounded picker edges.
use super::*;

const SIDE: usize = 256;

fn hue_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::unique("classic-hue-saturation-gradient");
    if let Some(texture) = ctx.data_mut(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let pixels = (0..SIDE)
        .flat_map(|y| {
            (0..SIDE).map(move |x| {
                color(
                    x as f32 / (SIDE - 1) as f32,
                    1.0 - y as f32 / (SIDE - 1) as f32,
                    1.0,
                )
            })
        })
        .collect();
    let texture = ctx.load_texture(
        "classic-hue-saturation",
        egui::ColorImage::new([SIDE, SIDE], pixels),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

#[derive(Clone)]
struct BrightnessTexture {
    key: (u32, u32),
    texture: egui::TextureHandle,
}

fn brightness_texture(ctx: &egui::Context, hue: f32, saturation: f32) -> egui::TextureHandle {
    let id = egui::Id::unique("classic-brightness-gradient");
    let key = (hue.to_bits(), saturation.to_bits());
    let cached = ctx.data_mut(|data| data.get_temp::<BrightnessTexture>(id));
    if let Some(cache) = &cached
        && cache.key == key
    {
        return cache.texture.clone();
    }
    let image = egui::ColorImage::new(
        [1, SIDE],
        (0..SIDE)
            .map(|y| color(hue, saturation, 1.0 - y as f32 / (SIDE - 1) as f32))
            .collect(),
    );
    let texture = if let Some(mut cache) = cached {
        cache.texture.set(image, egui::TextureOptions::LINEAR);
        cache.texture
    } else {
        ctx.load_texture("classic-brightness", image, egui::TextureOptions::LINEAR)
    };
    ctx.data_mut(|data| {
        data.insert_temp(
            id,
            BrightnessTexture {
                key,
                texture: texture.clone(),
            },
        )
    });
    texture
}

pub(super) fn square(ui: &egui::Ui, rect: egui::Rect) {
    paint(ui, rect, 4, &hue_texture(ui.ctx()));
}

pub(super) fn brightness(ui: &egui::Ui, rect: egui::Rect, hue: f32, saturation: f32) {
    paint(ui, rect, 8, &brightness_texture(ui.ctx(), hue, saturation));
}

fn paint(ui: &egui::Ui, rect: egui::Rect, radius: u8, texture: &egui::TextureHandle) {
    ui.painter().add(
        egui::epaint::RectShape::filled(rect, radius, Color32::WHITE).with_texture(
            texture.id(),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradients_keep_bounded_resident_identity_when_colors_change() {
        let ctx = egui::Context::default();
        let square = hue_texture(&ctx);
        assert_eq!(square.size(), [SIDE, SIDE]);
        assert_eq!(hue_texture(&ctx).id(), square.id());
        let brightness = brightness_texture(&ctx, 0.0, 1.0);
        assert_eq!(brightness.size(), [1, SIDE]);
        for hue in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(brightness_texture(&ctx, hue, 0.5).id(), brightness.id());
        }
        assert_ne!(square.id(), brightness.id());
    }
}
