//! Shared asset-export controls. Hosts own preparation, source guards and I/O.
use crate::t;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Icon,
    Cursor,
}
impl Kind {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Icon => "ico",
            Self::Cursor => "cur",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Icon => "ICO",
            Self::Cursor => "CUR",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Png,
    Bitmap,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    Lanczos3,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub width: u16,
    pub height: u16,
    pub encoding: Encoding,
    pub hotspot: (u16, u16),
}
impl Default for Entry {
    fn default() -> Self {
        Self {
            width: 32,
            height: 32,
            encoding: Encoding::Png,
            hotspot: (0, 0),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub kind: Kind,
    pub filter: Filter,
    pub entries: Vec<Entry>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            kind: Kind::Icon,
            filter: Filter::Lanczos3,
            entries: [16, 32, 48, 256]
                .map(|size| Entry {
                    width: size,
                    height: size,
                    ..Default::default()
                })
                .into(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub config: Config,
    pub selected: usize,
    pub accepted: bool,
}

pub fn controls(ui: &mut egui::Ui, draft: &mut Draft) {
    let ctx = ui.ctx().clone();
    egui::ComboBox::from_label(t!(&ctx, "File type"))
        .selected_text(t!(&ctx, kind_label(draft.config.kind)))
        .show_ui(ui, |ui| {
            for kind in [Kind::Icon, Kind::Cursor] {
                ui.selectable_value(&mut draft.config.kind, kind, t!(&ctx, kind_label(kind)));
            }
        });
    ui.label(t!(
        &ctx,
        "Each entry is fitted independently, preserving aspect ratio on transparent padding."
    ));
    ui.label(t!(&ctx, "Icon entries"));
    if draft.config.entries.is_empty() {
        draft.config.entries.push(Entry::default());
    }
    draft.selected = draft.selected.min(draft.config.entries.len() - 1);
    egui::ScrollArea::vertical()
        .id_salt("icon-export-entries")
        .max_height(108.0)
        .show(ui, |ui| {
            for (index, entry) in draft.config.entries.iter().enumerate() {
                ui.selectable_value(
                    &mut draft.selected,
                    index,
                    format!(
                        "{} × {} {}",
                        entry.width,
                        entry.height,
                        if entry.encoding == Encoding::Png {
                            "PNG"
                        } else {
                            "DIB"
                        }
                    ),
                );
            }
        });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                draft.config.entries.len() < 256,
                egui::Button::new(t!(&ctx, "Add entry")),
            )
            .clicked()
        {
            draft.config.entries.push(Entry::default());
            draft.selected = draft.config.entries.len() - 1;
        }
        if ui
            .add_enabled(
                draft.config.entries.len() > 1,
                egui::Button::new(t!(&ctx, "Remove entry")),
            )
            .clicked()
        {
            draft.config.entries.remove(draft.selected);
            draft.selected = draft.selected.min(draft.config.entries.len() - 1);
        }
    });
    let entry = &mut draft.config.entries[draft.selected];
    ui.horizontal_wrapped(|ui| {
        number(ui, "Width", &mut entry.width, 1..=256);
        number(ui, "Height", &mut entry.height, 1..=256);
    });
    egui::ComboBox::from_label(t!(&ctx, "Entry encoding"))
        .selected_text(t!(&ctx, encoding_label(entry.encoding)))
        .show_ui(ui, |ui| {
            for encoding in [Encoding::Png, Encoding::Bitmap] {
                ui.selectable_value(
                    &mut entry.encoding,
                    encoding,
                    t!(&ctx, encoding_label(encoding)),
                );
            }
        });
    if entry.encoding == Encoding::Bitmap {
        ui.label(t!(&ctx, "Fully transparent entries require PNG encoding."));
    }
    entry.width = entry.width.clamp(1, 256);
    entry.height = entry.height.clamp(1, 256);
    if draft.config.kind == Kind::Cursor {
        entry.hotspot.0 = entry.hotspot.0.min(entry.width.saturating_sub(1));
        entry.hotspot.1 = entry.hotspot.1.min(entry.height.saturating_sub(1));
        ui.horizontal_wrapped(|ui| {
            number(ui, "Hotspot X", &mut entry.hotspot.0, 0..=entry.width - 1);
            number(ui, "Hotspot Y", &mut entry.hotspot.1, 0..=entry.height - 1);
        });
    }
    egui::ComboBox::from_label(t!(&ctx, "Resampling"))
        .selected_text(t!(&ctx, filter_label(draft.config.filter)))
        .show_ui(ui, |ui| {
            for filter in [Filter::Nearest, Filter::Lanczos3] {
                ui.selectable_value(
                    &mut draft.config.filter,
                    filter,
                    t!(&ctx, filter_label(filter)),
                );
            }
        });
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut u16, range: std::ops::RangeInclusive<u16>) {
    let text = t!(ui.ctx(), label);
    let label = ui.label(&text);
    let response = ui
        .add(egui::DragValue::new(value).range(range))
        .labelled_by(label.id);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), &text));
}
fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Icon => "Windows icon (.ico)",
        Kind::Cursor => "Windows cursor (.cur)",
    }
}
fn encoding_label(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Png => "PNG",
        Encoding::Bitmap => "Bitmap (32-bit)",
    }
}
fn filter_label(filter: Filter) -> &'static str {
    match filter {
        Filter::Nearest => "Nearest neighbor",
        Filter::Lanczos3 => "Lanczos3",
    }
}

