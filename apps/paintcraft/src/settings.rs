//! User settings: active experience and Classic variant in a versioned JSON file.
//!
//! A damaged or unfamiliar settings file never costs the user anything: it is
//! moved aside (not deleted), defaults are used, and the reason is reported.

mod colors;
mod text;
pub use paintcraft_classic::canvas_size::{CanvasDefaults, DEFAULT_SIZE, SIZE_MAX};

use paintcraft_ui::i18n::Language;
use std::fs;
use std::path::{Path, PathBuf};

use paintcraft_core::types::json::{self, Value};
use paintcraft_platform::save;

pub use paintcraft_classic::ClassicVariant;
pub use paintcraft_ui::theme::Appearance;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Experience {
    #[default]
    Classic,
    Studio,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub experience: Experience,
    pub appearance: Appearance,
    pub language: Language,
    pub text: paintcraft_classic::TextOptions,
    /// `None` until the first-visit picker has been answered.
    pub classic_variant: Option<ClassicVariant>,
    pub canvas: CanvasDefaults,
    pub custom_colors: paintcraft_classic::CustomColors,
    pub classic_status_bar: bool,
    pub classic_rulers: bool,
    pub classic_toolbar_auto_hide: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            experience: Experience::default(),
            appearance: Appearance::default(),
            language: Default::default(),
            text: Default::default(),
            classic_variant: None,
            canvas: Default::default(),
            custom_colors: Default::default(),
            classic_status_bar: true,
            classic_rulers: false,
            classic_toolbar_auto_hide: false,
        }
    }
}

/// What loading found, so the application can explain a fallback.
#[derive(Debug, PartialEq, Eq)]
pub enum Loaded {
    Missing,
    Ok,
    /// The file could not be used; it was kept at `moved_to`.
    Unusable {
        reason: String,
        moved_to: Option<PathBuf>,
    },
}

/// `PAINTCRAFT_CONFIG_DIR` overrides the platform default.
pub fn default_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("PAINTCRAFT_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        home.unwrap_or_default()
            .join("Library/Application Support/PaintCraft")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("PaintCraft")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".config")))
            .unwrap_or_default()
            .join("paintcraft")
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

fn parse(bytes: &[u8]) -> Result<Settings, String> {
    let v = json::parse(bytes).map_err(|e| e.to_string())?;
    let version = v.get("schema").and_then(Value::as_str);
    if !matches!(
        version,
        Some("paintcraft.settings/v1" | "paintcraft.settings/v2" | "paintcraft.settings/v3")
    ) {
        return Err("unrecognized settings version".into());
    }
    let classic_variant = match v.get("classic_variant") {
        Some(Value::Null) | None => None,
        Some(Value::Str(s)) => Some(ClassicVariant::from_id(s).ok_or("unknown Classic variant")?),
        Some(_) => return Err("classic_variant has the wrong type".into()),
    };
    let experience = match (version, v.get("experience").and_then(Value::as_str)) {
        (Some("paintcraft.settings/v1"), _) => Experience::Classic,
        (_, Some("classic")) => Experience::Classic,
        (_, Some("studio")) => Experience::Studio,
        _ => return Err("unknown interface experience".into()),
    };
    let canvas = if version == Some("paintcraft.settings/v3") {
        CanvasDefaults::parse(v.get("canvas").ok_or("missing canvas preferences")?)?
    } else {
        CanvasDefaults::default()
    };
    let appearance = match v.get("appearance") {
        None => Appearance::System,
        Some(Value::Str(id)) => Appearance::parse(id).ok_or("unknown appearance")?,
        Some(_) => return Err("invalid appearance".into()),
    };
    let language = match v.get("language") {
        None => Default::default(),
        Some(value) => value
            .as_str()
            .and_then(Language::parse)
            .ok_or("unknown language")?,
    };
    Ok(Settings {
        language,
        appearance,
        classic_toolbar_auto_hide: match v.get("classic_toolbar_auto_hide") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("invalid Classic toolbar preference".into()),
        },
        classic_rulers: match v.get("classic_rulers") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("invalid Classic rulers preference".into()),
        },
        classic_status_bar: match v.get("classic_status_bar") {
            None => true,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("invalid Classic status bar preference".into()),
        },
        text: text::parse(v.get("text"))?,
        custom_colors: colors::parse(v.get("custom_colors"))?,
        canvas,
        classic_variant,
        experience,
    })
}

