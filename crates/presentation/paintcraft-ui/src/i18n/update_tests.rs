//! Actual public Fluent lookup regression coverage for the signed-update UI.
use super::*;

const LABELS: [&str; 30] = [
    "Updates",
    "Check for updates",
    "Checking for updates…",
    "Download update",
    "Downloading update…",
    "Cancelling update…",
    "Your version is up to date.",
    "Restart to update",
    "Update cancelled.",
    "Updates are unavailable in this build.",
    "This installation is updated through its package manager or by installing a new copy.",
    "Release notes",
    "Update task ended unexpectedly.",
    "PaintCraft is updating",
    "Another PaintCraft editor is using this installation. Close other editors and try again.",
    "Save all documents and restart?",
    "PaintCraft will save modified documents before installing the verified update.",
    "Save all and restart",
    "Finish active dialogs, printing, clipboard, paste, export, and font loading before restarting.",
    "The parked workspace must be clean and idle before restarting.",
    "Saving documents before update…",
    "Update restart cancelled; your documents remain open.",
    "Saving was cancelled, failed, or newer edits remain unsaved. Update not installed.",
    "Stopping background work before update…",
    "Installing verified update…",
    "Update installation finished. Restarting PaintCraft…",
    "PaintCraft could not finish installing the update. Close PaintCraft before trying again.",
    "Background shutdown failed. The update was not installed.",
    "Close PaintCraft",
    "Restarting to update…",
];

fn catalog_value(lang: Language, id: &str, args: Option<&FluentArgs<'_>>) -> String {
    with_bundle(lang, |bundle| {
        let pattern = bundle
            .get_message(id)
            .and_then(|message| message.value())
            .unwrap_or_else(|| panic!("{} missing catalog message {id}", lang.id()));
        let mut errors = Vec::new();
        let result = bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned();
        assert!(errors.is_empty(), "{} {id}: {errors:?}", lang.id());
        assert!(!result.is_empty(), "{} {id}", lang.id());
        result
    })
}

#[test]
fn all_update_save_and_install_guard_labels_resolve_real_catalog_messages() {
    for lang in Language::ALL {
        let ctx = egui::Context::default();
        set_language(&ctx, lang);
        for source in LABELS {
            let expected = catalog_value(lang, &key(source), None);
            let rendered = translate(lang, source);
            assert_eq!(rendered, expected, "{} {source}", lang.id());
            assert_eq!(tr(&ctx, source), expected, "{} context {source}", lang.id());
            // German intentionally uses the common loanword Updates. Its actual
            // resource presence was still required above rather than fallback.
            if lang != Language::English && !(lang == Language::German && source == "Updates") {
                assert_ne!(
                    rendered,
                    source,
                    "{} fell back to English: {source}",
                    lang.id()
                );
            }
        }
    }
}

#[test]
fn four_update_templates_resolve_formatted_ids_and_preserve_unicode_values() {
    let version = "0.2.0-日本語+β.हिन्दी";
    let detail = "署名 é Ω /tmp/画像.pcnd { $version }\nदस्तावेज़";
    for lang in Language::ALL {
        let ctx = egui::Context::default();
        set_language(&ctx, lang);
        for (source, id, name, value) in [
            (
                "Update available: { $version }",
                "format-update-available-version",
                "version",
                version,
            ),
            (
                "Verified update ready: { $version }",
                "format-verified-update-ready-version",
                "version",
                version,
            ),
            (
                "Update failed: { $detail }",
                "format-update-failed-detail",
                "detail",
                detail,
            ),
            (
                "PaintCraft could not obtain its installation lock: { $detail }",
                "format-paintcraft-could-not-obtain-its-installation-lock-detail",
                "detail",
                detail,
            ),
        ] {
            let mut args = FluentArgs::new();
            args.set(name, value);
            let expected = catalog_value(lang, id, Some(&args));
            let values = [(name, value.to_owned())];
            let rendered = format_message(lang, source, &values);
            assert_eq!(rendered, expected, "{} {id}", lang.id());
            assert_eq!(
                tr_format(&ctx, source, &values),
                expected,
                "{} context {id}",
                lang.id()
            );
            assert!(rendered.contains(value), "{} {id}: {rendered:?}", lang.id());
            assert_eq!(rendered.matches(value).count(), 1, "{} {id}", lang.id());
            if lang != Language::English {
                assert_ne!(
                    rendered,
                    format_message(Language::English, source, &values),
                    "{} English fallback {id}",
                    lang.id()
                );
            }
        }
    }
}

#[test]
fn updater_labels_and_unicode_arguments_remain_local_to_each_context() {
    let english = egui::Context::default();
    let japanese = egui::Context::default();
    set_language(&japanese, Language::Japanese);
    assert_eq!(tr(&english, "Restart to update"), "Restart to update");
    assert_eq!(tr(&japanese, "Restart to update"), "再起動して更新");
    let value = "v0.2-日本語-हिन्दी";
    let values = [("version", value.to_owned())];
    let source = "Verified update ready: { $version }";
    assert_eq!(
        tr_format(&english, source, &values),
        format!("Verified update ready: {value}")
    );
    assert!(tr_format(&japanese, source, &values).contains(value));
    assert_ne!(
        tr_format(&japanese, source, &values),
        tr_format(&english, source, &values)
    );
    assert_eq!(language(&english), Language::English);
}