pub fn loss_description(code: &str) -> Option<&'static str> {
    Some(match code {
        "flatten" => "Visible layers will be flattened into one picture.",
        "hidden-rgb" => "Invisible color information will not be retained.",
        "icon-resized" => "Entry dimensions will change to fit the selected sizes.",
        _ => return None,
    })
}
pub fn consent(ui: &mut egui::Ui, losses: &[String], accepted: &mut bool) -> bool {
    if losses.is_empty() {
        return true;
    }
    for code in losses {
        let Some(text) = loss_description(code) else {
            return false;
        };
        ui.label(t!(ui.ctx(), text));
    }
    ui.checkbox(accepted, t!(ui.ctx(), "I accept these export changes."));
    *accepted
}

#[derive(Debug)]
pub struct Frame<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub hotspot: Option<(u16, u16)>,
}
pub struct Cache<K, P> {
    key: Option<K>,
    prepared: Option<P>,
    texture: Option<egui::TextureHandle>,
    selected: Option<usize>,
    frame: Option<u64>,
}
impl<K, P> std::fmt::Debug for Cache<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IconExportCache")
            .field("ready", &self.prepared.is_some())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}
impl<K, P> Default for Cache<K, P> {
    fn default() -> Self {
        Self {
            key: None,
            prepared: None,
            texture: None,
            selected: None,
            frame: None,
        }
    }
}
impl<K: PartialEq, P> Cache<K, P> {
    pub fn value(&self) -> Option<&P> {
        self.prepared.as_ref()
    }
    /// Return true when a changed source/policy is prepared. Failed keys remain
    /// cached until input changes, avoiding expensive repeated frame work.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        key: K,
        prepare: impl FnOnce() -> Result<P, String>,
    ) -> bool {
        if self.key.as_ref() == Some(&key) {
            return false;
        }
        self.key = Some(key);
        self.prepared = prepare().ok();
        self.texture = None;
        self.selected = None;
        self.frame = None;
        ctx.request_repaint();
        true
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        selected: usize,
        frame: impl for<'a> Fn(&'a P, usize) -> Option<Frame<'a>>,
    ) -> bool {
        self.show_bounded(ui, selected, 256, "Export preview", frame)
    }
    /// ICNS may explicitly preview qualified entries up to 1024 physical pixels.
    /// Existing ICO/CUR callers retain their original 256-pixel allowance.
    pub fn show_bounded(
        &mut self,
        ui: &mut egui::Ui,
        selected: usize,
        side_max: u32,
        label: &'static str,
        frame: impl for<'a> Fn(&'a P, usize) -> Option<Frame<'a>>,
    ) -> bool {
        let Some(frame) = self
            .prepared
            .as_ref()
            .and_then(|prepared| frame(prepared, selected))
        else {
            ui.colored_label(ui.visuals().error_fg_color, t!(ui.ctx(), "Icon preview is unavailable. Check the entry settings or reduce the canvas size."));
            return false;
        };
        if frame.width == 0
            || frame.height == 0
            || frame.width > side_max.min(1024)
            || frame.height > side_max.min(1024)
            || frame.rgba.len() as u64 != u64::from(frame.width) * u64::from(frame.height) * 4
        {
            return false;
        }
        if self.selected != Some(selected) {
            self.texture = Some(ui.ctx().load_texture(
                "icon-export-preview",
                egui::ColorImage::from_rgba_unmultiplied(
                    [frame.width as usize, frame.height as usize],
                    frame.rgba,
                ),
                egui::TextureOptions::NEAREST,
            ));
            self.selected = Some(selected);
            self.frame = None;
            ui.ctx().request_repaint();
        }
        ui.label(t!(ui.ctx(), label));
        if let Some(texture) = &self.texture {
            let size = egui::vec2(frame.width as f32, frame.height as f32);
            let scale = 160.0 / size.x.max(size.y);
            let (rect, _) = ui.allocate_exact_size(size * scale, egui::Sense::hover());
            let shades = if ui.visuals().dark_mode {
                [60, 90]
            } else {
                [210, 240]
            };
            for y in 0..(rect.height() / 8.0).ceil() as usize {
                for x in 0..(rect.width() / 8.0).ceil() as usize {
                    let cell = egui::Rect::from_min_size(
                        rect.min + egui::vec2(x as f32 * 8.0, y as f32 * 8.0),
                        egui::vec2(8.0, 8.0),
                    )
                    .intersect(rect);
                    ui.painter().rect_filled(
                        cell,
                        0.0,
                        egui::Color32::from_gray(shades[(x + y) % 2]),
                    );
                }
            }
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            if self.frame.is_none() && ui.is_rect_visible(rect) {
                self.frame = Some(ui.ctx().cumulative_frame_nr());
                ui.ctx().request_repaint();
            }
            if let Some((x, y)) = frame.hotspot {
                let point = rect.min
                    + rect.size()
                        * egui::vec2(
                            (f32::from(x) + 0.5) / frame.width as f32,
                            (f32::from(y) + 0.5) / frame.height as f32,
                        );
                let stroke = egui::Stroke::new(1.0, ui.visuals().selection.stroke.color);
                ui.painter().line_segment(
                    [point - egui::vec2(5.0, 0.0), point + egui::vec2(5.0, 0.0)],
                    stroke,
                );
                ui.painter().line_segment(
                    [point - egui::vec2(0.0, 5.0), point + egui::vec2(0.0, 5.0)],
                    stroke,
                );
            }
        }
        self.frame
            .is_some_and(|frame| frame < ui.ctx().cumulative_frame_nr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offscreen_preview_does_not_count_as_displayed_before_consent() {
        let ctx = egui::Context::default();
        let mut cache: Cache<u8, Vec<u8>> = Cache::default();
        for (offscreen, expected) in [(true, false), (true, false), (false, false), (false, true)] {
            let mut observed = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 240.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    cache.update(ui.ctx(), 1, || Ok(vec![255; 32 * 32 * 4]));
                    if offscreen {
                        ui.add_space(600.0);
                    }
                    observed.push(cache.show_bounded(
                        ui,
                        0,
                        1024,
                        "Import preview",
                        |pixels, _| {
                            Some(Frame {
                                width: 32,
                                height: 32,
                                rgba: pixels,
                                hotspot: None,
                            })
                        },
                    ));
                },
            );
            output.textures_delta.clear();
            assert!(!observed.is_empty());
            assert!(observed.iter().all(|ready| *ready == expected));
        }
    }
    #[test]
    fn explicit_large_icon_preview_does_not_expand_existing_ico_allowance() {
        let ctx = egui::Context::default();
        let mut cache: Cache<u8, Vec<u8>> = Cache::default();
        for expected in [false, true] {
            let mut observed = Vec::new();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                cache.update(ui.ctx(), 1, || Ok(vec![255; 512 * 512 * 4]));
                assert!(!cache.show(ui, 0, |pixels, _| Some(Frame {
                    width: 512,
                    height: 512,
                    rgba: pixels,
                    hotspot: None
                })));
                observed.push(
                    cache.show_bounded(ui, 0, 1024, "Import preview", |pixels, _| {
                        Some(Frame {
                            width: 512,
                            height: 512,
                            rgba: pixels,
                            hotspot: None,
                        })
                    }),
                );
            });
            output.textures_delta.clear();
            assert!(observed.iter().all(|ready| *ready == expected));
        }
    }
    #[test]
    fn controls_and_consent_are_translated_in_every_language() {
        for language in crate::i18n::Language::ALL
            .into_iter()
            .filter(|language| *language != crate::i18n::Language::English)
        {
            for source in [
                "Export icon or cursor…",
                "Icon and cursor export",
                kind_label(Kind::Icon),
                kind_label(Kind::Cursor),
                "Each entry is fitted independently, preserving aspect ratio on transparent padding.",
                "Icon entries",
                "Add entry",
                "Remove entry",
                "Entry encoding",
                encoding_label(Encoding::Bitmap),
                "Fully transparent entries require PNG encoding.",
                "Hotspot X",
                "Hotspot Y",
                "Resampling",
                filter_label(Filter::Nearest),
                "Entry dimensions will change to fit the selected sizes.",
                "I accept these export changes.",
                "Icon preview is unavailable. Check the entry settings or reduce the canvas size.",
            ] {
                assert_ne!(
                    crate::i18n::translate(language, source),
                    source,
                    "{}: {source}",
                    language.id()
                );
            }
        }
    }
    #[test]
    fn cached_preview_must_be_displayed_before_export_and_stable_keys_are_not_rebuilt() {
        let ctx = egui::Context::default();
        let mut cache: Cache<u8, Vec<u8>> = Cache::default();
        let mut builds = 0;
        for (key, expected) in [(1, false), (1, true), (2, false), (2, true)] {
            let mut ready = Vec::new();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                cache.update(ui.ctx(), key, || {
                    builds += 1;
                    Ok(vec![10, 20, 30, 255])
                });
                ready.push(cache.show(ui, 0, |pixels, _| {
                    Some(Frame {
                        width: 1,
                        height: 1,
                        rgba: pixels,
                        hotspot: None,
                    })
                }));
            });
            output.textures_delta.clear();
            assert!(!ready.is_empty());
            assert!(ready.iter().all(|value| *value == expected));
        }
        assert_eq!(builds, 2);
    }
}
