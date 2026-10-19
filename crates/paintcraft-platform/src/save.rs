//! The parent-owned save service: stage next to the destination, replace
//! atomically, then confirm what actually happened. Every outcome is reported
//! as a `paintcraft.file-operation/v1` receipt; a failure after the replace
//! request is never described as "nothing changed".

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use paintcraft_types::json::Value;
use paintcraft_types::policy::hex;
use paintcraft_types::{Error, ErrorCode};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replacement {
    NotAttempted,
    NotReplaced,
    Replaced,
    Unknown,
}

impl Replacement {
    fn as_str(self) -> &'static str {
        match self {
            Replacement::NotAttempted => "not-attempted",
            Replacement::NotReplaced => "not-replaced",
            Replacement::Replaced => "replaced",
            Replacement::Unknown => "unknown",
        }
    }
}

/// What the caller says about the operation being recorded.
#[derive(Debug)]
pub struct Context<'a> {
    pub operation: &'static str,
    pub source_state: Uuid,
    pub source_sha256: Option<String>,
    pub losses_acknowledged: Vec<String>,
    pub destination: &'a Path,
}

#[derive(Debug)]
pub struct Outcome {
    pub replacement: Replacement,
    pub durable: bool,
    pub staged_sha256: Option<String>,
    pub observed_sha256: Option<String>,
    pub reasons: Vec<String>,
}

impl Outcome {
    /// Known replacement with the requested durability confirmed.
    pub fn is_success(&self) -> bool {
        self.replacement == Replacement::Replaced && self.durable
    }

    pub fn receipt(&self, ctx: &Context<'_>) -> Value {
        // Only a durable save or save-as clears the associated native state; an export never does.
        let acknowledged = self.replacement == Replacement::Replaced
            && self.durable
            && matches!(ctx.operation, "save" | "save-as");
        // A known replacement is never blindly repeated, durable or not.
        let retry = match self.replacement {
            Replacement::NotAttempted | Replacement::NotReplaced => "allowed-new-request",
            Replacement::Unknown => "inspect",
            Replacement::Replaced => "no-automatic-retry",
        };
        let text = |s: &String| Value::str(s);
        Value::object([
            ("schema", Value::str("paintcraft.file-operation/v1")),
            ("operation_id", Value::str(Uuid::new_v4().to_string())),
            ("operation", Value::str(ctx.operation)),
            ("source_state_id", Value::str(ctx.source_state.to_string())),
            (
                "source_sha256",
                ctx.source_sha256.as_ref().map_or(Value::Null, text),
            ),
            (
                "destination_id",
                Value::str(destination_id(ctx.destination)),
            ),
            ("save_sequence", Value::str("1")),
            ("binding_generation", Value::str("0")),
            ("replacement", Value::str(self.replacement.as_str())),
            ("requested_durability", Value::str("durable")),
            (
                "durability",
                Value::str(
                    if self.replacement == Replacement::Replaced && self.durable {
                        "confirmed"
                    } else {
                        "not-confirmed"
                    },
                ),
            ),
            (
                "staged_sha256",
                self.staged_sha256.as_ref().map_or(Value::Null, text),
            ),
            (
                "observed_destination_sha256",
                self.observed_sha256.as_ref().map_or(Value::Null, text),
            ),
            (
                "acknowledged_state_id",
                if acknowledged {
                    Value::str(ctx.source_state.to_string())
                } else {
                    Value::Null
                },
            ),
            ("source_lineage_adopted", Value::Bool(false)),
            ("retry", Value::str(retry)),
            (
                "losses_acknowledged",
                Value::Array(ctx.losses_acknowledged.iter().map(text).collect()),
            ),
            (
                "reasons",
                Value::Array(self.reasons.iter().map(text).collect()),
            ),
            ("synthetic", Value::Bool(false)),
        ])
    }
}

/// A stable, path-free identifier for a destination.
pub fn destination_id(path: &Path) -> String {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    format!(
        "dest:{}",
        &hex(&Sha256::digest(abs.to_string_lossy().as_bytes()))[..32]
    )
}

/// Refuse before doing any work when the destination exists and replacement was
/// not requested.
pub fn preflight(dest: &Path, overwrite: bool) -> Result<(), Error> {
    if dest.is_dir() {
        return Err(Error::new(
            ErrorCode::TargetInvalid,
            "output path is a directory",
        ));
    }
    if !overwrite && fs::symlink_metadata(dest).is_ok() {
        return Err(Error::new(
            ErrorCode::TargetInvalid,
            "output exists; pass --overwrite to replace it",
        ));
    }
    Ok(())
}

fn staging_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map_or_else(|| "output".into(), |n| n.to_string_lossy().into_owned());
    dest.with_file_name(format!(
        ".{name}.paintcraft-{}.tmp",
        Uuid::new_v4().simple()
    ))
}

