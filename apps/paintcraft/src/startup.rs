//! Startup failures must remain visible in packaged applications without a console.
use paintcraft_ui::i18n::{Language, translate};

fn error_message(language: Language, detail: &str) -> (String, String) {
    let title = translate(language, "PaintCraft could not start");
    let help = translate(
        language,
        "PaintCraft could not open its window. Check your display and graphics driver, then try again.",
    );
    (title, format!("{help}\n\n{detail}"))
}

pub fn report_error(language: Language, detail: &str) {
    let (title, description) = error_message(language, detail);
    eprintln!("{title}: {detail}");
    paintcraft_platform::dialogs::show_error(&title, &description, &translate(language, "OK"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_failures_are_localized_and_preserve_the_original_diagnostics() {
        let detail = "Adapter unavailable: driver.dll\n日本語/example.pcnd";
        let english = error_message(Language::English, detail);
        for language in Language::ALL {
            let (title, description) = error_message(language, detail);
            assert!(description.ends_with(&format!("\n\n{detail}")));
            assert!(!title.is_empty());
            if language != Language::English {
                assert_ne!(title, english.0, "{language:?}");
                assert_ne!(description, english.1, "{language:?}");
            }
        }
    }
}
