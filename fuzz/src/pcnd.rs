//! Pure PCND1/2 read and bounded canonical roundtrip; no files or helpers.
use crate::{INPUT_MAX, PIXELS_MAX};
use paintcraft_core::types::{Error, ErrorCode, Result, resources::SharedBudget};
use paintcraft_formats::pcnd;
use sha2::{Digest as _, Sha256};
use std::io::{self, Write};
pub const STAGED_PIXELS_MAX: u64 = 131_072;
pub const METADATA_BYTES_MAX: u64 = 16 << 10;
// Two admitted tiles are at most 512 KiB raw. Source strings/headers/profile
// consume at most the 256 KiB input; 1 MiB covers their canonical framing too.
pub const OUTPUT_MAX: usize = 1 << 20;
const LAYERS_MAX: usize = 8;
fn invariant(message: &str) -> Error {
    Error::new(ErrorCode::InvariantViolation, message)
}
struct Output<'a> {
    bytes: Vec<u8>,
    check: &'a dyn Fn() -> Result<()>,
    capacity_denied: bool,
    error: Option<Error>,
}
impl Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Err(error) = (self.check)() {
            self.error = Some(error);
            return Err(io::Error::other("PCND fuzz checkpoint"));
        }
        let Some(end) = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|n| *n <= OUTPUT_MAX)
        else {
            self.capacity_denied = true;
            return Err(io::Error::other("PCND fuzz output allowance"));
        };
        if self.bytes.try_reserve(end - self.bytes.len()).is_err() {
            self.error = Some(Error::new(
                ErrorCode::ResourceLimit,
                "PCND fuzz output allocation",
            ));
            return Err(io::Error::other("PCND fuzz allocation"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Readable files can have enormous default-only sparse layers. Only canvas
/// area and actual staged tile admission use pixel budgets; never layer area.
pub(crate) fn exercise(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<()> {
    check()?;
    if bytes.len() > INPUT_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCND fuzz source allowance",
        ));
    }
    let first = SharedBudget::new(METADATA_BYTES_MAX);
    let doc = pcnd::read_with_metadata_admission_checked(
        bytes,
        PIXELS_MAX,
        STAGED_PIXELS_MAX,
        LAYERS_MAX,
        pcnd::MetadataAdmission {
            bytes: METADATA_BYTES_MAX,
            budget: &first,
        },
        check,
    )?;
    if doc.canvas.0 == 0
        || doc.canvas.1 == 0
        || u64::from(doc.canvas.0) * u64::from(doc.canvas.1) > PIXELS_MAX
        || doc.roots.len() > LAYERS_MAX
        || doc.roots.len() != doc.layers.len()
    {
        return Err(invariant(
            "PCND readable document geometry/layer invariants",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for id in &doc.roots {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .ok_or_else(|| invariant("PCND readable root missing layer"))?;
        if layer.id != *id
            || !seen.insert(*id)
            || layer.raster.width() == 0
            || layer.raster.height() == 0
            || layer.raster.width() > 1_048_576
            || layer.raster.height() > 1_048_576
        {
            return Err(invariant("PCND readable layer identity/geometry"));
        }
    }
    if first.usage().reserved > METADATA_BYTES_MAX {
        return Err(invariant("PCND source metadata admission exceeded realm"));
    }
    let mut output = Output {
        bytes: Vec::new(),
        check,
        capacity_denied: false,
        error: None,
    };
    if pcnd::write_to(&doc, &mut output).is_err() {
        if let Some(error) = output.error {
            return Err(error);
        }
        if output.capacity_denied {
            return Err(invariant(
                "Admitted PCND exceeded its proven raw output bound",
            ));
        }
        return Err(invariant(
            "Readable PCND document failed canonical raw serialization",
        ));
    }
    check()?;
    // Two independently bounded realms avoid charging two retained 16KiB
    // attachments to one 16KiB budget while comparing both immutable snapshots.
    let second = SharedBudget::new(METADATA_BYTES_MAX);
    let restored = pcnd::read_with_metadata_admission_checked(
        &output.bytes,
        PIXELS_MAX,
        STAGED_PIXELS_MAX,
        LAYERS_MAX,
        pcnd::MetadataAdmission {
            bytes: METADATA_BYTES_MAX,
            budget: &second,
        },
        check,
    )
    .map_err(|error| {
        if matches!(
            error.code,
            ErrorCode::OperationCancelled | ErrorCode::ResourceLimit
        ) {
            error
        } else {
            invariant("Canonical PCND output failed its public reader")
        }
    })?;
    if !doc.same_semantics(&restored) || doc.state_id != restored.state_id {
        return Err(invariant(
            "PCND canonical roundtrip changed document/source metadata semantics",
        ));
    }
    if second.usage().reserved > METADATA_BYTES_MAX {
        return Err(invariant(
            "PCND roundtrip metadata admission exceeded realm",
        ));
    }
    drop(restored);
    drop(doc);
    if first.usage().reserved != 0 || second.usage().reserved != 0 {
        return Err(invariant(
            "PCND attachment lease survived final document drop",
        ));
    }
    check()
}
#[cfg(test)]
mod tests;

/// No source-file mutation: one bounded fallible copy for the fuzz wrapper only.
pub(crate) fn repair_outer_sha(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    if bytes.len() > INPUT_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCND fuzz source allowance",
        ));
    }
    let body_len = bytes.len().checked_sub(32).ok_or_else(|| {
        Error::new(
            ErrorCode::CodecFailed,
            "PCND checksum-aware source too short",
        )
    })?;
    let body = &bytes[..body_len];
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len()).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "PCND checksum-aware copy allocation",
        )
    })?;
    let mut hash = Sha256::new();
    for chunk in body.chunks(65536) {
        check()?;
        hash.update(chunk);
        copy.extend_from_slice(chunk);
    }
    check()?;
    copy.extend_from_slice(&hash.finalize());
    Ok(copy)
}
