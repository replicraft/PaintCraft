//! Consent is tied to one validated isolated result, never a parent JXL decode.
use super::*;
use crate::codec_worker::jxl_integer::Report;

impl Job {
    /// Select precision support from the verified helper, never from an
    /// unconfined JPEG XL header decode. Older valid helpers keep their strict
    /// eight-bit route; a failed verification or decode is never retried.
    pub fn prepare_jxl_family(
        mut self,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        admit: impl FnOnce(Codec) -> Result<()>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if !matches!(self.codec, Codec::JxlRgba8 | Codec::JxlIntegerRgba8) {
            return Err(unsupported("JXL family selection requires a JXL job"));
        }
        let deadline = self.deadline;
        if deadline.is_zero() || deadline > Duration::from_secs(120) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid native codec deadline",
            ));
        }
        let started = Instant::now();
        let captured = self.owner;
        let check = || {
            host_check()?;
            if current_owner()? != captured {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "JXL import owner changed",
                ));
            }
            if started.elapsed() >= deadline {
                return Err(resource("JXL family preparation deadline exceeded"));
            }
            Ok(())
        };
        check()?;
        let selected = super::super::jxl_family_codec(&self.application_version, &check)?;
        admit(selected)?;
        check()?;
        self.codec = selected;
        self.deadline = deadline
            .checked_sub(started.elapsed())
            .ok_or_else(|| resource("JXL family preparation deadline exceeded"))?;
        // The ordinary path revalidates the installation before launch. This
        // deliberate baseline preserves integrity if it changed after selection;
        // it is not a decode retry or a performance-qualified cache.
        self.prepare_import(residency, lease, &check, current_owner, launch)
    }
}

pub(super) fn require_losses(report: &Report, allowed: &[String]) -> Result<()> {
    if allowed.iter().any(|code| code != crate::jxl::DEPTH16_LOSS) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Unknown integer JXL import loss consent",
        ));
    }
    if report
        .losses()
        .iter()
        .any(|loss| !allowed.iter().any(|code| code == loss))
    {
        return Err(unsupported(
            "Integer JXL import requires depth-16-reduced-to-8 consent",
        ));
    }
    Ok(())
}

impl PreparedImport {
    pub fn jxl_integer_assessment(&self) -> Option<&Report> {
        match &self.profile {
            PreparedProfile::JxlInteger(report) => Some(report),
            _ => None,
        }
    }

    /// Canonical policy/depth/statistics, source and output hashes all contribute.
    /// The private prepared value retains this same report throughout review.
    pub fn jxl_integer_assessment_digest(&self) -> Result<[u8; 32]> {
        let report = self
            .jxl_integer_assessment()
            .ok_or_else(|| unsupported("Integer JXL assessment is unavailable"))?;
        Ok(Sha256::digest(report.encode()?).into())
    }

    pub fn adopt_jxl_integer(
        self,
        expected_digest: [u8; 32],
        allowed: &[String],
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
    ) -> Result<AdoptedImport> {
        if self.jxl_integer_assessment_digest()? != expected_digest {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Integer JXL assessment changed since consent",
            ));
        }
        self.adopt_checked(allowed, host_check, current_owner)
    }
}

#[cfg(test)]
mod tests;
