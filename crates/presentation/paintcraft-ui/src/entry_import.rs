//! Explicit icon/cursor entry selection; encoded bytes belong to the host.
use crate::{page_import::Response, t};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub index: u16,
    pub width: u32,
    pub height: u32,
    pub encoding: &'static str,
    pub depth: u16,
    pub hotspot: Option<(u16, u16)>,
}

pub fn dialog(
    ctx: &egui::Context,
    name: &str,
    entries: &[Entry],
    selected: &mut Option<u16>,
    pixel_limit: u64,
    error: Option<&str>,
) -> Response {
    let response = egui::Modal::new(egui::Id::unique("icon-entry-import")).show(ctx, |ui| {
        ui.set_width(460.0_f32.min((ctx.content_rect().width() - 48.0).max(160.0)));
        ui.heading(t!(ctx, "Choose an icon or cursor entry"));
        ui.label(name);
        ui.label(t!(ctx, "Import one entry as a picture. Other entries remain in the source file."));
        egui::ScrollArea::vertical().id_salt("icon-entry-list").max_height(240.0).show(ui, |ui| {
            for entry in entries {
                ui.radio_value(selected, Some(entry.index), t!(ctx, "Entry { $number }: { $width } × { $height } px, { $encoding }, { $depth } bits",
                    number = u32::from(entry.index) + 1, width = entry.width, height = entry.height, encoding = entry.encoding, depth = u32::from(entry.depth)));
                if let Some((x, y)) = entry.hotspot {
                    ui.label(t!(ctx, "Hotspot: { $x }, { $y }", x = u32::from(x), y = u32::from(y)));
                }
                ui.add_space(4.0);
            }
        });
        let current = selected.and_then(|index| entries.iter().find(|entry| entry.index == index));
        let eligible = current.is_some_and(|entry| u64::from(entry.width) * u64::from(entry.height) <= pixel_limit);
        if current.is_some() && !eligible { ui.label(t!(ctx, "This entry exceeds the canvas size limit.")); }
        if let Some(error) = error { ui.colored_label(ui.visuals().error_fg_color, error); }
        ui.separator();
        ui.horizontal(|ui| (
            ui.button(t!(ctx, "Cancel")).clicked(),
            ui.add_enabled(eligible, egui::Button::new(t!(ctx, "Import selected entry"))).clicked(),
        )).inner
    });
    if ctx.current_pass_index() != 0 {
        return Response::Pending;
    }
    if response.inner.0 || response.should_close() {
        Response::Cancel
    } else if response.inner.1 {
        selected.map_or(Response::Pending, Response::Import)
    } else {
        Response::Pending
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn entry_controls_and_metadata_are_translated_in_every_language() {
        use crate::i18n::{Language, format_message, translate};
        for language in Language::ALL
            .into_iter()
            .filter(|language| *language != Language::English)
        {
            for source in [
                "Choose an icon or cursor entry",
                "Import one entry as a picture. Other entries remain in the source file.",
                "This entry exceeds the canvas size limit.",
                "Import selected entry",
            ] {
                assert_ne!(
                    translate(language, source),
                    source,
                    "{}: {source}",
                    language.id()
                );
            }
            for (source, args) in [
                (
                    "Entry { $number }: { $width } × { $height } px, { $encoding }, { $depth } bits",
                    vec![
                        ("number", "2".into()),
                        ("width", "256".into()),
                        ("height", "256".into()),
                        ("encoding", "PNG".into()),
                        ("depth", "32".into()),
                    ],
                ),
                (
                    "Hotspot: { $x }, { $y }",
                    vec![("x", "3".into()), ("y", "7".into())],
                ),
            ] {
                let translated = format_message(language, source, &args);
                assert!(!translated.contains('{'), "{translated}");
                assert_ne!(
                    translated,
                    format_message(Language::English, source, &args),
                    "{}",
                    language.id()
                );
            }
        }
    }
}
