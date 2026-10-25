//! Font payloads are loaded once before the browser editor starts.
use std::sync::OnceLock;
#[cfg(target_arch = "wasm32")]
pub(super) static FONTS: Fonts = Fonts(OnceLock::new());
pub(super) struct Fonts(OnceLock<[Vec<u8>; 2]>);
impl Fonts {
    pub(super) fn data(&self) -> Option<[&[u8]; 2]> {
        self.0
            .get()
            .map(|data| [data[0].as_slice(), data[1].as_slice()])
    }
    pub(super) fn install(&self, japanese: Vec<u8>, chinese: Vec<u8>) -> Result<(), &'static str> {
        if [&japanese, &chinese].iter().any(|bytes| {
            bytes.len() < 32 || bytes.len() > 16 * 1024 * 1024 || !bytes.starts_with(b"OTTO")
        }) {
            return Err("Invalid browser font payload");
        }
        self.0
            .set([japanese, chinese])
            .map_err(|_| "Browser fonts are already installed")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_and_duplicate_initialization_do_not_replace_the_font_collection() {
        let fonts = Fonts(OnceLock::new());
        assert!(fonts.install(vec![], vec![]).is_err());
        assert!(fonts.data().is_none());
        let mut japanese = vec![0; 32];
        japanese[..4].copy_from_slice(b"OTTO");
        let mut chinese = japanese.clone();
        chinese[31] = 7;
        fonts.install(japanese.clone(), chinese.clone()).unwrap();
        assert_eq!(
            fonts.data().unwrap(),
            [japanese.as_slice(), chinese.as_slice()]
        );
        assert!(fonts.install(japanese, chinese).is_err());
        assert_eq!(fonts.data().unwrap()[1][31], 7);
    }
}
