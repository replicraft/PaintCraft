use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// An explicit in-memory font collection. The adapter never enumerates or opens
/// installed fonts; callers may supply font bytes they have separately admitted.
#[derive(Clone)]
pub struct Fonts {
    pub(super) database: Arc<usvg::fontdb::Database>,
    pub(super) default_family: String,
}
impl std::fmt::Debug for Fonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvgFonts")
            .field("faces", &self.database.len())
            .field("default_family", &self.default_family)
            .finish_non_exhaustive()
    }
}
impl Default for Fonts {
    fn default() -> Self {
        Self {
            database: Arc::new(usvg::fontdb::Database::new()),
            default_family: "Selawik".into(),
        }
    }
}
impl Fonts {
    pub fn from_data(
        data: &[&[u8]],
        default_family: &str,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        if data.len() > 32 || default_family.len() > 256 {
            return Err(resource("SVG font inventory exceeds allowance"));
        }
        let mut bytes = 0usize;
        let mut database = usvg::fontdb::Database::new();
        for font in data {
            check()?;
            bytes = bytes
                .checked_add(font.len())
                .filter(|n| *n <= 64 << 20)
                .ok_or_else(|| resource("SVG font bytes exceed allowance"))?;
            let before = database.len();
            database.load_font_data(font.to_vec());
            if database.len() == before {
                return Err(malformed("invalid SVG font data"));
            }
            if database.len() > 64 {
                return Err(resource("SVG font face count exceeds allowance"));
            }
        }
        if !database.faces().any(|face| {
            face.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(default_family))
        }) {
            return Err(unsupported(
                "SVG default font family is absent from the explicit font collection",
            ));
        }
        database.set_serif_family(default_family);
        database.set_sans_serif_family(default_family);
        database.set_cursive_family(default_family);
        database.set_fantasy_family(default_family);
        database.set_monospace_family(default_family);
        Ok(Self {
            database: Arc::new(database),
            default_family: default_family.into(),
        })
    }
}
#[derive(Default)]
pub(super) struct Events {
    substituted: Arc<AtomicBool>,
    missing: Arc<AtomicBool>,
}
impl Events {
    pub fn resolver(&self) -> usvg::FontResolver<'static> {
        let substituted = Arc::clone(&self.substituted);
        let missing = Arc::clone(&self.missing);
        let select = usvg::FontResolver::default_font_selector();
        let fallback = usvg::FontResolver::default_fallback_selector();
        usvg::FontResolver {
            select_font: Box::new(move |font, db| {
                let id = select(font, db)?;
                if let Some(face) = db.face(id) {
                    let requested = font.families().iter().any(|family| match family {
                        usvg::FontFamily::Named(name) => face
                            .families
                            .iter()
                            .any(|(actual, _)| actual.eq_ignore_ascii_case(name)),
                        _ => true,
                    });
                    if !requested {
                        substituted.store(true, Ordering::Relaxed);
                    }
                }
                Some(id)
            }),
            select_fallback: Box::new(move |ch, used, db| {
                let selected = fallback(ch, used, db);
                if selected.is_none() {
                    missing.store(true, Ordering::Relaxed);
                }
                selected
            }),
        }
    }
    pub fn finish(&self, losses: &mut Vec<String>) -> Result<()> {
        if self.missing.load(Ordering::Relaxed) {
            return Err(unsupported(
                "SVG text has glyphs missing from the explicit font collection",
            ));
        }
        if self.substituted.load(Ordering::Relaxed) {
            loss(losses, "svg-font-substituted");
        }
        Ok(())
    }
}
