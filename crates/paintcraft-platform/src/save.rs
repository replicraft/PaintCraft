//! The parent-owned save service: stage next to the destination, replace
//! atomically, then confirm what actually happened. Every outcome is reported
//! as a `paintcraft.file-operation/v1` receipt; a failure after the replace
//! request is never described as "nothing changed".

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write};
use std::path::{Path, PathBuf};

use paintcraft_core::types::json::Value;
use paintcraft_core::types::policy::hex;
use paintcraft_core::types::{Error, ErrorCode};
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
    operation_id: Uuid,
    destination_id: Option<String>,
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
        self.receipt_stamped(ctx, 1, 0)
    }

    /// Coordinator-provided ordering and association stamps, captured at admission.
    pub fn receipt_stamped(
        &self,
        ctx: &Context<'_>,
        sequence: u64,
        binding_generation: u64,
    ) -> Value {
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
            ("operation_id", Value::str(self.operation_id.to_string())),
            ("operation", Value::str(ctx.operation)),
            ("source_state_id", Value::str(ctx.source_state.to_string())),
            (
                "source_sha256",
                ctx.source_sha256.as_ref().map_or(Value::Null, text),
            ),
            (
                "destination_id",
                Value::str(
                    self.destination_id
                        .clone()
                        .unwrap_or_else(|| destination_id(ctx.destination)),
                ),
            ),
            ("save_sequence", Value::str(sequence.to_string())),
            (
                "binding_generation",
                Value::str(binding_generation.to_string()),
            ),
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
    let resolved = resolve_destination(path)
        .unwrap_or_else(|_| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    path_id(&resolved)
}

fn path_id(path: &Path) -> String {
    format!(
        "dest:{}",
        &hex(&Sha256::digest(path.as_os_str().as_encoded_bytes()))[..32]
    )
}

/// Capture a replacement namespace. Follow an existing symlink and resolve parent
/// aliases for a new file. Dangling symlinks are rejected rather than overwritten.
fn resolve_destination(path: &Path) -> std::io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Ok(resolved) => Ok(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(error);
            }
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let name = path
                .file_name()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
            Ok(fs::canonicalize(parent)?.join(name))
        }
        Err(error) => Err(error),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    bytes: u64,
    hash: String,
}
fn fingerprint(path: &Path, limit: u64) -> std::io::Result<Option<Fingerprint>> {
    match fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
            }
            if metadata.len() > limit {
                return Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge));
            }
            Ok(Some(Fingerprint {
                bytes: metadata.len(),
                hash: file_hash(path, metadata.len())?,
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
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
        operation_id: Uuid::new_v4(),
        destination_id: None,
        replacement: Replacement::NotAttempted,
        durable: false,
        staged_sha256: None,
        observed_sha256: None,
        reasons: vec![format!("{}: {}", reason.code.as_str(), reason.message)],
    }
}

/// Default byte quota for the currently admitted native archive subset.
pub const STAGED_FILE_BYTES_MAX: u64 = u32::MAX as u64 - 1;

pub fn save(dest: &Path, bytes: &[u8], overwrite: bool) -> Outcome {
    save_with(dest, overwrite, STAGED_FILE_BYTES_MAX, |sink| {
        sink.write_all(bytes)
            .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))
    })
}

/// The coordinator owns the destination and replacement. The encoder receives only
/// a bounded writable staging capability; it cannot replace a destination through it.
/// Encoding and disk backpressure are synchronous here; scheduling belongs to callers.
pub fn save_with(
    dest: &Path,
    overwrite: bool,
    byte_limit: u64,
    encode: impl FnOnce(&mut dyn Write) -> Result<(), Error>,
) -> Outcome {
    save_impl(dest, overwrite, byte_limit, encode, |_| Ok(()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Encode,
    StageSync,
    Replace,
    TemporaryCleanup,
    ReadBack,
    DirectorySync,
}

struct StagingWriter {
    file: File,
    hash: Sha256,
    bytes: u64,
    limit: u64,
    failed: bool,
}
impl Write for StagingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() as u64 > self.limit.saturating_sub(self.bytes) {
            self.failed = true;
            return Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge));
        }
        let written = self.file.write(bytes).inspect_err(|_| self.failed = true)?;
        self.hash.update(&bytes[..written]);
        self.bytes += written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush().inspect_err(|_| self.failed = true)
    }
}

