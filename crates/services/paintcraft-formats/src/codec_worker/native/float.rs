//! Explicit, sealed float preparation. Admission bounds are logical, not RSS guarantees.
use super::import::PreparedProfile;
use super::*;
use crate::{float_import, precision};
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};
use paintcraft_raster::{Admission, RasterLayer};

/// Child heap once, captured raw bytes, distinct settings envelope, response and IO.
/// Metadata and resident tiles belong to separate, caller-owned budget realms.
pub fn float_reservation_bytes(length: u64) -> Result<u64> {
    if length == 0 || length > super::super::float::RAW_INPUT_BYTES_MAX as u64 {
        return Err(resource("Float source allowance"));
    }
    HEAP_BYTES
        .checked_add(length)
        .and_then(|n| n.checked_add(length))
        .and_then(|n| n.checked_add(float_import::wire::SETTINGS_BYTES as u64))
        .and_then(|n| n.checked_add(super::super::float::OUTPUT_BYTES_MAX as u64))
        .and_then(|n| n.checked_add(65536))
        .ok_or_else(|| resource("Float preparation quote overflow"))
}
/// Exact padded resident tile allocation, including extreme thin geometries.
pub fn float_resident_bytes(width: u32, height: u32) -> Result<u64> {
    let (w, h) = (u64::from(width), u64::from(height));
    if w == 0
        || h == 0
        || w > 65536
        || h > 65536
        || w.checked_mul(h)
            .is_none_or(|p| p > super::super::float::PIXELS_MAX)
    {
        return Err(resource("Float resident geometry allowance"));
    }
    w.checked_add(255)
        .and_then(|n| (n / 256).checked_mul(h.div_ceil(256)))
        .and_then(|n| n.checked_mul(RasterLayer::TILE_BYTES as u64))
        .ok_or_else(|| resource("Float resident tile extent overflow"))
}
/// Derive the worst padded geometry from the independent side/pixel limits.
/// For each width, tile demand is monotone in height, so only its maximum is needed.
pub fn float_max_resident_bytes() -> Result<u64> {
    let mut maximum = 0;
    for width in 1..=65536 {
        let height = (super::super::float::PIXELS_MAX / u64::from(width)).min(65536);
        maximum = maximum.max(float_resident_bytes(width, height as u32)?);
    }
    Ok(maximum)
}
impl Job {
    /// Caller admits the quote, metadata and frontend before launching; the lease
    /// remains owned through preview/discard/adoption. The report cannot supply
    /// source semantics: they come from the plan sealed over the captured raw bytes.
    // Keep the independent budget realms and host security callbacks explicit,
    // matching native preparation APIs; grouping them would hide required admission.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_float(
        self,
        settings: precision::Settings,
        residency: SharedBudget,
        metadata: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        admit: impl FnOnce(Codec, &float_import::SourcePlan) -> Result<()>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if !self.codec.is_float() || self.owner.frontend != 3 {
            return Err(unsupported(
                "Float preparation requires an explicit native Studio float job",
            ));
        }
        float_reservation_bytes(self.length)?;
        if self.pixels == 0
            || self.pixels > super::super::float::PIXELS_MAX
            || self.deadline.is_zero()
            || self.deadline > Duration::from_secs(120)
        {
            return Err(resource("Float job pixels/deadline allowance"));
        }
        let start = Instant::now();
        let check = || {
            host_check()?;
            if current_owner()? != self.owner {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Codec job owner changed",
                ));
            }
            if start.elapsed() >= self.deadline {
                return Err(resource("Codec job deadline exceeded"));
            }
            Ok(())
        };
        check()?;
        let source = self.identity.capture_bytes(
            self.length,
            super::super::float::RAW_INPUT_BYTES_MAX as u64,
            &check,
        )?;
        let raw_sha: [u8; 32] = Sha256::digest(&source).into();
        if let Some(expected) = &self.expected_sha256 {
            if expected.len() != 64
                || !expected
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Expected input SHA-256 must be lowercase hexadecimal",
                ));
            }
            if *expected != hex(&raw_sha) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Captured codec source digest changed",
                ));
            }
        }
        let plan = match self.codec {
            Codec::HdrSdr8 => {
                float_import::plan_hdr(&source, self.pixels, settings, metadata, &check)?
            }
            Codec::ExrSdr8 => {
                float_import::plan_exr(&source, self.pixels, settings, metadata, &check)?
            }
            _ => return Err(unsupported("Not a private float codec")),
        };
        if plan.binding().source_sha256() != raw_sha {
            return Err(invalid("Float plan source binding changed"));
        }
        let (width, height) = plan.binding().size();
        let resident_bytes = float_resident_bytes(width, height)?;
        let reservation = residency.reserve_capacity(resident_bytes)?;
        let mut admission = Admission::with_reserved_residency(self.pixels, reservation);
        admit(self.codec, &plan)?;
        check()?;
        let request = super::super::float::capture(
            self.codec,
            Arc::new(source),
            settings,
            self.owner,
            self.pixels,
        )?;
        self.identity
            .verify_bytes(&self.input, self.length, &raw_sha, &check)?;
        let record = native_float_build_record(&self.application_version)?;
        let helper = codec_helper::discover(
            codec_helper::Contract {
                application_version: &self.application_version,
                protocol_version: PROTOCOL_VERSION,
                profile: FLOAT_NATIVE_PROFILE,
                requested_heap_ceiling_bytes: HEAP_BYTES,
                qualified_import_variants: QUALIFIED_AVIF_SDR_IMPORT_VARIANTS,
                build_marker: FLOAT_BUILD_RECORD_MARKER,
                build_record: &record,
            },
            &check,
        )?;
        let executable = helper.checked_executable(&check)?;
        let remaining = self
            .deadline
            .checked_sub(start.elapsed())
            .ok_or_else(|| resource("Codec job deadline exceeded"))?;
        let output = launch::Backing::new(&request, &check)?
            .invoke(executable, &request, remaining, &check, launch)?;
        if output.len() > super::super::float::OUTPUT_BYTES_MAX {
            return Err(resource("Native float output exceeds allowance"));
        }
        helper.checked_executable(&check)?;
        self.identity
            .verify_bytes(&self.input, self.length, &raw_sha, &check)?;
        let frame = decode(&output, &request, current_owner()?)?;
        let Pixels::SrgbRgba8(rgba) = frame.pixels else {
            return Err(invalid("Float worker lacks sRGB RGBA8 output"));
        };
        let report = frame
            .float_report
            .ok_or_else(|| invalid("Float worker lacks bound report"))?;
        let assessment = plan.accept_report(report, (frame.width, frame.height), rgba, &check)?;
        let document = paintcraft_document::import_image_with_admission_checked(
            frame.width,
            frame.height,
            rgba,
            &hex(&raw_sha),
            &mut admission,
            &check,
        )?;
        helper.checked_executable(&check)?;
        self.identity
            .verify_bytes(&self.input, self.length, &raw_sha, &check)?;
        check()?;
        Ok(PreparedImport {
            document,
            input: self.input,
            identity: self.identity,
            length: self.length,
            sha256: raw_sha,
            owner: self.owner,
            profile: PreparedProfile::Float {
                codec: self.codec,
                assessment: Arc::new(assessment),
            },
            _lease: lease,
        })
    }
}
#[cfg(test)]
mod tests;
