use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
pub const FONT_BYTES_MAX: usize = 64 << 20;
const BUNDLED: [&[u8]; 2] = [
    include_bytes!("../../../../../assets/fonts/selawik/selawk.ttf"),
    include_bytes!("../../../../../assets/fonts/selawik/selawkb.ttf"),
];
#[cfg(test)]
mod tests;

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
    pub fn bundled_bytes() -> usize {
        BUNDLED.iter().map(|data| data.len()).sum()
    }
    /// Shared root assets, also used by the desktop/browser UI. Extra fonts must
    /// already be acquired by an authorized caller; SVG content cannot open them.
    pub fn with_bundled(additional: &[&[u8]], check: &dyn Fn() -> Result<()>) -> Result<Self> {
        if additional.len() > 30 {
            return Err(resource("SVG font inventory exceeds allowance"));
        }
        let mut data: Vec<&[u8]> = BUNDLED.to_vec();
        data.extend_from_slice(additional);
        Self::from_data(&data, "Selawik", check)
    }
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
                .filter(|n| *n <= FONT_BYTES_MAX)
                .ok_or_else(|| resource("SVG font bytes exceed allowance"))?;
            let before = database.len();
            let faces = face_count(font, 64 - before)?;
            database.load_font_data(font.to_vec());
            if database.len() - before != faces {
                return Err(malformed(
                    "invalid or partially unreadable SVG font collection",
                ));
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

fn face_count(bytes: &[u8], remaining: usize) -> Result<usize> {
    if bytes.len() < 12 {
        return Err(malformed("truncated SVG font header"));
    }
    if !bytes.starts_with(b"ttcf") {
        if ![b"\0\x01\0\0", b"OTTO"].contains(&bytes[..4].try_into().unwrap()) {
            return Err(unsupported(
                "SVG font data must be OpenType/TrueType or a bounded collection",
            ));
        }
        if remaining == 0 {
            return Err(resource("SVG font face count exceeds allowance"));
        }
        admit_face(bytes, 0)?;
        return Ok(1);
    }
    let version = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
    if ![0x00010000, 0x00020000].contains(&version) {
        return Err(unsupported("SVG font collection version is not qualified"));
    }
    let count = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if count == 0 || count > remaining {
        return Err(resource("SVG font collection face count exceeds allowance"));
    }
    let header = 12 + count * 4 + if version == 0x00020000 { 12 } else { 0 };
    if bytes.len() < header {
        return Err(malformed("truncated SVG font collection offsets"));
    }
    for offset in bytes[12..12 + count * 4].as_chunks::<4>().0 {
        let offset = u32::from_be_bytes(*offset) as usize;
        if offset < header || offset.checked_add(12).is_none_or(|end| end > bytes.len()) {
            return Err(malformed(
                "SVG font collection face offset is outside its payload",
            ));
        }
        admit_face(bytes, offset)?;
    }
    Ok(count)
}
fn admit_face(bytes: &[u8], offset: usize) -> Result<()> {
    if ![b"\0\x01\0\0", b"OTTO"].contains(&bytes[offset..offset + 4].try_into().unwrap()) {
        return Err(unsupported(
            "SVG font face does not use a qualified TrueType/OpenType scaler",
        ));
    }
    let count = usize::from(u16::from_be_bytes(
        bytes[offset + 4..offset + 6].try_into().unwrap(),
    ));
    if count == 0 || count > 512 {
        return Err(resource("SVG font table count exceeds allowance"));
    }
    let records = bytes
        .get(offset + 12..offset + 12 + count * 16)
        .ok_or_else(|| malformed("truncated SVG font table directory"))?;
    let mut tags = Vec::new();
    for record in records.as_chunks::<16>().0 {
        let tag: [u8; 4] = record[..4].try_into().unwrap();
        if tags.contains(&tag) {
            return Err(malformed("duplicate SVG font table tag"));
        }
        tags.push(tag);
        if [
            *b"SVG ", *b"sbix", *b"CBDT", *b"CBLC", *b"EBDT", *b"EBLC", *b"COLR", *b"CPAL",
        ]
        .contains(&tag)
        {
            // Font glyph SVGs use usvg's default Options, bypassing the import
            // resolver; bitmap/color glyphs also bypass embedded-image admission.
            return Err(unsupported(
                "SVG rasterization currently requires outline fonts without embedded SVG bitmap or color glyph tables",
            ));
        }
        let start = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
        let size = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
        let end = start
            .checked_add(size)
            .filter(|n| *n <= bytes.len())
            .ok_or_else(|| malformed("SVG font table exceeds its file boundary"))?;
        if tag == *b"name" {
            admit_names(&bytes[start..end])?;
        }
    }
    if !tags
        .iter()
        .any(|tag| [*b"glyf", *b"CFF ", *b"CFF2"].contains(tag))
    {
        return Err(unsupported("SVG font face has no qualified outline table"));
    }
    Ok(())
}
fn admit_names(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 6 {
        return Err(malformed("truncated SVG font name table"));
    }
    if bytes.len() > 1 << 20 {
        return Err(resource("SVG font name metadata exceeds allowance"));
    }
    let count = usize::from(u16::from_be_bytes(bytes[2..4].try_into().unwrap()));
    if count > 1024 {
        return Err(resource("SVG font name record count exceeds allowance"));
    }
    let records = bytes
        .get(6..6 + count * 12)
        .ok_or_else(|| malformed("truncated SVG font name records"))?;
    let base = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
    let mut decoded = 0usize;
    for record in records.as_chunks::<12>().0 {
        let size = usize::from(u16::from_be_bytes(record[8..10].try_into().unwrap()));
        let start = base + usize::from(u16::from_be_bytes(record[10..12].try_into().unwrap()));
        if start + size > bytes.len() {
            return Err(malformed("SVG font name points outside its table"));
        }
        decoded += size;
        if decoded > 1 << 20 {
            return Err(resource("SVG expanded font names exceed allowance"));
        }
    }
    Ok(())
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
