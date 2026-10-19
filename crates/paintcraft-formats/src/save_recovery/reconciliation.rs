//! Reacquired destination inspection under an exact snapshot source claim.
use super::*;

#[derive(Debug)]
pub struct ReconciledSavedAttempt {
    pub observation: save::Observation,
    pub receipt: Value,
    pub destination: PathBuf,
}

/// The selected path comes from the host/user, never from persisted diagnostics.
/// This reads or syncs that destination; it never replaces output or adopts a binding.
pub fn reconcile(
    found: &Found,
    selected: &Path,
    mode: save::ReconcileMode,
) -> Result<ReconciledSavedAttempt> {
    let claim = Recovery::claim(found)?;
    let mut body = read_record(found)?;
    let receipt = body
        .get("receipt")
        .ok_or_else(|| invalid("no saved receipt to reconcile"))?;
    let outcome = save::Outcome::reacquire(
        receipt,
        body.get("evidence"),
        selected,
        &found.sha256,
        claim.snapshot_bytes,
    )?;
    let checked = outcome.reconcile(mode)?;
    let operation = match body.get("operation").and_then(Value::as_str) {
        Some("save") => "save",
        Some("save-as") => "save-as",
        _ => return Err(invalid("unsupported native save operation")),
    };
    let number = |key| {
        body.get(key)
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| invalid("invalid persisted save ordinal"))
    };
    let receipt = checked.outcome.receipt_stamped(
        &save::Context {
            operation,
            source_state: found.state_id,
            source_sha256: receipt
                .get("source_sha256")
                .and_then(Value::as_str)
                .map(str::to_owned),
            losses_acknowledged: vec![],
            destination: selected,
        },
        number("save_sequence")?,
        number("binding_generation")?,
    );
    let destination = checked
        .outcome
        .resolved_destination()
        .expect("reacquired namespace")
        .to_path_buf();
    let Value::Object(fields) = &mut body else {
        unreachable!("validated body")
    };
    fields.insert("phase".into(), Value::str("finished"));
    fields.insert("receipt".into(), receipt.clone());
    fields.insert("evidence".into(), checked.outcome.reconciliation_evidence());
    write_record(
        found.checkpoint.parent().expect("claimed directory"),
        found.session,
        &body,
    )?;
    drop(claim); // No document publication or snapshot retirement on inspection.
    Ok(ReconciledSavedAttempt {
        observation: checked.observation,
        receipt,
        destination,
    })
}

pub(super) fn write_record(directory: &Path, owner: Uuid, body: &Value) -> Result<()> {
    validate_body(body, true)?;
    let bytes = json::canonical(&Value::object([
        ("schema", Value::str("paintcraft.saved-attempt/v2")),
        ("sha256", Value::str(hash(&json::canonical(body)))),
        ("body", body.clone()),
    ]));
    if bytes.len() as u64 > RECORD_MAX {
        return Err(limited("saved-attempt record exceeds its limit"));
    }
    let path = record_path(directory, owner);
    if fs::symlink_metadata(&path)
        .is_ok_and(|metadata| !metadata.is_file() || metadata.len() > RECORD_MAX)
    {
        return Err(invalid("invalid existing saved-attempt record; retained"));
    }
    let written = save::save(&path, &bytes, true);
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

#[cfg(all(test, unix))]
mod tests;
