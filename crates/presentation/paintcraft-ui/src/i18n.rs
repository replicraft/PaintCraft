//! Fluent-backed UI translations. Locale belongs to an egui context, not a process.
use fluent_bundle::{FluentBundle, FluentResource};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    English,
    Japanese,
}
impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Japanese];
    pub fn id(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::Japanese => "ja",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Japanese => "日本語",
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.id() == id)
    }
}
fn context_key() -> egui::Id {
    egui::Id::unique("paintcraft-language")
}
pub fn language(ctx: &egui::Context) -> Language {
    ctx.data(|data| data.get_temp(context_key()).unwrap_or_default())
}
pub fn set_language(ctx: &egui::Context, lang: Language) {
    if language(ctx) != lang {
        ctx.data_mut(|data| data.insert_temp(context_key(), lang));
        crate::fonts::install(ctx);
        ctx.request_repaint();
    }
}
fn bundle(lang: Language) -> FluentBundle<FluentResource> {
    let mut bundle = FluentBundle::new(vec![
        lang.id()
            .parse::<unic_langid::LanguageIdentifier>()
            .expect("valid bundled locale"),
    ]);
    let source = match lang {
        Language::English => include_str!("../locales/en-US.ftl"),
        Language::Japanese => include_str!("../locales/ja.ftl"),
    };
    bundle
        .add_resource(FluentResource::try_new(source.into()).expect("valid bundled translations"))
        .expect("unique bundled message IDs");
    bundle
}
thread_local! { static JAPANESE: FluentBundle<FluentResource> = bundle(Language::Japanese); }
fn key(source: &str) -> String {
    let mut result = String::new();
    for c in source.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else if !result.is_empty() && !result.ends_with('-') {
            result.push('-');
        }
    }
    let mut result = result.trim_end_matches('-').to_owned();
    if source.ends_with('…') {
        result.push_str("-more");
    }
    result
}
/// English source text is the fallback and keeps existing English accessibility labels.
pub fn tr(ctx: &egui::Context, source: impl AsRef<str>) -> String {
    translate(language(ctx), source.as_ref())
}
pub fn translate(lang: Language, source: &str) -> String {
    if lang == Language::English {
        return source.into();
    }
    JAPANESE.with(|bundle| {
        let Some(value) = bundle.get_message(&key(source)).and_then(|msg| msg.value()) else {
            return source.into();
        };
        let mut errors = Vec::new();
        let translated = bundle.format_pattern(value, None, &mut errors);
        if errors.is_empty() {
            translated.into_owned()
        } else {
            source.into()
        }
    })
}
pub fn picker(ui: &mut egui::Ui, lang: &mut Language) {
    egui::ComboBox::from_label(tr(ui.ctx(), "Language"))
        .selected_text(lang.name())
        .show_ui(ui, |ui| {
            for choice in Language::ALL {
                ui.selectable_value(lang, choice, choice.name());
            }
        });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_english_message_has_a_valid_japanese_translation() {
        let en = bundle(Language::English);
        let ja = bundle(Language::Japanese);
        for line in include_str!("../locales/en-US.ftl").lines() {
            let (id, source) = line.split_once(" = ").unwrap();
            assert!(en.get_message(id).is_some());
            assert!(ja.get_message(id).is_some(), "{id}");
            if source != "OK" {
                assert_ne!(translate(Language::Japanese, source), source, "{id}");
            }
        }
        assert_eq!(
            translate(Language::Japanese, "unknown user text"),
            "unknown user text"
        );
    }
    #[test]
    fn language_is_isolated_per_context_and_unknown_locales_are_rejected() {
        let a = egui::Context::default();
        let b = egui::Context::default();
        set_language(&a, Language::Japanese);
        assert_eq!(tr(&a, "Settings"), "設定");
        assert_eq!(tr(&b, "Settings"), "Settings");
        assert!(Language::parse("xx").is_none());
    }
}