/// The bytes could not be produced, so no destination work was attempted.
pub fn not_attempted(reason: &Error) -> Outcome {
    Outcome {
        replacement: Replacement::NotAttempted,
        durable: false,
        staged_sha256: None,
        observed_sha256: None,
        reasons: vec![format!("{}: {}", reason.code.as_str(), reason.message)],
    }
}

pub fn save(dest: &Path, bytes: &[u8], overwrite: bool) -> Outcome {
    let staged_sha256 = hex(&Sha256::digest(bytes));
    let mut out = Outcome {
        replacement: Replacement::NotReplaced,
        durable: false,
        staged_sha256: Some(staged_sha256.clone()),
        observed_sha256: None,
        reasons: Vec::new(),
    };
    let tmp = staging_path(dest);
    let staged = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| {
            f.write_all(bytes)?;
            f.sync_all()
        });
    if let Err(e) = staged {
        out.reasons.push(format!("staging failed: {}", e.kind()));
        let _ = fs::remove_file(&tmp);
        return out;
    }
    // Without --overwrite a hard link is an atomic "create only if absent".
    let placed = if overwrite {
        fs::rename(&tmp, dest)
    } else {
        fs::hard_link(&tmp, dest).and_then(|()| fs::remove_file(&tmp))
    };
    if let Err(e) = placed {
        out.reasons
            .push(format!("replacement failed: {}", e.kind()));
        let _ = fs::remove_file(&tmp);
        return out;
    }
    out.replacement = Replacement::Replaced;
    match fs::read(dest) {
        Ok(now) => {
            let seen = hex(&Sha256::digest(&now));
            if seen != staged_sha256 {
                out.replacement = Replacement::Unknown;
                out.reasons.push(
                    "destination content differs from the staged bytes after replacement".into(),
                );
            }
            out.observed_sha256 = Some(seen);
        }
        Err(e) => {
            out.replacement = Replacement::Unknown;
            out.reasons
                .push(format!("destination could not be re-read: {}", e.kind()));
        }
    }
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match File::open(parent).and_then(|d| d.sync_all()) {
        Ok(()) => out.durable = out.replacement == Replacement::Replaced,
        Err(e) => out
            .reasons
            .push(format!("directory sync failed: {}", e.kind())),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "paintcraft-test-{name}-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn creates_new_files_and_leaves_no_staging_files() {
        let dir = scratch("create");
        let dest = dir.join("a.pcnd");
        let out = save(&dest, b"hello", false);
        assert!(out.is_success(), "{:?}", out.reasons);
        assert_eq!(fs::read(&dest).unwrap(), b"hello");
        assert_eq!(out.observed_sha256, out.staged_sha256);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_to_clobber_without_overwrite_and_keeps_the_original() {
        let dir = scratch("clobber");
        let dest = dir.join("a.pcnd");
        fs::write(&dest, b"original").unwrap();
        assert_eq!(
            preflight(&dest, false).unwrap_err().code,
            ErrorCode::TargetInvalid
        );
        let out = save(&dest, b"new", false);
        assert_eq!(out.replacement, Replacement::NotReplaced);
        assert_eq!(fs::read(&dest).unwrap(), b"original");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "staging file removed"
        );
        let out = save(&dest, b"new", true);
        assert!(out.is_success());
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_parent_is_a_known_non_replacement() {
        let dir = scratch("missing");
        let out = save(&dir.join("no/such/dir/a.pcnd"), b"x", true);
        assert_eq!(out.replacement, Replacement::NotReplaced);
        assert!(!out.is_success() && !out.reasons.is_empty());
        let ctx = Context {
            operation: "save-as",
            source_state: Uuid::nil(),
            source_sha256: None,
            losses_acknowledged: vec![],
            destination: &dir,
        };
        let receipt = out.receipt(&ctx);
        assert_eq!(
            receipt.get("retry").and_then(Value::as_str),
            Some("allowed-new-request")
        );
        assert_eq!(receipt.get("acknowledged_state_id"), Some(&Value::Null));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn directories_and_receipt_shape() {
        let dir = scratch("dir");
        assert_eq!(
            preflight(&dir, true).unwrap_err().code,
            ErrorCode::TargetInvalid
        );
        let dest = dir.join("a.pcnd");
        let out = save(&dest, b"abc", false);
        let ctx = Context {
            operation: "save-as",
            source_state: Uuid::nil(),
            source_sha256: Some("a".repeat(64)),
            losses_acknowledged: vec![],
            destination: &dest,
        };
        let r = out.receipt(&ctx);
        for key in ["replacement", "durability", "retry"] {
            assert!(r.get(key).and_then(Value::as_str).is_some(), "{key}");
        }
        assert_eq!(
            r.get("replacement").and_then(Value::as_str),
            Some("replaced")
        );
        assert_eq!(
            r.get("durability").and_then(Value::as_str),
            Some("confirmed")
        );
        assert!(destination_id(&dest).starts_with("dest:") && !destination_id(&dest).contains('/'));
        fs::remove_dir_all(dir).unwrap();
    }
}
