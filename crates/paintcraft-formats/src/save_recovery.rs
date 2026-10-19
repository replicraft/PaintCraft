//! Write-ahead snapshots and diagnostic receipts for native saves. Recovery
//! admission and checksummed payloads are shared with the normal recovery store.
//! Receipts read from disk never authorize replacement or a new session binding.
use crate::{
    recovery::{Found, Recovery},
    save,
};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
};
use paintcraft_document::Document;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read as _,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(crate) const DIRECTORY: &str = "saved-attempts";
const RECORD_MAX: u64 = 256 * 1024;
const SNAPSHOT_MAX: u64 = 1 << 30;
const DISK_MAX: u64 = 4 << 30;
const METADATA_RESERVE: u64 = 64 << 20;
const OWNERS_MAX: usize = 64;
pub(crate) const FILES_MAX: usize = 1024;

#[derive(Clone, Debug)]
pub struct Stamp {
    pub session: Uuid,
    pub sequence: u64,
    pub generation: u64,
    pub operation: &'static str,
}
#[derive(Debug)]
pub struct SavedAttempt {
    recovery: Recovery,
    directory: PathBuf,
    stamp: Stamp,
    source: Uuid,
    document: Uuid,
    requested: PathBuf,
}

impl SavedAttempt {
    /// A full durable snapshot precedes staging/replacement of the user's file.
    /// Failure refuses the user-visible write; useful partial data is retained.
    pub fn capture(
        root: &Path,
        document: &Document,
        requested: &Path,
        stamp: Stamp,
    ) -> Result<Self> {
        validate_stamp(&stamp)?;
        let directory = root.join(DIRECTORY);
        crate::recovery::ensure_directory(root)?;
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err(invalid("invalid saved-attempt recovery directory")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&directory).map_err(storage)?
            }
            Err(error) => return Err(storage(error)),
        }
        // Serialize cooperating admissions across windows. No arbitrary external
        // writer exclusion or network-filesystem durability is claimed.
        let lock_path = directory.join(".admission.lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.is_file()) {
            return Err(invalid("invalid saved-attempt admission lock"));
        }
        let admission = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(storage)?;
        admission.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => {
                limited("saved-attempt recovery admission is busy")
            }
            other => Error::new(
                ErrorCode::StorageFailed,
                format!("saved-attempt admission locking failed: {other}"),
            ),
        })?;
        admit(&directory)?;
        let recovery = Recovery::start(&directory)?;
        // Sync the parent as well: a durable child entry needs a durable directory.
        File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(storage)?;
        let retained = Self {
            recovery,
            directory,
            stamp,
            source: document.state_id,
            document: document.document_id,
            requested: requested.to_owned(),
        };
        retained.record("capturing", None)?;
        retained
            .recovery
            .checkpoint_document(document, Some(requested))?;
        retained.record("captured", None)?;
        drop(admission);
        Ok(retained)
    }
    /// The prepared receipt deliberately says unknown: a crash after publication
    /// may occur on either side of replacement, before a final receipt is durable.
    pub fn prepare(&self, outcome: &save::Outcome) -> Result<()> {
        let mut uncertain = outcome.clone();
        uncertain.replacement = save::Replacement::Unknown;
        uncertain.durable = false;
        self.record("prepared", Some(&uncertain))
    }
    pub fn finish(&self, outcome: &save::Outcome) -> Result<()> {
        self.record("finished", Some(outcome))
    }
    pub fn discard(&self) -> Result<()> {
        self.recovery.clear()
    }
    pub fn session(&self) -> Uuid {
        self.recovery.session()
    }
    fn record(&self, phase: &str, outcome: Option<&save::Outcome>) -> Result<()> {
        let receipt = outcome.map_or(Value::Null, |outcome| {
            outcome.receipt_stamped(
                &save::Context {
                    operation: self.stamp.operation,
                    source_state: self.source,
                    source_sha256: None,
                    losses_acknowledged: vec![],
                    destination: &self.requested,
                },
                self.stamp.sequence,
                self.stamp.generation,
            )
        });
        let body = Value::object([
            ("owner", Value::str(self.recovery.session().to_string())),
            ("editor_session", Value::str(self.stamp.session.to_string())),
            ("document_id", Value::str(self.document.to_string())),
            ("source_state_id", Value::str(self.source.to_string())),
            ("save_sequence", Value::str(self.stamp.sequence.to_string())),
            (
                "binding_generation",
                Value::str(self.stamp.generation.to_string()),
            ),
            ("operation", Value::str(self.stamp.operation)),
            ("phase", Value::str(phase)),
            ("receipt", receipt),
        ]);
        let bytes = json::canonical(&Value::object([
            ("schema", Value::str("paintcraft.saved-attempt/v1")),
            ("sha256", Value::str(hash(&json::canonical(&body)))),
            ("body", body),
        ]));
        if bytes.len() as u64 > RECORD_MAX {
            return Err(limited("saved-attempt record exceeds its limit"));
        }
        let written = save::save(&record_path(&self.directory, self.session()), &bytes, true);
        if !written.is_success() {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                format!(
                    "saved-attempt evidence was not durable: {}",
                    written.reasons.join("; ")
                ),
            ));
        }
        Ok(())
    }
}

