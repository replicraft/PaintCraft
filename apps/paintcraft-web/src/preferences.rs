//! Browser preferences contain presentation/startup choices, never document pixels.
use eframe::egui::Theme;
use paintcraft_classic::{ClassicVariant, CustomColors, canvas_size::CanvasDefaults};
use paintcraft_core::types::{
    Rgba,
    json::{self, Value},
};

#[cfg(target_arch = "wasm32")]
const KEY: &str = "paintcraft.web.preferences.v1";
const BYTES_MAX: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Preferences {
    pub canvas: CanvasDefaults,
    pub variant: ClassicVariant,
    pub custom_colors: CustomColors,
    pub theme: Option<Theme>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            canvas: CanvasDefaults::default(),
            variant: ClassicVariant::DEFAULT,
            custom_colors: [None; 28],
            theme: None,
        }
    }
}

impl Preferences {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > BYTES_MAX {
            return Err("preferences exceed their size limit".into());
        }
        let value = json::parse(bytes).map_err(|error| error.to_string())?;
        if value.get("schema").and_then(Value::as_str) != Some("paintcraft.web.preferences/v1") {
            return Err("unknown preferences schema".into());
        }
        let canvas =
            CanvasDefaults::parse(value.get("canvas").ok_or("missing canvas preferences")?)?;
        for size in std::iter::once(canvas.configured).chain(canvas.last) {
            if u64::from(size.0) * u64::from(size.1) > crate::editor::MAX_PIXELS {
                return Err("canvas preferences exceed the browser pixel limit".into());
            }
        }
        let variant = value
            .get("skin")
            .and_then(Value::as_str)
            .and_then(ClassicVariant::from_id)
            .ok_or("unknown Classic skin")?;
        let theme = match value.get("theme") {
            Some(Value::Null) => None,
            Some(theme) if theme.as_str() == Some("dark") => Some(Theme::Dark),
            Some(theme) if theme.as_str() == Some("light") => Some(Theme::Light),
            _ => return Err("unknown theme".into()),
        };
        let colors = value
            .get("custom_colors")
            .and_then(Value::as_array)
            .filter(|colors| colors.len() == 28)
            .ok_or("invalid custom palette")?;
        let mut custom_colors = [None; 28];
        for (dest, src) in custom_colors.iter_mut().zip(colors) {
            *dest = match src {
                Value::Null => None,
                _ => Some(
                    src.as_str()
                        .and_then(Rgba::parse)
                        .ok_or("invalid custom color")?,
                ),
            };
        }
        Ok(Self {
            canvas,
            variant,
            custom_colors,
            theme,
        })
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        let value = Value::object([
            ("schema", Value::str("paintcraft.web.preferences/v1")),
            ("canvas", self.canvas.value()),
            ("skin", Value::str(self.variant.id())),
            (
                "theme",
                self.theme.map_or(Value::Null, |theme| {
                    Value::str(if theme == Theme::Dark {
                        "dark"
                    } else {
                        "light"
                    })
                }),
            ),
            (
                "custom_colors",
                Value::Array(
                    self.custom_colors
                        .iter()
                        .map(|color| color.map_or(Value::Null, |color| Value::str(color.to_hex())))
                        .collect(),
                ),
            ),
        ]);
        let bytes = json::canonical(&value);
        Self::parse(&bytes)?;
        Ok(bytes)
    }

    pub fn load() -> (Self, Option<String>) {
        #[cfg(target_arch = "wasm32")]
        {
            let result = storage().and_then(|storage| {
                storage
                    .get_item(KEY)
                    .map_err(|_| "browser preferences cannot be read".to_owned())
            });
            match result {
                Ok(None) => (Self::default(), None),
                Ok(Some(value)) => match Self::parse(value.as_bytes()) {
                    Ok(prefs) => (prefs, None),
                    Err(error) => (
                        Self::default(),
                        Some(format!("Browser preferences not loaded: {error}")),
                    ),
                },
                Err(error) => (Self::default(), Some(error)),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            (Self::default(), None)
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let bytes = self.bytes()?;
        #[cfg(target_arch = "wasm32")]
        {
            storage()?
                .set_item(
                    KEY,
                    std::str::from_utf8(&bytes).map_err(|_| "invalid preferences encoding")?,
                )
                .map_err(|_| "browser preferences could not be saved".into())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = bytes;
            Ok(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("browser window is unavailable")?
        .local_storage()
        .map_err(|_| "browser preference storage is unavailable")?
        .ok_or_else(|| "browser preference storage is unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_preserves_default_and_remembered_size_independently() {
        let mut prefs = Preferences::default();
        assert_eq!(prefs.canvas.new_size(), (1000, 486));
        prefs.canvas.configured = (640, 480);
        prefs.canvas.last = Some((900, 700));
        prefs.variant = ClassicVariant::V7;
        prefs.theme = Some(Theme::Light);
        prefs.custom_colors[9] = Some(Rgba([32, 128, 255, 255]));
        assert_eq!(Preferences::parse(&prefs.bytes().unwrap()).unwrap(), prefs);
        assert_eq!(prefs.canvas.new_size(), (900, 700));
        prefs.canvas.remember_last = false;
        let decoded = Preferences::parse(&prefs.bytes().unwrap()).unwrap();
        assert_eq!(decoded.canvas.new_size(), (640, 480));
        assert_eq!(decoded.canvas.last, Some((900, 700)));
    }
    #[test]
    fn malformed_unknown_or_oversized_preferences_are_rejected() {
        assert!(Preferences::parse(b"{}").is_err());
        assert!(Preferences::parse(&vec![b' '; BYTES_MAX + 1]).is_err());
        let mut prefs = Preferences::default();
        prefs.canvas.configured = (4096, 4096);
        assert!(prefs.bytes().is_err());
        prefs.canvas.configured = (1000, 486);
        prefs.canvas.last = Some((4096, 4096));
        assert!(prefs.bytes().is_err());
        let mut value = json::parse(&Preferences::default().bytes().unwrap()).unwrap();
        let Value::Object(ref mut object) = value else {
            unreachable!()
        };
        object.insert("schema".into(), Value::str("unknown"));
        assert!(Preferences::parse(&json::canonical(&value)).is_err());
    }
}
