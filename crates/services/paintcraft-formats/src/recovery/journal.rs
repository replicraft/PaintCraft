//! Bounded checksummed records for the private recovery store. A journal segment
//! contains one full-snapshot commit; head records anchor a validated position.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
};
use paintcraft_document::{Document, hex};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(super) const RECORD_MAX: u64 = 128 * 1024;
pub const SNAPSHOT_BYTES_MAX: u64 = 1 << 30;
pub(super) const TAIL_RECORDS_MAX: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Commit {
    pub sequence: u64,
    pub previous: Option<String>,
    pub payload: Uuid,
    pub bytes: u64,
    pub sha256: String,
    pub document: Uuid,
    pub state: Uuid,
    pub original: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Head {
    pub sequence: u64,
    pub commit_sha256: String,
}
pub(super) fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
pub(super) fn hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn object(value: &Value, keys: &[&str]) -> Result<()> {
    let Value::Object(fields) = value else {
        return Err(invalid("recovery record must be an object"));
    };
    if fields.len() != keys.len() || keys.iter().any(|key| !fields.contains_key(*key)) {
        return Err(invalid("unsupported recovery record fields"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("recovery field must be text"))
}
fn number(value: &Value, key: &str) -> Result<u64> {
    let raw = text(value, key)?;
    let number = raw
        .parse::<u64>()
        .map_err(|_| invalid("invalid recovery integer"))?;
    if raw != number.to_string() || number == 0 {
        return Err(invalid("noncanonical recovery integer"));
    }
    Ok(number)
}
fn digest(raw: &str) -> Result<String> {
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("invalid recovery digest"));
    }
    Ok(raw.to_owned())
}
fn uuid(value: &Value, key: &str) -> Result<Uuid> {
    let raw = text(value, key)?;
    let id = Uuid::parse_str(raw).map_err(|_| invalid("invalid recovery UUID"))?;
    if id.to_string() != raw {
        return Err(invalid("noncanonical recovery UUID"));
    }
    Ok(id)
}
fn envelope(schema: &str, body: Value) -> Vec<u8> {
    let checksum = hash(&json::canonical(&body));
    json::canonical(&Value::object([
        ("schema", Value::str(schema)),
        ("body", body),
        ("sha256", Value::str(checksum)),
    ]))
}
fn decode(bytes: &[u8], schema: &str) -> Result<Value> {
    if bytes.len() as u64 > RECORD_MAX {
        return Err(invalid("recovery record exceeds its limit"));
    }
    let value = json::parse(bytes).map_err(|_| invalid("invalid recovery JSON"))?;
    object(&value, &["schema", "body", "sha256"])?;
    if text(&value, "schema")? != schema {
        return Err(invalid("unsupported recovery record schema"));
    }
    let body = value.get("body").expect("checked field");
    if hash(&json::canonical(body)) != digest(text(&value, "sha256")?)? {
        return Err(invalid("recovery record checksum mismatch"));
    }
    Ok(body.clone())
}
impl Commit {
    pub fn encode(&self) -> Vec<u8> {
        envelope(
            "paintcraft.recovery-commit/v1",
            Value::object([
                ("sequence", Value::str(self.sequence.to_string())),
                (
                    "previous_sha256",
                    self.previous.as_ref().map_or(Value::Null, Value::str),
                ),
                ("payload", Value::str(self.payload.to_string())),
                ("payload_bytes", Value::str(self.bytes.to_string())),
                ("payload_sha256", Value::str(&self.sha256)),
                ("document_id", Value::str(self.document.to_string())),
                ("state_id", Value::str(self.state.to_string())),
                (
                    "original",
                    self.original.as_ref().map_or(Value::Null, Value::str),
                ),
            ]),
        )
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let body = decode(bytes, "paintcraft.recovery-commit/v1")?;
        object(
            &body,
            &[
                "sequence",
                "previous_sha256",
                "payload",
                "payload_bytes",
                "payload_sha256",
                "document_id",
                "state_id",
                "original",
            ],
        )?;
        let sequence = number(&body, "sequence")?;
        let previous = match body.get("previous_sha256") {
            Some(Value::Null) if sequence == 1 => None,
            Some(Value::Str(raw)) if sequence > 1 => Some(digest(raw)?),
            _ => return Err(invalid("invalid recovery predecessor")),
        };
        let bytes = number(&body, "payload_bytes")?;
        if bytes > SNAPSHOT_BYTES_MAX {
            return Err(invalid("recovery snapshot exceeds its limit"));
        }
        let original = match body.get("original") {
            Some(Value::Null) => None,
            Some(Value::Str(raw)) if raw.len() <= 32 * 1024 => Some(raw.clone()),
            _ => return Err(invalid("invalid recovery original path")),
        };
        Ok(Self {
            sequence,
            previous,
            payload: uuid(&body, "payload")?,
            bytes,
            sha256: digest(text(&body, "payload_sha256")?)?,
            document: uuid(&body, "document_id")?,
            state: uuid(&body, "state_id")?,
            original,
        })
    }
    #[cfg(test)]
    pub fn validate_payload(&self, dir: &Path, session: Uuid) -> Result<Document> {
        super::admission::native(|metadata| {
            self.validate_payload_checked(
                dir,
                session,
                &super::admission::Context::new(metadata, &|| Ok(())),
            )
        })
    }
    pub(super) fn validate_payload_checked(
        &self,
        dir: &Path,
        session: Uuid,
        context: &super::admission::Context<'_>,
    ) -> Result<Document> {
        let bytes = context.file(&payload_path(dir, session, self.payload), self.bytes)?;
        if bytes.len() as u64 != self.bytes || hash(&bytes) != self.sha256 {
            return Err(invalid("recovery payload checksum or length mismatch"));
        }
        let document = context.read(&bytes)?;
        if document.document_id != self.document || document.state_id != self.state {
            return Err(invalid("recovery snapshot identity mismatch"));
        }
        Ok(document)
    }
}
impl Head {
    pub fn encode(&self) -> Vec<u8> {
        envelope(
            "paintcraft.recovery-head/v1",
            Value::object([
                ("sequence", Value::str(self.sequence.to_string())),
                ("commit_sha256", Value::str(&self.commit_sha256)),
            ]),
        )
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let body = decode(bytes, "paintcraft.recovery-head/v1")?;
        object(&body, &["sequence", "commit_sha256"])?;
        Ok(Self {
            sequence: number(&body, "sequence")?,
            commit_sha256: digest(text(&body, "commit_sha256")?)?,
        })
    }
}
/// Check both metadata and observed bytes; refuse symlinks and nonregular files.
pub(super) fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| super::storage("cannot inspect recovery file", &e))?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(invalid("invalid recovery file type or size"));
    }
    let mut data = Vec::new();
    File::open(path)
        .and_then(|f| f.take(limit.saturating_add(1)).read_to_end(&mut data))
        .map_err(|e| super::storage("cannot read recovery file", &e))?;
    if data.len() as u64 > limit {
        return Err(invalid("recovery file grew beyond its limit"));
    }
    Ok(data)
}
pub(super) fn commit_path(dir: &Path, session: Uuid, sequence: u64) -> PathBuf {
    dir.join(format!("{session}.journal-{sequence:020}.json"))
}
pub(super) fn payload_path(dir: &Path, session: Uuid, payload: Uuid) -> PathBuf {
    dir.join(format!("{session}.payload-{payload}.pcnd"))
}
pub(super) fn head_path(dir: &Path, session: Uuid, slot: u64) -> PathBuf {
    dir.join(format!("{session}.head-{slot}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> Commit {
        Commit {
            sequence: 1,
            previous: None,
            payload: Uuid::new_v4(),
            bytes: 10,
            sha256: "a".repeat(64),
            document: Uuid::new_v4(),
            state: Uuid::new_v4(),
            original: Some("source.pcnd".into()),
        }
    }
    #[test]
    fn checked_records_round_trip_and_detect_corruption_and_torn_tails() {
        let record = record();
        let bytes = record.encode();
        assert_eq!(Commit::decode(&bytes).unwrap(), record);
        for end in 0..bytes.len() {
            assert!(Commit::decode(&bytes[..end]).is_err());
        }
        let mut corrupt = bytes.clone();
        let at = corrupt
            .windows(11)
            .position(|w| w == b"source.pcnd")
            .unwrap();
        corrupt[at] = b'S';
        assert!(Commit::decode(&corrupt).is_err());
        let head = Head {
            sequence: 1,
            commit_sha256: hash(&bytes),
        };
        assert_eq!(Head::decode(&head.encode()).unwrap(), head);
    }
    #[test]
    fn record_semantics_reject_unbacked_identity_and_unbounded_sizes() {
        let mut record = record();
        record.sequence = 2;
        assert!(
            Commit::decode(&record.encode()).is_err(),
            "missing predecessor"
        );
        record.previous = Some("A".repeat(64));
        assert!(
            Commit::decode(&record.encode()).is_err(),
            "noncanonical digest"
        );
        record.previous = Some("b".repeat(64));
        record.bytes = SNAPSHOT_BYTES_MAX + 1;
        assert!(
            Commit::decode(&record.encode()).is_err(),
            "oversize payload"
        );
        assert!(Head::decode(&vec![b' '; RECORD_MAX as usize + 1]).is_err());
    }
}
