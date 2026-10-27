//! Shared explicit DDS choices. Hosts own admission, rendering, conversion and I/O.
//! No format-service dependency: these values must be mapped by an admitted host.
use crate::t;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Rgba8,
    Bc1,
    Bc2,
    Bc3,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transfer {
    #[default]
    Srgb,
    Linear8,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mips {
    #[default]
    None,
    Full,
    /// Includes the original image as level one.
    Custom(u8),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    Nearest,
    BoxStored,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    #[default]
    Normal,
    High,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub encoding: Encoding,
    pub transfer: Transfer,
    pub mips: Mips,
    pub filter: Filter,
    pub quality: Quality,
}
/// Geometry only; format limits and resource admission remain the host's job.
pub fn maximum_levels(width: u32, height: u32) -> u8 {
    if width == 0 || height == 0 {
        0
    } else {
        (u32::BITS - width.max(height).leading_zeros()) as u8
    }
}
impl Settings {
    pub fn level_count(self, width: u32, height: u32) -> Option<u8> {
        let maximum = maximum_levels(width, height);
        if maximum == 0 {
            return None;
        }
        match self.mips {
            Mips::None => Some(1),
            Mips::Full => Some(maximum),
            Mips::Custom(count) if (1..=maximum).contains(&count) => Some(count),
            Mips::Custom(_) => None,
        }
    }
    fn normalize(&mut self, width: u32, height: u32) {
        if let Mips::Custom(count) = &mut self.mips {
            *count = (*count).clamp(1, maximum_levels(width, height).max(1));
        }
        // An inactive effort choice must not silently affect uncompressed output.
        if self.encoding == Encoding::Rgba8 {
            self.quality = Quality::Normal;
        }
    }
}
fn encoding_label(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Rgba8 => "RGBA8 (uncompressed)",
        Encoding::Bc1 => "BC1 (opaque)",
        Encoding::Bc2 => "BC2 (straight alpha)",
        Encoding::Bc3 => "BC3 (straight alpha)",
    }
}
fn mip_label(mips: Mips) -> &'static str {
    match mips {
        Mips::None => "Single level",
        Mips::Full => "Full mip chain",
        Mips::Custom(_) => "Custom level count",
    }
}
fn filter_label(filter: Filter) -> &'static str {
    match filter {
        Filter::Nearest => "Nearest sample",
        Filter::BoxStored => "Box (alpha-weighted stored samples)",
    }
}
fn quality_label(quality: Quality) -> &'static str {
    match quality {
        Quality::Normal => "Normal effort",
        Quality::High => "High effort",
    }
}
/// Returns true on any settings change, including geometry-driven normalization.
/// Hosts must retire an earlier prepared preview/consent when this returns true.
pub fn controls(ui: &mut egui::Ui, settings: &mut Settings, width: u32, height: u32) -> bool {
    let before = *settings;
    settings.normalize(width, height);
    let ctx = ui.ctx().clone();
    egui::ComboBox::from_label(t!(&ctx, "DDS encoding"))
        .selected_text(t!(&ctx, encoding_label(settings.encoding)))
        .show_ui(ui, |ui| {
            for encoding in [Encoding::Rgba8, Encoding::Bc1, Encoding::Bc2, Encoding::Bc3] {
                ui.selectable_value(
                    &mut settings.encoding,
                    encoding,
                    t!(&ctx, encoding_label(encoding)),
                );
            }
        });
    match settings.encoding {
        Encoding::Rgba8 => {
            ui.label(t!(
                &ctx,
                "RGBA8 stores each pixel without block compression."
            ));
        }
        Encoding::Bc1 => {
            ui.label(t!(
                &ctx,
                "BC1 changes colors and refuses pictures with transparency. No background is added."
            ));
        }
        Encoding::Bc2 | Encoding::Bc3 => {
            ui.label(t!(
                &ctx,
                "BC2 and BC3 store straight alpha. Compression can change color and transparency."
            ));
        }
    }
    egui::ComboBox::from_label(t!(&ctx, "DDS color samples"))
        .selected_text(t!(
            &ctx,
            match settings.transfer {
                Transfer::Srgb => "sRGB samples",
                Transfer::Linear8 => "Linear 8-bit samples",
            }
        ))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut settings.transfer,
                Transfer::Srgb,
                t!(&ctx, "sRGB samples"),
            );
            ui.selectable_value(
                &mut settings.transfer,
                Transfer::Linear8,
                t!(&ctx, "Linear 8-bit samples"),
            );
        });
    ui.label(t!(&ctx, match settings.transfer {
        Transfer::Srgb => "sRGB keeps the document's stored color samples.",
        Transfer::Linear8 => "Linear 8-bit converts sRGB samples and can lose color precision. Transparency is unchanged by this conversion.",
    }));
    egui::ComboBox::from_label(t!(&ctx, "Mip levels"))
        .selected_text(t!(&ctx, mip_label(settings.mips)))
        .show_ui(ui, |ui| {
            let custom = match settings.mips {
                Mips::Custom(n) => n,
                _ => 1,
            };
            for mode in [Mips::None, Mips::Full, Mips::Custom(custom)] {
                ui.selectable_value(&mut settings.mips, mode, t!(&ctx, mip_label(mode)));
            }
        });
    if let Mips::Custom(count) = &mut settings.mips {
        ui.add_enabled(
            maximum_levels(width, height) > 0,
            egui::Slider::new(count, 1..=maximum_levels(width, height).max(1))
                .text(t!(&ctx, "Level count (including base image)")),
        );
    }
    if settings.level_count(width, height).is_some_and(|n| n > 1) {
        egui::ComboBox::from_label(t!(&ctx, "Mip filter"))
            .selected_text(t!(&ctx, filter_label(settings.filter)))
            .show_ui(ui, |ui| {
                for filter in [Filter::Nearest, Filter::BoxStored] {
                    ui.selectable_value(
                        &mut settings.filter,
                        filter,
                        t!(&ctx, filter_label(filter)),
                    );
                }
            });
        ui.label(t!(&ctx, "Smaller mip images are derived from the base. Filtering uses the chosen stored samples."));
    }
    if settings.encoding != Encoding::Rgba8 {
        egui::ComboBox::from_label(t!(&ctx, "Encoding effort"))
            .selected_text(t!(&ctx, quality_label(settings.quality)))
            .show_ui(ui, |ui| {
                for quality in [Quality::Normal, Quality::High] {
                    ui.selectable_value(
                        &mut settings.quality,
                        quality,
                        t!(&ctx, quality_label(quality)),
                    );
                }
            });
        ui.label(t!(&ctx, "Higher effort takes longer. It does not guarantee an error limit or lossless compression."));
    }
    if maximum_levels(width, height) == 0 {
        ui.colored_label(
            ui.visuals().error_fg_color,
            t!(&ctx, "DDS export requires a nonempty image."),
        );
    }
    settings.normalize(width, height);
    *settings != before
}

#[cfg(test)]
#[path = "dds_export/tests.rs"]
mod tests;
