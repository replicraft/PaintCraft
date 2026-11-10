use super::*;
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("paintcraft-language-hint-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn entries(&self) -> Vec<PathBuf> {
        let mut entries: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        entries
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn japanese_hint_uses_existing_parser_without_rewriting_current_or_legacy_settings() {
    let dir = Scratch::new();
    Settings {
        language: Language::Japanese,
        ..Settings::default()
    }
    .save(&dir.0)
    .unwrap();
    let bytes = fs::read(file(&dir.0)).unwrap();
    let entries = dir.entries();
    assert_eq!(Settings::language_hint(&dir.0), Language::Japanese);
    assert_eq!(fs::read(file(&dir.0)).unwrap(), bytes);
    assert_eq!(dir.entries(), entries);
    let legacy = br#"{"schema":"paintcraft.settings/v1","language":"ja"}"#;
    fs::write(file(&dir.0), legacy).unwrap();
    assert_eq!(Settings::language_hint(&dir.0), Language::Japanese);
    assert_eq!(fs::read(file(&dir.0)).unwrap(), legacy);
    assert_eq!(dir.entries(), entries);
}
#[test]
fn invalid_hint_never_moves_or_migrates_the_original_file() {
    for bytes in [
        b"\xff\xfe invalid JSON".as_slice(),
        br#"{"schema":"paintcraft.settings/v999","language":"ja"}"#.as_slice(),
        br#"{"schema":"paintcraft.settings/v2","experience":"classic","language":"xx"}"#.as_slice(),
        br#"{"schema":"paintcraft.settings/v2","experience":"unknown","language":"ja"}"#.as_slice(),
        br#"{"schema":"paintcraft.settings/v2","experience":"classic","language":7}"#.as_slice(),
    ] {
        let dir = Scratch::new();
        fs::write(file(&dir.0), bytes).unwrap();
        let entries = dir.entries();
        assert_eq!(Settings::language_hint(&dir.0), Language::default());
        assert_eq!(fs::read(file(&dir.0)).unwrap(), bytes);
        assert_eq!(dir.entries(), entries);
    }
}
#[test]
fn absent_settings_or_directory_use_default_without_creating_files() {
    let dir = Scratch::new();
    assert_eq!(Settings::language_hint(&dir.0), Language::default());
    assert!(dir.entries().is_empty());
    let missing = dir.0.join("not-created");
    assert!(!missing.exists());
    assert_eq!(Settings::language_hint(&missing), Language::default());
    assert!(!missing.exists());
    assert!(dir.entries().is_empty());
}
