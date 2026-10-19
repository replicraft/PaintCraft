//! Path-free persistence evidence. Reacquisition always uses a caller-authorized
//! path and checks its resolved destination ID; stored data supplies no path.
use super::*;

impl Outcome {
    pub fn reconciliation_evidence(&self) -> Value {
        self.attempt.as_ref().map_or(Value::Null, |attempt| {
            Value::object([
                ("schema", Value::str("paintcraft.save-evidence/v1")),
                ("operation_id", Value::str(self.operation_id.to_string())),
                ("destination_id", Value::str(path_id(&attempt.destination))),
                (
                    "original",
                    attempt
                        .original
                        .as_ref()
                        .map_or(Value::Null, fingerprint_value),
                ),
                ("staged", fingerprint_value(&attempt.staged)),
                (
                    "replacement_acknowledged",
                    Value::Bool(attempt.replacement_acknowledged),
                ),
                (
                    "original_reason_count",
                    Value::str(
                        self.reconciliation_notes
                            .unwrap_or(self.reasons.len())
                            .to_string(),
                    ),
                ),
            ])
        })
    }

    /// Resume an incomplete attempt at an explicitly reacquired destination.
    /// The host must not adopt this previous-session outcome into a new editor.
    /// Snapshot identity/bytes are validated by the format owner before this call.
    pub fn reacquire(
        receipt: &Value,
        evidence: Option<&Value>,
        destination: &Path,
        snapshot_sha256: &str,
        snapshot_bytes: u64,
    ) -> Result<Self, Error> {
        object(
            receipt,
            &[
                "schema",
                "operation_id",
                "operation",
                "source_state_id",
                "source_sha256",
                "destination_id",
                "save_sequence",
                "binding_generation",
                "replacement",
                "requested_durability",
                "durability",
                "staged_sha256",
                "observed_destination_sha256",
                "acknowledged_state_id",
                "source_lineage_adopted",
                "retry",
                "losses_acknowledged",
                "reasons",
                "synthetic",
            ],
        )?;
        if snapshot_bytes > STAGED_FILE_BYTES_MAX || !is_digest(snapshot_sha256) {
            return Err(invalid("invalid captured snapshot fingerprint"));
        }
        let replacement = match text(receipt, "replacement")? {
            "unknown" => Replacement::Unknown,
            "replaced" if text(receipt, "durability")? == "not-confirmed" => Replacement::Replaced,
            _ => {
                return Err(invalid(
                    "only incomplete replacement attempts can be reacquired",
                ));
            }
        };
        if text(receipt, "schema")? != "paintcraft.file-operation/v1"
            || !matches!(text(receipt, "operation")?, "save" | "save-as")
            || text(receipt, "requested_durability")? != "durable"
            || text(receipt, "durability")? != "not-confirmed"
            || receipt.get("acknowledged_state_id") != Some(&Value::Null)
            || text(receipt, "staged_sha256")? != snapshot_sha256
        {
            return Err(invalid("inconsistent captured save receipt"));
        }
        if receipt.get("losses_acknowledged") != Some(&Value::Array(Vec::new()))
            || receipt.get("source_lineage_adopted") != Some(&Value::Bool(false))
            || receipt.get("synthetic") != Some(&Value::Bool(false))
        {
            return Err(invalid(
                "unsupported native save receipt provenance/loss plan",
            ));
        }
        for key in ["source_sha256", "observed_destination_sha256"] {
            match receipt.get(key) {
                Some(Value::Null) => {}
                Some(Value::Str(value)) if is_digest(value) => {}
                _ => return Err(invalid("invalid captured digest")),
            }
        }
        let retry = if replacement == Replacement::Unknown {
            "inspect"
        } else {
            "no-automatic-retry"
        };
        if receipt.get("retry").and_then(Value::as_str) != Some(retry) {
            return Err(invalid("inconsistent receipt retry disposition"));
        }
        let operation_id = uuid(receipt, "operation_id")?;
        uuid(receipt, "source_state_id")?;
        for key in ["save_sequence", "binding_generation"] {
            number(receipt, key)?;
        }
        let expected = Fingerprint {
            bytes: snapshot_bytes,
            hash: snapshot_sha256.to_owned(),
        };
        let (original, acknowledged) =
            if let Some(evidence) = evidence.filter(|v| **v != Value::Null) {
                object(
                    evidence,
                    &[
                        "schema",
                        "operation_id",
                        "destination_id",
                        "original",
                        "staged",
                        "replacement_acknowledged",
                        "original_reason_count",
                    ],
                )?;
                if text(evidence, "schema")? != "paintcraft.save-evidence/v1"
                    || evidence.get("operation_id") != receipt.get("operation_id")
                    || evidence.get("destination_id") != receipt.get("destination_id")
                    || fingerprint(evidence.get("staged").expect("checked field"))? != expected
                {
                    return Err(invalid(
                        "captured save evidence does not match the snapshot/receipt",
                    ));
                }
                let original = match evidence.get("original").expect("checked field") {
                    Value::Null => None,
                    value => Some(fingerprint(value)?),
                };
                let acknowledged = evidence
                    .get("replacement_acknowledged")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("missing replacement evidence"))?;
                if replacement == Replacement::Replaced && !acknowledged {
                    return Err(invalid("replacement receipt lacks positive evidence"));
                }
                (original, acknowledged)
            } else {
                // v1 saved-attempt records retain a public receipt and snapshot only.
                // Unknown never acquires positive syscall evidence from matching bytes.
                (None, replacement == Replacement::Replaced)
            };
        let resolved = resolve_destination(destination).map_err(io_error)?;
        let captured_id = text(receipt, "destination_id")?;
        if path_id(&resolved) != captured_id {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "selected file is not the captured save destination",
            ));
        }
        let raw_reasons = receipt
            .get("reasons")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("missing receipt reasons"))?;
        if raw_reasons.len() > 128 {
            return Err(invalid("too many receipt reasons"));
        }
        let reasons = raw_reasons
            .iter()
            .map(|v| {
                v.as_str()
                    .filter(|s| s.len() <= 32768)
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("invalid receipt reason"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let reason_count = evidence
            .filter(|v| **v != Value::Null)
            .map(|value| number(value, "original_reason_count"))
            .transpose()?
            .unwrap_or(reasons.len() as u64);
        if reason_count > 64 || reason_count > reasons.len() as u64 {
            return Err(invalid("invalid original reason count"));
        }
        Ok(Self {
            operation_id,
            destination_id: Some(captured_id.to_owned()),
            attempt: Some(Attempt {
                destination: resolved,
                original,
                staged: expected.clone(),
                replacement_acknowledged: acknowledged,
            }),
            reconciliation_notes: Some(reason_count as usize),
            replacement,
            durable: false,
            staged_sha256: Some(expected.hash),
            observed_sha256: None,
            reasons,
        })
    }
}
fn fingerprint_value(value: &Fingerprint) -> Value {
    Value::object([
        ("bytes", Value::str(value.bytes.to_string())),
        ("sha256", Value::str(&value.hash)),
    ])
}
fn fingerprint(value: &Value) -> Result<Fingerprint, Error> {
    object(value, &["bytes", "sha256"])?;
    let bytes = number(value, "bytes")?;
    let hash = text(value, "sha256")?;
    if bytes > STAGED_FILE_BYTES_MAX || !is_digest(hash) {
        return Err(invalid("invalid persisted fingerprint"));
    }
    Ok(Fingerprint {
        bytes,
        hash: hash.to_owned(),
    })
}
fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn object(value: &Value, keys: &[&str]) -> Result<(), Error> {
    let Value::Object(fields) = value else {
        return Err(invalid("save evidence object required"));
    };
    if fields.len() != keys.len() || keys.iter().any(|key| !fields.contains_key(*key)) {
        return Err(invalid("unsupported save evidence fields"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("missing save evidence string"))
}
fn number(value: &Value, key: &str) -> Result<u64, Error> {
    let raw = text(value, key)?;
    let number = raw
        .parse::<u64>()
        .map_err(|_| invalid("invalid save evidence integer"))?;
    if number.to_string() != raw || (key == "save_sequence" && number == 0) {
        return Err(invalid("noncanonical save evidence integer"));
    }
    Ok(number)
}
fn uuid(value: &Value, key: &str) -> Result<Uuid, Error> {
    let raw = text(value, key)?;
    let uuid = Uuid::parse_str(raw).map_err(|_| invalid("invalid save identity"))?;
    if uuid.to_string() != raw {
        return Err(invalid("noncanonical save identity"));
    }
    Ok(uuid)
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}

#[cfg(all(test, unix))]
mod tests;