/// Read no more than the expected length plus one, even if an external writer
/// extends the file concurrently. No second archive-sized allocation is retained.
fn file_hash(path: &Path, expected_bytes: u64) -> std::io::Result<String> {
    let mut input = File::open(path)?.take(expected_bytes.saturating_add(1));
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut total = 0;
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        hash.update(&buffer[..read]);
    }
    if total != expected_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file size changed",
        ));
    }
    Ok(hex(&hash.finalize()))
}

fn save_impl(
    dest: &Path,
    overwrite: bool,
    byte_limit: u64,
    encode: impl FnOnce(&mut dyn Write) -> Result<(), Error>,
    mut barrier: impl FnMut(Phase) -> std::io::Result<()>,
) -> Outcome {
    let mut out = Outcome {
        operation_id: Uuid::new_v4(),
        destination_id: None,
        replacement: Replacement::NotReplaced,
        durable: false,
        staged_sha256: None,
        observed_sha256: None,
        reasons: Vec::new(),
    };
    let prepared_destination = resolve_destination(dest).and_then(|resolved| {
        let expected = fingerprint(&resolved, STAGED_FILE_BYTES_MAX)?;
        Ok((resolved, expected))
    });
    let (resolved, expected) = match prepared_destination {
        Ok(prepared) => prepared,
        Err(error) => {
            out.reasons
                .push(format!("destination admission failed: {}", error.kind()));
            return out;
        }
    };
    let dest = resolved.as_path();
    out.destination_id = Some(path_id(dest));
    let tmp = staging_path(dest);
    let staged = (|| -> Result<(String, u64), Error> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(io_error)?;
        let mut sink = StagingWriter {
            file,
            hash: Sha256::new(),
            bytes: 0,
            limit: byte_limit,
            failed: false,
        };
        barrier(Phase::Encode).map_err(io_error)?;
        encode(&mut sink)?;
        if sink.failed {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                "encoder ignored a failed output write",
            ));
        }
        barrier(Phase::StageSync).map_err(io_error)?;
        sink.file.sync_all().map_err(io_error)?;
        let digest = hex(&sink.hash.finalize());
        let observed = file_hash(&tmp, sink.bytes).map_err(io_error)?;
        if observed != digest {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                "staging content differs from encoded bytes",
            ));
        }
        Ok((digest, sink.bytes))
    })();
    let (staged_sha256, byte_count) = match staged {
        Ok(staged) => staged,
        Err(error) => {
            out.reasons.push(format!("staging failed: {error}"));
            let _ = fs::remove_file(&tmp);
            return out;
        }
    };
    out.staged_sha256 = Some(staged_sha256.clone());
    if let Err(error) = barrier(Phase::Replace) {
        out.reasons
            .push(format!("before replacement failed: {}", error.kind()));
        let _ = fs::remove_file(&tmp);
        return out;
    }
    // This catches changes during encoding. It is best effort: portable rename
    // cannot provide atomic content compare-and-swap against external writers.
    match fingerprint(dest, STAGED_FILE_BYTES_MAX) {
        Ok(observed) if observed == expected => {}
        observed => {
            out.reasons.push(match observed {
                Ok(_) => "destination changed while encoding; inspect or choose Save As".into(),
                Err(error) => format!(
                    "destination could not be checked before replacement: {}",
                    error.kind()
                ),
            });
            let _ = fs::remove_file(&tmp);
            return out;
        }
    }
    // Hard linking is atomic create-if-absent. Link success is the replacement
    // boundary; a subsequent temporary-link cleanup failure cannot undo it.
    let placed = if overwrite {
        fs::rename(&tmp, dest)
    } else {
        fs::hard_link(&tmp, dest)
    };
    if let Err(error) = placed {
        // A failed create-if-absent with an existing target is known not to replace.
        // Other adapter failures are conservatively ambiguous; retain useful staging.
        if !overwrite && error.kind() == std::io::ErrorKind::AlreadyExists {
            let _ = fs::remove_file(&tmp);
        } else {
            out.replacement = Replacement::Unknown;
        }
        out.reasons
            .push(format!("replacement failed: {}", error.kind()));
        return out;
    }
    out.replacement = Replacement::Replaced;
    if !overwrite
        && let Err(error) = barrier(Phase::TemporaryCleanup).and_then(|()| fs::remove_file(&tmp))
    {
        out.reasons
            .push(format!("temporary link cleanup failed: {}", error.kind()));
    }
    if let Err(error) = barrier(Phase::ReadBack) {
        out.reasons
            .push(format!("after replacement failed: {}", error.kind()));
        return out;
    }
    match file_hash(dest, byte_count) {
        Ok(seen) => {
            if seen != staged_sha256 {
                out.replacement = Replacement::Unknown;
                out.reasons.push(
                    "destination content differs from the staged bytes after replacement".into(),
                );
            }
            out.observed_sha256 = Some(seen);
        }
        Err(error) => {
            out.replacement = Replacement::Unknown;
            out.reasons.push(format!(
                "destination could not be re-read: {}",
                error.kind()
            ));
        }
    }
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match barrier(Phase::DirectorySync)
        .and_then(|()| File::open(parent).and_then(|dir| dir.sync_all()))
    {
        Ok(()) => out.durable = out.replacement == Replacement::Replaced,
        Err(error) => out
            .reasons
            .push(format!("directory sync failed: {}", error.kind())),
    }
    out
}

