//! Fluent-backed UI translations. Locale belongs to an egui context, not a process.
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use std::cell::RefCell;
use std::collections::HashMap;

pub(crate) mod catalogs;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    English,
    Japanese,
    German,
    Spanish,
    PortugueseBrazil,
    Hindi,
    ChineseSimplified,
}
impl Language {
    pub const ALL: [Self; 7] = [
        Self::English,
        Self::German,
        Self::Spanish,
        Self::Hindi,
        Self::Japanese,
        Self::PortugueseBrazil,
        Self::ChineseSimplified,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::Japanese => "ja",
            Self::German => "de",
            Self::Spanish => "es",
            Self::PortugueseBrazil => "pt-BR",
            Self::Hindi => "hi",
            Self::ChineseSimplified => "zh-CN",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Japanese => "日本語",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::PortugueseBrazil => "Português (Brasil)",
            Self::Hindi => "हिन्दी",
            Self::ChineseSimplified => "简体中文",
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        // Retain en-US for existing preferences, while the catalog directory is en.
        let id = if id == "en" { "en-US" } else { id };
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
        crate::fonts::install_for_locale(ctx, lang.id());
        ctx.request_repaint();
    }
}
fn bundle(lang: Language) -> FluentBundle<FluentResource> {
    let mut bundle = FluentBundle::new(vec![
        lang.id()
            .parse::<unic_langid::LanguageIdentifier>()
            .expect("valid bundled locale"),
    ]);
    // All bundled languages are left-to-right; preserve literal accessible labels.
    bundle.set_use_isolating(false);
    for source in catalogs::sources(lang) {
        bundle
            .add_resource(
                FluentResource::try_new((*source).into()).expect("valid bundled translations"),
            )
            .expect("unique bundled message IDs across interfaces");
    }
    bundle
}
thread_local! {
    static BUNDLES: RefCell<HashMap<Language, FluentBundle<FluentResource>>> = RefCell::default();
}
fn with_bundle<T>(lang: Language, render: impl FnOnce(&FluentBundle<FluentResource>) -> T) -> T {
    BUNDLES.with(|cache| {
        let mut cache = cache.borrow_mut();
        render(cache.entry(lang).or_insert_with(|| bundle(lang)))
    })
}
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
    with_bundle(lang, |bundle| {
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
/// Format a translated template with named Fluent arguments. User values are never translated.
/// Call before entering context-locked accessibility callbacks.
pub fn tr_format(ctx: &egui::Context, source: &str, values: &[(&str, String)]) -> String {
    format_message(language(ctx), source, values)
}
pub fn format_message(lang: Language, source: &str, values: &[(&str, String)]) -> String {
    let mut args = FluentArgs::new();
    for (name, value) in values {
        args.set(*name, value.as_str());
    }
    let render = |bundle: &FluentBundle<FluentResource>| {
        let value = bundle
            .get_message(&format!("format-{}", key(source)))?
            .value()?;
        let mut errors = Vec::new();
        let result = bundle
            .format_pattern(value, Some(&args), &mut errors)
            .into_owned();
        errors.is_empty().then_some(result)
    };
    let translated = with_bundle(lang, render);
    translated.unwrap_or_else(|| {
        // Keep the English fallback useful even for templates absent from the catalog.
        let mut result = source.to_owned();
        for (name, value) in values {
            result = result.replace(&format!("{{ ${name} }}"), value);
        }
        result
    })
}
/// Localize controller status envelopes at the presentation boundary. Diagnostic details
/// remain intact; document paths and error payloads are never treated as message keys.
pub fn tr_status(ctx: &egui::Context, source: &str) -> String {
    if language(ctx) == Language::English {
        return source.into();
    }
    let translated = tr(ctx, source);
    if translated != source {
        return translated;
    }
    if let Some(size) = source
        .strip_prefix("New ")
        .and_then(|s| s.strip_suffix(" document"))
        && let Some((width, height)) = size.split_once('×')
        && width.parse::<u32>().is_ok()
        && height.parse::<u32>().is_ok()
    {
        return tr_format(
            ctx,
            "New { $width }×{ $height } document",
            &[("width", width.into()), ("height", height.into())],
        );
    }
    for (prefix, template) in [
        (
            "Crash recovery unavailable: ",
            "Crash recovery unavailable: { $detail }",
        ),
        ("Cannot recover: ", "Cannot recover: { $detail }"),
        (
            "Cannot discard recovery: ",
            "Cannot discard recovery: { $detail }",
        ),
        ("Using CPU rendering: ", "Using CPU rendering: { $detail }"),
        ("Using CPU display: ", "Using CPU display: { $detail }"),
        (
            "Scaled display used CPU: ",
            "Scaled display used CPU: { $detail }",
        ),
        ("Cannot render: ", "Cannot render: { $detail }"),
        (
            "Cannot render viewport: ",
            "Cannot render viewport: { $detail }",
        ),
        (
            "Background saving unavailable: ",
            "Background saving unavailable: { $detail }",
        ),
        (
            "Background renderer unavailable: ",
            "Background renderer unavailable: { $detail }",
        ),
        (
            "Background editing unavailable: ",
            "Background editing unavailable: { $detail }",
        ),
        (
            "Installed fonts unavailable: ",
            "Installed fonts unavailable: { $detail }",
        ),
        (
            "Could not save preferences: ",
            "Could not save preferences: { $detail }",
        ),
        (
            "Cannot create preferred canvas: ",
            "Cannot create preferred canvas: { $detail }",
        ),
        ("Edit failed: ", "Edit failed: { $detail }"),
        ("Open failed: ", "Open failed: { $detail }"),
        ("Cannot open: ", "Cannot open: { $detail }"),
        ("Cannot export: ", "Cannot export: { $detail }"),
        (
            "File operation failed: ",
            "File operation failed: { $detail }",
        ),
        ("Layer change failed: ", "Layer change failed: { $detail }"),
        ("Preview unavailable: ", "Preview unavailable: { $detail }"),
        ("Stroke not prepared: ", "Stroke not prepared: { $detail }"),
        (
            "Cannot replace document: ",
            "Cannot replace document: { $detail }",
        ),
        ("Save not queued: ", "Save not queued: { $detail }"),
        ("Autosave not queued: ", "Autosave not queued: { $detail }"),
        (
            "Native menu unavailable: ",
            "Native menu unavailable: { $detail }",
        ),
        (
            "Native menu update failed: ",
            "Native menu update failed: { $detail }",
        ),
        ("Save All stopped: ", "Save All stopped: { $detail }"),
        ("Opened ", "Opened { $detail }"),
        ("Imported ", "Imported { $detail }"),
        ("Exported ", "Exported { $detail }"),
        ("Saved ", "Saved { $detail }"),
    ] {
        if let Some(detail) = source.strip_prefix(prefix) {
            return tr_format(ctx, template, &[("detail", detail.into())]);
        }
    }
    source.into()
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
    fn every_interface_catalog_is_valid_and_complete_in_every_language() {
        for language in Language::ALL {
            let translated = bundle(language);
            for source in catalogs::sources(Language::English) {
                for line in source
                    .lines()
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                {
                    let (id, english) = line.split_once(" = ").unwrap();
                    let message = translated
                        .get_message(id)
                        .unwrap_or_else(|| panic!("{}: missing {id}", language.id()));
                    let mut args = FluentArgs::new();
                    for tail in english.split('$').skip(1) {
                        let name = tail
                            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                            .next()
                            .unwrap();
                        args.set(name, "sample 日本語 हिन्दी.png");
                    }
                    let mut errors = Vec::new();
                    let value = translated.format_pattern(
                        message.value().unwrap(),
                        Some(&args),
                        &mut errors,
                    );
                    assert!(errors.is_empty(), "{}: {id}: {errors:?}", language.id());
                    assert!(!value.is_empty(), "{}: {id}", language.id());
                }
            }
            assert_eq!(
                translate(language, "unknown user text"),
                "unknown user text"
            );
            assert_eq!(Language::parse(language.id()), Some(language));
        }
        assert_eq!(Language::parse("en"), Some(Language::English));
    }
    #[test]
    fn macro_formats_named_arguments_and_preserves_user_values() {
        let ctx = egui::Context::default();
        for language in Language::ALL {
            set_language(&ctx, language);
            assert_eq!(crate::t!(&ctx, "Settings"), translate(language, "Settings"));
            let filename = "my 日本語 हिन्दी.png";
            let text = crate::t!(&ctx, "Opened { $detail }", detail = filename);
            assert!(text.contains(filename), "{}: {text}", language.id());
            assert!(!text.contains("{ $detail }"));
        }
    }
    #[test]
    fn formatted_messages_translate_templates_and_preserve_user_values() {
        let value = [("count", "315".to_owned())];
        assert_eq!(
            format_message(
                Language::English,
                "{ $count } available font families",
                &value
            )
            .replace(['\u{2068}', '\u{2069}'], ""),
            "315 available font families"
        );
        assert_eq!(
            format_message(
                Language::Japanese,
                "{ $count } available font families",
                &value
            )
            .replace(['\u{2068}', '\u{2069}'], ""),
            "利用可能なフォント: 315種類"
        );
        let ctx = egui::Context::default();
        set_language(&ctx, Language::Japanese);
        let size = tr_format(&ctx, "{ $tool } size", &[("tool", tr(&ctx, "Pencil"))]);
        assert_eq!(size.replace(['\u{2068}', '\u{2069}'], ""), "鉛筆のサイズ");
        assert_eq!(tr(&ctx, "Tool size"), "ツールのサイズ");
        let filename = "Pencil Size.png";
        assert!(tr_status(&ctx, &format!("Opened {filename}")).contains(filename));
        assert!(tr_status(&ctx, "New 1000×486 document").contains("新しい"));
        assert_eq!(
            tr_status(&ctx, "Crash recovery ready"),
            "クラッシュからの復元の準備ができました"
        );
        assert_eq!(
            tr_status(&ctx, "Crash recovery unavailable: disk full")
                .replace(['\u{2068}', '\u{2069}'], ""),
            "クラッシュからの復元を利用できません: disk full"
        );
        assert_eq!(
            format_message(
                Language::Japanese,
                "Uncatalogued { $name }",
                &[("name", filename.into())]
            ),
            "Uncatalogued Pencil Size.png"
        );
    }
    #[test]
    fn reported_classic_labels_are_translated() {
        for source in [
            "Automatically hide toolbar",
            "Zoom to window",
            "Rotate custom",
            "Selection options",
            "Invert selection",
            "Stroke",
        ] {
            assert_ne!(translate(Language::Japanese, source), source, "{source}");
        }
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
