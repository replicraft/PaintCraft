//! One real generated batch per fixed intent, with cached operation SHA for views.
use super::{capture::Capture7, preparation::Preparation7};
use paintcraft_commands::{Batch, FilterOp};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
};
use sha2::{Digest, Sha256};

pub(in crate::app::filter_editing) struct Operation7 {
    pub value: Value,
    pub sha256: [u8; 32],
    pub batch: Option<Batch>,
}
pub(in crate::app::filter_editing) fn capture(
    capture: &Capture7,
    prefix: &Preparation7,
) -> Result<Operation7> {
    let Some(operation) = &capture.operation else {
        prefix.spend(4)?;
        return Ok(Operation7 {
            value: Value::Null,
            sha256: Sha256::digest(b"null").into(),
            batch: None,
        });
    };
    if matches!(operation, FilterOp::Mask { .. }) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source7 numeric control cannot mutate masks",
        ));
    }
    let value = operation.to_value()?;
    // Fixed <=8KiB control serialization class. It contains no source payload,
    // title, profile, A8 data or variable document inventory scan.
    let encoded = json::canonical(&value);
    if encoded.len() > 4096 {
        return Err(bad("Source7 operation control bound"));
    }
    let ticket = capture.wire_ticket();
    let operations = vec![value.clone()];
    let generated = Value::object([
        (
            "schema",
            Value::str(Batch::schema_for_operations(&operations)),
        ),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(paintcraft_document::POLICY)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(ticket.state_id.to_string())),
                (
                    "publication_epoch",
                    Value::str(ticket.publication_epoch.to_string()),
                ),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ]);
    // At most one numerical descriptor requirement is injected by the genuine
    // generated helper: field syntax +quoted ID<=96 bytes fit128. This reserves
    // a complete8KiB structural input upper BEFORE parser/canonical batch hashing.
    if json::canonical(&generated).len() > 8192 - 128 {
        return Err(bad("Source7 generated batch bound"));
    }
    prefix.spend(8192 * 64 + 512)?;
    prefix.spend(encoded.len() as u64)?; // cached operation digest input, once
    let sha256 = Sha256::digest(&encoded).into();
    let batch = Batch::from_generated_value(generated)?;
    Ok(Operation7 {
        value,
        sha256,
        batch: Some(batch),
    })
}
fn bad(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
