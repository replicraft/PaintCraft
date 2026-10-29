//! Consent is tied to one validated isolated result, never a parent JXL decode.
use super::*;
use crate::codec_worker::jxl_integer::Report;

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
