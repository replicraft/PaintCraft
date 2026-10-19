//! User settings: the saved Classic variant, kept in a small versioned JSON file.
//!
//! A damaged or unfamiliar settings file never costs the user anything: it is
//! moved aside (not deleted), defaults are used, and the reason is reported.

use std::fs;
use std::path::{Path, PathBuf};

use paintcraft_core::types::json::{self, Value};
use paintcraft_platform::save;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassicVariant {
    V7,
    V10,
    V11,
}

impl ClassicVariant {
    pub const ALL: [ClassicVariant; 3] =
        [ClassicVariant::V7, ClassicVariant::V10, ClassicVariant::V11];
    /// Selected when the user has not chosen.
    pub const DEFAULT: ClassicVariant = ClassicVariant::V11;

    pub fn id(self) -> &'static str {
        match self {
            ClassicVariant::V7 => "v7",
            ClassicVariant::V10 => "v10",
            ClassicVariant::V11 => "v11",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ClassicVariant::V7 => "Paint v7",
            ClassicVariant::V10 => "Paint v10",
            ClassicVariant::V11 => "Paint v11",
        }
    }

    fn from_id(s: &str) -> Option<ClassicVariant> {
        ClassicVariant::ALL.into_iter().find(|v| v.id() == s)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// `None` until the first-visit picker has been answered.
    pub classic_variant: Option<ClassicVariant>,
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
    if v.get("schema").and_then(Value::as_str) != Some("paintcraft.settings/v1") {
        return Err("unrecognized settings version".into());
    }
    let classic_variant = match v.get("classic_variant") {
        Some(Value::Null) | None => None,
        Some(Value::Str(s)) => Some(ClassicVariant::from_id(s).ok_or("unknown Classic variant")?),
        Some(_) => return Err("classic_variant has the wrong type".into()),
    };
    Ok(Settings { classic_variant })
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
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create the settings folder: {}", e.kind()))?;
        let v = Value::object([
            ("schema", Value::str("paintcraft.settings/v1")),
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
    fn the_choice_round_trips_for_every_variant() {
        let dir = scratch("roundtrip");
        for v in ClassicVariant::ALL {
            Settings {
                classic_variant: Some(v),
            }
            .save(&dir)
            .unwrap();
            assert_eq!(
                Settings::load(&dir),
                (
                    Settings {
                        classic_variant: Some(v)
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
            }
            .save(&dir)
            .unwrap();
            assert_eq!(Settings::load(&dir).1, Loaded::Ok);
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
