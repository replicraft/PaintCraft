use super::*;
use std::{cell::Cell, fs};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/source-inspection/unit")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        MUTATIONS
            .lock()
            .unwrap()
            .retain(|(path, _)| !path.starts_with(&self.0));
        fs::remove_dir_all(&self.0).unwrap();
    }
}
type Hook = Box<dyn FnOnce() + Send>;
static MUTATIONS: std::sync::Mutex<Vec<(PathBuf, Hook)>> = std::sync::Mutex::new(Vec::new());
pub(crate) fn schedule(path: PathBuf, hook: impl FnOnce() + Send + 'static) {
    MUTATIONS.lock().unwrap().push((path, Box::new(hook)));
}
pub(crate) fn remove_under(path: &Path) {
    MUTATIONS
        .lock()
        .unwrap()
        .retain(|(p, _)| !p.starts_with(path));
}
pub(super) fn before_revalidation(path: &Path) {
    let hook = {
        let mut jobs = MUTATIONS.lock().unwrap();
        jobs.iter()
            .position(|(p, _)| p == path)
            .map(|at| jobs.remove(at).1)
    };
    if let Some(hook) = hook {
        hook();
    }
}
#[test]
fn actual_captured_handle_detects_same_bytes_replacement_and_in_place_mutation() {
    let folder = Folder::new();
    let bytes = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/bmp-rle-fixtures/rle8-padding.bmp"
    );
    for replace in [false, true] {
        let path = folder.0.join(format!("source-{replace}.bmp"));
        fs::write(&path, bytes).unwrap();
        let input = path.clone();
        schedule(path.clone(), move || {
            let bytes = fs::read(&input).unwrap();
            if replace {
                fs::rename(&input, input.with_extension("original")).unwrap();
                fs::write(input, bytes).unwrap();
            } else {
                let mut changed = bytes;
                *changed.last_mut().unwrap() ^= 1;
                fs::write(input, changed).unwrap();
            }
        });
        assert_eq!(
            inspect_path(&path, None, Limits::default(), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        fs::write(&path, bytes).unwrap();
        if replace {
            fs::remove_file(path.with_extension("original")).unwrap();
        }
        let (snapshot, sha, identity) = acquired(&path, Limits::default(), &|| Ok(())).unwrap();
        let source = source_inspection::inspect(&snapshot, Limits::default(), &|| Ok(())).unwrap();
        let value = json::report(&source, Limits::default()).unwrap();
        assert!(matches!(value, Value::Object(_)));
        if replace {
            fs::rename(&path, path.with_extension("original")).unwrap();
            fs::write(&path, bytes).unwrap();
        } else {
            let mut changed = bytes.to_vec();
            *changed.last_mut().unwrap() ^= 1;
            fs::write(&path, changed).unwrap();
        }
        assert_eq!(
            identity
                .verify_bytes(&path, bytes.len() as u64, &sha, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn command_checks_cancellation_hash_and_caller_limits_without_output_side_effects() {
    let folder = Folder::new();
    let path = folder.0.join("input.bmp");
    let bytes = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/bmp-rle-fixtures/rle4-combined.bmp"
    );
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        inspect_path(&path, Some(&"0".repeat(64)), Limits::default(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(
        inspect_path(
            &path,
            None,
            Limits {
                input_bytes: 1,
                ..Limits::default()
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let calls = Cell::new(0);
    inspect_path(&path, None, Limits::default(), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let at = Cell::new(0);
        assert_eq!(
            inspect_path(&path, None, Limits::default(), &|| {
                at.set(at.get() + 1);
                if at.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel source read/report",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 1);
}