fn io_error(error: std::io::Error) -> Error {
    Error::new(
        if error.kind() == std::io::ErrorKind::FileTooLarge {
            ErrorCode::ResourceLimit
        } else {
            ErrorCode::StorageFailed
        },
        format!("save output failed: {}", error.kind()),
    )
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
    fn phase_failures_preserve_truthful_replacement_outcomes() {
        for phase in [
            Phase::Encode,
            Phase::StageSync,
            Phase::Replace,
            Phase::ReadBack,
            Phase::DirectorySync,
        ] {
            let dir = scratch("phase");
            let dest = dir.join("a.pcnd");
            fs::write(&dest, b"original").unwrap();
            let out = save_impl(
                &dest,
                true,
                1024,
                |sink| sink.write_all(b"new").map_err(io_error),
                |current| {
                    if current == phase {
                        Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
                    } else {
                        Ok(())
                    }
                },
            );
            let replaced = matches!(phase, Phase::ReadBack | Phase::DirectorySync);
            assert_eq!(
                out.replacement,
                if replaced {
                    Replacement::Replaced
                } else {
                    Replacement::NotReplaced
                }
            );
            assert_eq!(
                fs::read(&dest).unwrap(),
                if replaced {
                    b"new".as_slice()
                } else {
                    b"original".as_slice()
                }
            );
            assert!(!out.is_success());
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn sink_quota_and_encoder_failures_cannot_publish_partial_files() {
        let dir = scratch("quota");
        let dest = dir.join("a.pcnd");
        fs::write(&dest, b"original").unwrap();
        let out = save_with(&dest, true, 2, |sink| {
            // Even an encoder incorrectly ignoring the failed write cannot publish.
            let _ = sink.write_all(b"longer than quota");
            Ok(())
        });
        assert_eq!(out.replacement, Replacement::NotReplaced);
        assert_eq!(fs::read(&dest).unwrap(), b"original");
        let out = save_with(&dest, true, 1024, |sink| {
            sink.write_all(b"partial").map_err(io_error)?;
            Err(Error::new(ErrorCode::CodecFailed, "injected encoder error"))
        });
        assert_eq!(out.replacement, Replacement::NotReplaced);
        assert_eq!(fs::read(&dest).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn changed_destination_after_replace_is_unknown_and_never_acknowledged() {
        let dir = scratch("external");
        let dest = dir.join("a.pcnd");
        let out = save_impl(
            &dest,
            true,
            1024,
            |sink| sink.write_all(b"ours").map_err(io_error),
            |phase| {
                if phase == Phase::ReadBack {
                    fs::write(&dest, b"external")?;
                }
                Ok(())
            },
        );
        assert_eq!(out.replacement, Replacement::Unknown);
        let ctx = Context {
            operation: "save-as",
            source_state: Uuid::new_v4(),
            source_sha256: None,
            losses_acknowledged: vec![],
            destination: &dest,
        };
        let first = out.receipt_stamped(&ctx, 23, 4);
        assert_eq!(first, out.receipt_stamped(&ctx, 23, 4));
        assert_eq!(
            first.get("save_sequence").and_then(Value::as_str),
            Some("23")
        );
        assert_eq!(
            first.get("binding_generation").and_then(Value::as_str),
            Some("4")
        );
        assert_eq!(first.get("acknowledged_state_id"), Some(&Value::Null));
        assert_eq!(first.get("retry").and_then(Value::as_str), Some("inspect"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn altered_stage_is_rejected_before_replacement() {
        let dir = scratch("stage-corruption");
        let dest = dir.join("a.pcnd");
        fs::write(&dest, b"original").unwrap();
        let out = save_impl(
            &dest,
            true,
            1024,
            |sink| sink.write_all(b"encoded").map_err(io_error),
            |phase| {
                if phase == Phase::StageSync {
                    for entry in fs::read_dir(&dir)? {
                        let path = entry?.path();
                        if path.extension().is_some_and(|e| e == "tmp") {
                            fs::write(path, b"corrupt")?;
                        }
                    }
                }
                Ok(())
            },
        );
        assert_eq!(out.replacement, Replacement::NotReplaced);
        assert_eq!(fs::read(&dest).unwrap(), b"original");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_temporary_link_cleanup_cannot_report_non_replacement() {
        let dir = scratch("link-cleanup");
        let dest = dir.join("a.pcnd");
        let out = save_impl(
            &dest,
            false,
            1024,
            |sink| sink.write_all(b"new").map_err(io_error),
            |phase| {
                if phase == Phase::TemporaryCleanup {
                    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(out.replacement, Replacement::Replaced);
        assert!(
            out.is_success(),
            "replacement and durability are independent of temporary-link cleanup"
        );
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        assert!(out.reasons.iter().any(|r| r.contains("cleanup failed")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn external_changes_during_encoding_are_not_overwritten() {
        let dir = scratch("fingerprint");
        for existed in [false, true] {
            let dest = dir.join(if existed { "existing.pcnd" } else { "new.pcnd" });
            if existed {
                fs::write(&dest, b"original").unwrap();
            }
            let out = save_with(&dest, true, 1024, |sink| {
                sink.write_all(b"ours").map_err(io_error)?;
                fs::write(&dest, b"external").map_err(io_error)
            });
            assert_eq!(out.replacement, Replacement::NotReplaced);
            assert_eq!(fs::read(&dest).unwrap(), b"external");
            assert!(
                out.reasons
                    .iter()
                    .any(|r| r.contains("changed while encoding"))
            );
        }
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            2,
            "failed requests remove staging"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destinations_follow_captured_target_and_receipts_keep_its_identity() {
        use std::os::unix::fs::symlink;
        let dir = scratch("alias");
        let dest = dir.join("actual.pcnd");
        let alias = dir.join("alias.pcnd");
        fs::write(&dest, b"original").unwrap();
        symlink(&dest, &alias).unwrap();
        assert_eq!(destination_id(&alias), destination_id(&dest));
        let captured_id = destination_id(&dest);
        let out = save(&alias, b"new", true);
        assert!(out.is_success(), "{:?}", out.reasons);
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        assert!(
            fs::symlink_metadata(&alias)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let other = dir.join("other.pcnd");
        fs::write(&other, b"other").unwrap();
        fs::remove_file(&alias).unwrap();
        symlink(&other, &alias).unwrap();
        let receipt = out.receipt(&Context {
            operation: "save",
            source_state: Uuid::new_v4(),
            source_sha256: None,
            losses_acknowledged: vec![],
            destination: &alias,
        });
        assert_eq!(
            receipt.get("destination_id").and_then(Value::as_str),
            Some(captured_id.as_str())
        );
        let dangling = dir.join("dangling.pcnd");
        symlink(dir.join("absent.pcnd"), &dangling).unwrap();
        assert_eq!(
            save(&dangling, b"new", true).replacement,
            Replacement::NotReplaced
        );
        assert!(
            fs::symlink_metadata(&dangling)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn parent_aliases_share_namespace_but_hardlinks_are_separate_replacement_paths() {
        use std::os::unix::fs::symlink;
        let dir = scratch("parent-alias");
        let actual = dir.join("actual");
        fs::create_dir(&actual).unwrap();
        symlink(&actual, dir.join("alias")).unwrap();
        let direct = actual.join("new.pcnd");
        let alias = dir.join("alias/new.pcnd");
        assert_eq!(destination_id(&direct), destination_id(&alias));
        assert!(save(&alias, b"original", false).is_success());
        let hardlink = actual.join("hardlink.pcnd");
        fs::hard_link(&direct, &hardlink).unwrap();
        assert_ne!(destination_id(&direct), destination_id(&hardlink));
        assert!(save(&hardlink, b"replacement", true).is_success());
        assert_eq!(fs::read(&direct).unwrap(), b"original");
        assert_eq!(fs::read(&hardlink).unwrap(), b"replacement");
        fs::remove_dir_all(dir).unwrap();
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