impl Settings {
    pub fn load(dir: &Path) -> (Settings, Loaded) {
        let path = file(dir);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (Settings::default(), Loaded::Missing);
            }
            Err(e) => {
                return (
                    Settings::default(),
                    Loaded::Unusable {
                        reason: format!("cannot read settings: {}", e.kind()),
                        moved_to: None,
                    },
                );
            }
        };
        match parse(&bytes) {
            Ok(s) => (s, Loaded::Ok),
            Err(reason) => {
                let aside = (0..100)
                    .map(|n| dir.join(format!("settings.json.unusable-{n}")))
                    .find(|p| !p.exists());
                let moved_to = aside.filter(|a| fs::rename(&path, a).is_ok());
                (Settings::default(), Loaded::Unusable { reason, moved_to })
            }
        }
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        CanvasDefaults::parse(&self.canvas.value())?;
        text::parse(Some(&text::value(&self.text)))?;
        colors::parse(Some(&colors::value(&self.custom_colors)))?;
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create the settings folder: {}", e.kind()))?;
        let v = Value::object([
            ("schema", Value::str("paintcraft.settings/v3")),
            ("canvas", self.canvas.value()),
            ("custom_colors", colors::value(&self.custom_colors)),
            ("appearance", Value::str(self.appearance.id())),
            ("language", Value::str(self.language.id())),
            ("classic_status_bar", Value::Bool(self.classic_status_bar)),
            ("classic_rulers", Value::Bool(self.classic_rulers)),
            (
                "classic_toolbar_auto_hide",
                Value::Bool(self.classic_toolbar_auto_hide),
            ),
            ("text", text::value(&self.text)),
            (
                "experience",
                Value::str(match self.experience {
                    Experience::Classic => "classic",
                    Experience::Studio => "studio",
                }),
            ),
            (
                "classic_variant",
                self.classic_variant
                    .map_or(Value::Null, |c| Value::str(c.id())),
            ),
        ]);
        let outcome = save::save(&file(dir), &json::canonical(&v), true);
        if outcome.is_success() {
            Ok(())
        } else {
            Err(outcome.reasons.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "paintcraft-settings-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn nothing_saved_means_the_picker_is_still_owed_with_v11_preselected() {
        let dir = scratch("missing");
        let (s, l) = Settings::load(&dir);
        assert_eq!((s.classic_variant, l), (None, Loaded::Missing));
        assert_eq!(ClassicVariant::DEFAULT, ClassicVariant::V11);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn v1_settings_migrate_without_losing_the_classic_choice() {
        let s = parse(br#"{"schema":"paintcraft.settings/v1","classic_variant":"v10"}"#).unwrap();
        assert_eq!(s.canvas.new_size(), (1000, 486));
        assert_eq!(s.experience, Experience::Classic);
        assert_eq!(s.classic_variant, Some(ClassicVariant::V10));
    }

    #[test]
    fn studio_roundtrips_before_classic_has_ever_been_entered() {
        let dir = scratch("studio");
        let s = Settings {
            experience: Experience::Studio,
            classic_variant: None,
            ..Default::default()
        };
        s.save(&dir).unwrap();
        assert_eq!(Settings::load(&dir), (s, Loaded::Ok));
        fs::remove_dir_all(dir).unwrap();
        assert!(parse(br#"{"schema":"paintcraft.settings/v2","experience":"unknown"}"#).is_err());
    }

    #[test]
    fn the_choice_round_trips_for_every_variant() {
        let dir = scratch("roundtrip");
        for v in ClassicVariant::ALL {
            Settings {
                classic_variant: Some(v),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            assert_eq!(
                Settings::load(&dir),
                (
                    Settings {
                        classic_variant: Some(v),
                        ..Default::default()
                    },
                    Loaded::Ok
                )
            );
        }
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "no staging files");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn damaged_or_unfamiliar_files_are_moved_aside_not_destroyed() {
        for (name, content) in [
            ("garbage", &b"\xff\xfe not json"[..]),
            ("future", br#"{"schema":"paintcraft.settings/v9"}"#),
            (
                "variant",
                br#"{"schema":"paintcraft.settings/v1","classic_variant":"v95"}"#,
            ),
            (
                "type",
                br#"{"schema":"paintcraft.settings/v1","classic_variant":7}"#,
            ),
        ] {
            let dir = scratch(name);
            fs::write(file(&dir), content).unwrap();
            let (s, l) = Settings::load(&dir);
            assert_eq!(s, Settings::default(), "{name}");
            let Loaded::Unusable {
                moved_to: Some(kept),
                ..
            } = l
            else {
                panic!("{name}: {l:?}")
            };
            assert_eq!(
                fs::read(kept).unwrap(),
                content,
                "{name}: the original bytes are preserved"
            );
            assert!(!file(&dir).exists());
            // A fresh save then works and does not disturb the preserved copy.
            Settings {
                classic_variant: Some(ClassicVariant::V7),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            assert_eq!(Settings::load(&dir).1, Loaded::Ok);
            fs::remove_dir_all(dir).unwrap();
        }
    }
    #[test]
    fn status_bar_defaults_on_for_old_settings_and_rejects_wrong_types() {
        assert!(
            parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic"}"#)
                .unwrap()
                .classic_status_bar
        );
        assert!(!parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic","classic_status_bar":false}"#).unwrap().classic_status_bar);
        assert!(parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic","classic_status_bar":"false"}"#).is_err());
    }
    #[test]
    fn ruler_preference_is_optional_boolean_in_existing_settings() {
        assert!(
            !parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic"}"#)
                .unwrap()
                .classic_rulers
        );
        assert!(parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic","classic_rulers":true}"#).unwrap().classic_rulers);
        assert!(parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic","classic_rulers":"true"}"#).is_err());
    }
    #[test]
    fn toolbar_visibility_defaults_validate_and_round_trip() {
        assert!(
            !parse(br#"{"schema":"paintcraft.settings/v1"}"#)
                .unwrap()
                .classic_toolbar_auto_hide
        );
        assert!(parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic","classic_toolbar_auto_hide":true}"#).unwrap().classic_toolbar_auto_hide);
        assert!(parse(br#"{"schema":"paintcraft.settings/v2","experience":"classic","classic_toolbar_auto_hide":"true"}"#).is_err());
        let dir = scratch("toolbar-visibility");
        let settings = Settings {
            classic_toolbar_auto_hide: true,
            ..Default::default()
        };
        settings.save(&dir).unwrap();
        assert_eq!(Settings::load(&dir), (settings, Loaded::Ok));
        fs::remove_dir_all(dir).unwrap();
    }
}