/// Diagnostic evidence only. Call while holding the recovery source claim when
/// using the evidence in a restore flow; this function does not acquire authority.
pub fn read_record(found: &Found) -> Result<Value> {
    let directory = found
        .checkpoint
        .parent()
        .ok_or_else(|| invalid("missing saved-attempt directory"))?;
    let path = record_path(directory, found.session);
    if !fs::symlink_metadata(&path).map_err(storage)?.is_file() {
        return Err(invalid("invalid saved-attempt record type"));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(storage)?
        .take(RECORD_MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(storage)?;
    if bytes.len() as u64 > RECORD_MAX {
        return Err(limited("saved-attempt record exceeds its limit"));
    }
    let value = json::parse(&bytes).map_err(|_| invalid("invalid saved-attempt JSON"))?;
    object(&value, &["schema", "sha256", "body"])?;
    let body = value
        .get("body")
        .ok_or_else(|| invalid("missing saved-attempt body"))?;
    if value.get("schema").and_then(Value::as_str) != Some("paintcraft.saved-attempt/v1")
        || value.get("sha256").and_then(Value::as_str)
            != Some(hash(&json::canonical(body)).as_str())
        || body.get("owner").and_then(Value::as_str) != Some(found.session.to_string().as_str())
        || body.get("source_state_id").and_then(Value::as_str)
            != Some(found.state_id.to_string().as_str())
        || body.get("document_id").and_then(Value::as_str)
            != Some(found.document_id.to_string().as_str())
    {
        return Err(invalid("saved-attempt evidence identity/checksum mismatch"));
    }
    validate_body(body)?;
    Ok(body.clone())
}

fn object(value: &Value, keys: &[&str]) -> Result<()> {
    let Value::Object(fields) = value else {
        return Err(invalid("saved-attempt object required"));
    };
    if fields.len() != keys.len() || keys.iter().any(|key| !fields.contains_key(*key)) {
        return Err(invalid("unsupported saved-attempt fields"));
    }
    Ok(())
}
fn validate_body(body: &Value) -> Result<()> {
    object(
        body,
        &[
            "owner",
            "editor_session",
            "document_id",
            "source_state_id",
            "save_sequence",
            "binding_generation",
            "operation",
            "phase",
            "receipt",
        ],
    )?;
    for key in ["owner", "editor_session", "document_id", "source_state_id"] {
        let raw = body
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("missing save identity"))?;
        if !Uuid::parse_str(raw).is_ok_and(|id| id.to_string() == raw) {
            return Err(invalid("invalid save identity"));
        }
    }
    for key in ["save_sequence", "binding_generation"] {
        let raw = body
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("missing save ordinal"))?;
        if !raw
            .parse::<u64>()
            .is_ok_and(|number| number.to_string() == raw && (key != "save_sequence" || number > 0))
        {
            return Err(invalid("invalid save ordinal"));
        }
    }
    if !matches!(
        body.get("operation").and_then(Value::as_str),
        Some("save" | "save-as")
    ) {
        return Err(invalid("unsupported native save operation"));
    }
    let phase = body.get("phase").and_then(Value::as_str);
    let receipt = body.get("receipt").expect("checked receipt");
    if matches!(phase, Some("capturing" | "captured")) {
        if *receipt != Value::Null {
            return Err(invalid("premature save receipt"));
        }
    } else if matches!(phase, Some("prepared" | "finished")) {
        if receipt.get("schema").and_then(Value::as_str) != Some("paintcraft.file-operation/v1") {
            return Err(invalid("unsupported save receipt"));
        }
        for key in [
            "operation",
            "source_state_id",
            "save_sequence",
            "binding_generation",
        ] {
            if receipt.get(key) != body.get(key) {
                return Err(invalid("save receipt stamp mismatch"));
            }
        }
        let id = receipt
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("missing operation identity"))?;
        if !Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id) {
            return Err(invalid("invalid operation identity"));
        }
        if phase == Some("prepared")
            && (receipt.get("replacement") != Some(&Value::str("unknown"))
                || receipt.get("acknowledged_state_id") != Some(&Value::Null))
        {
            return Err(invalid("prepared receipt cannot acknowledge a replacement"));
        }
    } else {
        return Err(invalid("unsupported save retention phase"));
    }
    Ok(())
}

fn record_path(directory: &Path, session: Uuid) -> PathBuf {
    directory.join(format!("{session}.save.json"))
}
fn hash(bytes: &[u8]) -> String {
    paintcraft_document::hex(&Sha256::digest(bytes))
}
fn storage(error: std::io::Error) -> Error {
    Error::new(
        ErrorCode::StorageFailed,
        format!("saved-attempt storage failed: {}", error.kind()),
    )
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn limited(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn validate_stamp(stamp: &Stamp) -> Result<()> {
    if stamp.sequence == 0 || !matches!(stamp.operation, "save" | "save-as") {
        return Err(invalid("invalid native save stamp"));
    }
    Ok(())
}
fn admit(directory: &Path) -> Result<()> {
    let mut bytes = 0u64;
    let mut owners = 0usize;
    for (count, entry) in fs::read_dir(directory).map_err(storage)?.enumerate() {
        if count >= FILES_MAX - 16 {
            return Err(limited("saved-attempt file count limit reached"));
        }
        let entry = entry.map_err(storage)?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(storage)?;
        if !metadata.is_file() {
            return Err(invalid("non-file in saved-attempt store; retained"));
        }
        bytes = bytes
            .checked_add(metadata.len())
            .ok_or_else(|| limited("saved-attempt disk charge overflows"))?;
        if entry
            .file_name()
            .to_str()
            .and_then(|s| s.strip_suffix(".lock"))
            .is_some_and(|s| Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s))
        {
            owners += 1;
        }
    }
    // Reserve a whole maximum snapshot and metadata slack before writing. This
    // intentionally conservative baseline has no measured disk/performance claim.
    if owners >= OWNERS_MAX || bytes > DISK_MAX - SNAPSHOT_MAX - METADATA_RESERVE {
        return Err(limited(
            "resolve retained saved attempts before saving more",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
