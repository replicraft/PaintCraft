//! Bounded native recent-file metadata, independent of document/settings contents.
use paintcraft_core::types::json::{self, Value};
use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

pub(super) const LIMIT: usize = 16;
const PATH_BYTES: usize = 65_536;
const FILE_BYTES: usize = LIMIT * PATH_BYTES * 2 + 4096;

#[derive(Debug, Default)]
pub(crate) struct History {
    pub entries: Vec<PathBuf>,
    pub warning: Option<String>,
    dir: Option<PathBuf>,
    writable: bool,
}
impl History {
    pub fn load(dir: PathBuf) -> Self {
        let file = dir.join("recent-files.json");
        let mut history = Self {
            dir: Some(dir),
            writable: true,
            ..Self::default()
        };
        let read = || -> Result<Vec<PathBuf>, String> {
            let file = match fs::File::open(&file) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                Err(e) => return Err(e.to_string()),
            };
            let mut bytes = Vec::new();
            file.take(FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > FILE_BYTES {
                return Err("recent-file history exceeds its size limit".into());
            }
            parse(&bytes)
        };
        match read() {
            Ok(entries) => history.entries = entries,
            Err(error) => {
                history.warning = Some(error);
                history.writable = (0..100)
                    .map(|n| file.with_extension(format!("json.unusable-{n}")))
                    .find(|path| !path.exists())
                    .is_some_and(|aside| fs::rename(&file, aside).is_ok());
            }
        }
        history
    }
    pub fn remember(&mut self, path: &Path) {
        let Ok(path) = fs::canonicalize(path).or_else(|_| std::path::absolute(path)) else {
            return;
        };
        if encode(&path).is_err() {
            return;
        }
        self.entries.retain(|old| old != &path);
        self.entries.insert(0, path);
        self.entries.truncate(LIMIT);
        self.persist();
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.writable = true;
        self.persist();
    }
    fn persist(&mut self) {
        let Some(dir) = &self.dir else { return };
        if !self.writable {
            return;
        }
        let result = (|| -> Result<(), String> {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let entries = self
                .entries
                .iter()
                .map(|p| encode(p))
                .collect::<Result<Vec<_>, _>>()?;
            let value = Value::object([
                ("schema", Value::str("paintcraft.recent-files/v1")),
                ("paths", Value::Array(entries)),
            ]);
            let outcome = paintcraft_platform::save::save(
                &dir.join("recent-files.json"),
                &json::canonical(&value),
                true,
            );
            if outcome.is_success() {
                Ok(())
            } else {
                Err(outcome.reasons.join("; "))
            }
        })();
        self.warning = result.err();
    }
}
fn parse(bytes: &[u8]) -> Result<Vec<PathBuf>, String> {
    let value = json::parse(bytes).map_err(|e| e.to_string())?;
    if value.get("schema").and_then(Value::as_str) != Some("paintcraft.recent-files/v1") {
        return Err("unknown recent-file history version".into());
    }
    let Some(Value::Array(values)) = value.get("paths") else {
        return Err("missing recent paths".into());
    };
    if values.len() > LIMIT {
        return Err("too many recent paths".into());
    }
    let mut paths = Vec::new();
    for v in values {
        let path = decode(v)?;
        if !path.is_absolute() {
            return Err("recent paths must be absolute".into());
        }
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    Ok(paths)
}
fn encode(path: &Path) -> Result<Value, String> {
    #[cfg(unix)]
    let (kind, bytes) = {
        use std::os::unix::ffi::OsStrExt;
        ("unix-bytes", path.as_os_str().as_bytes().to_vec())
    };
    #[cfg(windows)]
    let (kind, bytes) = {
        use std::os::windows::ffi::OsStrExt;
        (
            "windows-utf16le",
            path.as_os_str()
                .encode_wide()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )
    };
    #[cfg(not(any(unix, windows)))]
    let (kind, bytes) = (
        "utf8",
        path.to_str()
            .ok_or("unsupported path encoding")?
            .as_bytes()
            .to_vec(),
    );
    if bytes.len() > PATH_BYTES {
        return Err("recent path exceeds its size limit".into());
    }
    Ok(Value::object([
        ("encoding", Value::str(kind)),
        (
            "data",
            Value::str(paintcraft_core::types::policy::hex(&bytes)),
        ),
    ]))
}
fn decode(value: &Value) -> Result<PathBuf, String> {
    let kind = value
        .get("encoding")
        .and_then(Value::as_str)
        .ok_or("missing path encoding")?;
    let data = value
        .get("data")
        .and_then(Value::as_str)
        .ok_or("missing path data")?;
    if data.is_empty() || data.len() > PATH_BYTES * 2 || data.len() % 2 != 0 || !data.is_ascii() {
        return Err("invalid encoded path length".into());
    }
    let bytes = (0..data.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&data[i..i + 2], 16).map_err(|_| "invalid path bytes".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    #[cfg(unix)]
    if kind == "unix-bytes" {
        use std::os::unix::ffi::OsStringExt;
        if bytes.contains(&0) {
            return Err("invalid NUL in recent path".into());
        }
        return Ok(std::ffi::OsString::from_vec(bytes).into());
    }
    #[cfg(windows)]
    if kind == "windows-utf16le" && bytes.len() % 2 == 0 {
        use std::os::windows::ffi::OsStringExt;
        let words = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect::<Vec<_>>();
        if words.contains(&0) {
            return Err("invalid NUL in recent path".into());
        }
        return Ok(std::ffi::OsString::from_wide(&words).into());
    }
    #[cfg(not(any(unix, windows)))]
    if kind == "utf8" {
        return String::from_utf8(bytes)
            .map(PathBuf::from)
            .map_err(|e| e.to_string());
    }
    Err("recent path encoding belongs to another platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("paintcraft-recents-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        fs::canonicalize(p).unwrap()
    }
    #[test]
    fn successful_records_are_bounded_deduplicated_and_reloaded_in_mru_order() {
        let dir = dir();
        let mut history = History::load(dir.clone());
        for i in 0..LIMIT + 3 {
            history.remember(&dir.join(format!("{i}.pcnd")));
        }
        assert_eq!(history.entries.len(), LIMIT);
        history.remember(&dir.join("4.pcnd"));
        assert_eq!(history.entries[0], dir.join("4.pcnd"));
        assert_eq!(
            history
                .entries
                .iter()
                .filter(|p| p.ends_with("4.pcnd"))
                .count(),
            1
        );
        assert_eq!(History::load(dir.clone()).entries, history.entries);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn corrupt_history_is_preserved_and_new_records_do_not_touch_documents() {
        let dir = dir();
        let file = dir.join("recent-files.json");
        fs::write(&file, b"broken JSON").unwrap();
        let picture = dir.join("picture.png");
        fs::write(&picture, b"keep this picture").unwrap();
        let mut history = History::load(dir.clone());
        assert!(history.warning.is_some());
        assert!(history.entries.is_empty());
        assert_eq!(
            fs::read(dir.join("recent-files.json.unusable-0")).unwrap(),
            b"broken JSON"
        );
        history.remember(&picture);
        history.clear();
        assert!(History::load(dir.clone()).entries.is_empty());
        assert_eq!(fs::read(&picture).unwrap(), b"keep this picture");
        assert_eq!(
            fs::read(dir.join("recent-files.json.unusable-0")).unwrap(),
            b"broken JSON"
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unreadable_history_is_not_silently_overwritten_and_memory_still_works() {
        let dir = dir();
        let file = dir.join("recent-files.json");
        fs::write(&file, b"damaged").unwrap();
        for i in 0..100 {
            fs::write(dir.join(format!("recent-files.json.unusable-{i}")), b"keep").unwrap();
        }
        let mut history = History::load(dir.clone());
        history.remember(&dir.join("new.pcnd"));
        assert_eq!(fs::read(&file).unwrap(), b"damaged");
        assert_eq!(history.entries.len(), 1);
        history.clear();
        assert!(History::load(dir.clone()).entries.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn malformed_relative_platform_and_oversized_inputs_are_rejected() {
        let dir = dir();
        let mut value = Value::object([
            ("schema", Value::str("paintcraft.recent-files/v1")),
            (
                "paths",
                Value::Array(vec![encode(Path::new("relative.pcnd")).unwrap()]),
            ),
        ]);
        assert!(parse(&json::canonical(&value)).is_err());
        let Value::Object(ref mut object) = value else {
            unreachable!()
        };
        object.insert(
            "paths".into(),
            Value::Array(vec![encode(&dir).unwrap(); LIMIT + 1]),
        );
        assert!(parse(&json::canonical(&value)).is_err());
        assert!(
            decode(&Value::object([
                ("encoding", Value::str("unknown")),
                ("data", Value::str("2f61"))
            ]))
            .is_err()
        );
        assert!(
            decode(&Value::object([
                ("encoding", Value::str("unix-bytes")),
                ("data", Value::str("00"))
            ]))
            .is_err()
        );
        let bytes = vec![b' '; FILE_BYTES + 1];
        fs::write(dir.join("recent-files.json"), &bytes).unwrap();
        let history = History::load(dir.clone());
        assert!(history.warning.is_some());
        assert_eq!(
            fs::metadata(dir.join("recent-files.json.unusable-0"))
                .unwrap()
                .len(),
            bytes.len() as u64
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn aliases_deduplicate_and_non_utf8_names_round_trip_losslessly() {
        use std::os::unix::{ffi::OsStringExt, fs::symlink};
        let dir = dir();
        let file = dir.join("ink.pcnd");
        fs::write(&file, b"data").unwrap();
        let alias = dir.join("alias.pcnd");
        symlink(&file, &alias).unwrap();
        let mut history = History::load(dir.clone());
        history.remember(&file);
        history.remember(&alias);
        assert_eq!(history.entries, vec![file]);
        // Test non-UTF-8 metadata without requiring the filesystem to create such a name.
        let raw = dir.join(std::ffi::OsString::from_vec(b"ink-\xff.pcnd".to_vec()));
        history.remember(&raw);
        assert_eq!(history.entries[0], raw);
        assert_eq!(History::load(dir.clone()).entries, history.entries);
        fs::remove_dir_all(dir).unwrap();
    }
}
