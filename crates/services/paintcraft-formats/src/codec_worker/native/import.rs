//! Resident adoption after bounded isolated decoding, never public read admission.
use super::*;
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};
use paintcraft_document::Document;
use paintcraft_raster::Admission;

#[derive(Debug)]
pub struct PreparedImport {
    document: Document,
    input: PathBuf,
    identity: SourceIdentity,
    length: u64,
    sha256: [u8; 32],
    owner: Owner,
    profile: PreparedProfile,
    // The dead helper/source/output charge stays held until adoption or discard.
    // Document drops first on stale/error, before this final preparation charge.
    _lease: ResourceLease,
}
/// Adopted tile leases are intrinsic to the document and all shared snapshots;
/// this result does not retain the dead child's 256MiB preparation reservation.
#[derive(Debug)]
pub struct AdoptedImport {
    pub document: Document,
    pub input: PathBuf,
    pub identity: SourceIdentity,
    pub sha256: [u8; 32],
    pub jxr_assessment: Option<crate::jxr::ImportAssessment>,
}
#[derive(Debug)]
enum PreparedProfile {
    Jxl,
    Jxr(crate::jxr::ImportAssessment),
}
impl PreparedProfile {
    fn jxr(&self) -> Option<&crate::jxr::ImportAssessment> {
        match self {
            Self::Jxl => None,
            Self::Jxr(assessment) => Some(assessment),
        }
    }
    fn losses(&self) -> &'static [&'static str] {
        self.jxr().map_or(&[], |assessment| assessment.losses())
    }
    fn require_losses(&self, allowed: &[String]) -> Result<()> {
        if let Self::Jxr(assessment) = self {
            assessment.require_losses(allowed)?;
        }
        Ok(())
    }
}
impl Job {
    /// Caller must use the existing host resident realm and a sufficiently sized
    /// admitted preparation lease. Only this profile has assigned sRGB semantics;
    /// native HEIF gray planes cannot pass through this document conversion.
    pub fn prepare_jxl(
        self,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if self.codec != Codec::JxlRgba8 {
            return Err(unsupported(
                "Document adoption requires qualified straight sRGB JXL samples",
            ));
        }
        self.prepare_import(residency, lease, host_check, current_owner, launch)
    }
    /// Prepare only the assigned sRGB JXL or strict default-sRGB RGB8 JXR
    /// profile. Native HEIF gray/color planes cannot enter this conversion.
    pub fn prepare_import(
        self,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if !matches!(self.codec, Codec::JxlRgba8 | Codec::JxrRgb8) {
            return Err(unsupported(
                "Native samples have no qualified document color interpretation",
            ));
        }
        let input = self.input.clone();
        let identity = self.identity.clone();
        let length = self.length;
        let owner = self.owner;
        let (document, sha256, profile) = self.consume(
            host_check,
            current_owner,
            launch,
            |request, frame, _, check| {
                let Pixels::SrgbRgba8(rgba) = frame.pixels else {
                    return Err(unsupported(
                        "Native gray encoding has no assigned document color conversion",
                    ));
                };
                let profile = match request.header.codec {
                    Codec::JxlRgba8 => PreparedProfile::Jxl,
                    Codec::JxrRgb8 => {
                        let assessment = crate::jxr::assess_import(
                            request.source.as_slice(),
                            u64::from(request.header.pixels),
                            check,
                        )?;
                        let source = assessment.source();
                        if (frame.width, frame.height) != (source.width, source.height)
                            || rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255)
                        {
                            return Err(invalid(
                                "JXR frame geometry or opacity differs from admitted RGB8 source",
                            ));
                        }
                        PreparedProfile::Jxr(assessment)
                    }
                    _ => {
                        return Err(unsupported(
                            "Native sample profile has no assigned document color interpretation",
                        ));
                    }
                };
                let mut admission = Admission::with_residency(
                    paintcraft_core::types::policy::STAGED_PIXELS_MAX,
                    residency,
                );
                let document = paintcraft_document::import_image_with_admission_checked(
                    frame.width,
                    frame.height,
                    rgba,
                    &hex(&request.header.sha256),
                    &mut admission,
                    check,
                )?;
                Ok((document, request.header.sha256, profile))
            },
        )?;
        // Job.consume has now returned: verified launch/reap, post-consume checks,
        // source request and response buffers have all completed/dropped. This
        // constructor remains private; callers cannot adopt from inside consume.
        Ok(PreparedImport {
            document,
            input,
            identity,
            length,
            sha256,
            owner,
            profile,
            _lease: lease,
        })
    }
}
impl PreparedImport {
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn jxr_assessment(&self) -> Option<&crate::jxr::ImportAssessment> {
        self.profile.jxr()
    }
    pub fn losses(&self) -> &'static [&'static str] {
        self.profile.losses()
    }
    pub fn decoder_profile(&self) -> &'static str {
        match self.profile {
            PreparedProfile::Jxl => NATIVE_JXL_IMPORT_VARIANT,
            PreparedProfile::Jxr(_) => NATIVE_JXR_IMPORT_VARIANT,
        }
    }
    /// Revalidate before the host installs a new Session/tab. On refusal the old
    /// host state remains untouched and private document+both charges are dropped.
    pub fn adopt(
        self,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
    ) -> Result<AdoptedImport> {
        self.adopt_with_losses(&[], host_check, current_owner)
    }
    /// Consent is checked against the captured typed source assessment before
    /// the document or its resident leases can be published into a host session.
    pub fn adopt_with_losses(
        self,
        allowed: &[String],
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
    ) -> Result<AdoptedImport> {
        let check = || {
            host_check()?;
            if current_owner()? != self.owner {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Codec import owner changed before adoption",
                ));
            }
            Ok(())
        };
        check()?;
        self.profile.require_losses(allowed)?;
        self.identity
            .verify_bytes(&self.input, self.length, &self.sha256, &check)?;
        check()?;
        // Moving the document preserves each tile's resident BudgetLease. Only
        // preparation scratch is retired; there is no uncharged pixel interval.
        Ok(AdoptedImport {
            document: self.document,
            input: self.input,
            identity: self.identity,
            sha256: self.sha256,
            jxr_assessment: match self.profile {
                PreparedProfile::Jxl => None,
                PreparedProfile::Jxr(assessment) => Some(assessment),
            },
        })
    }
}
#[cfg(test)]
mod tests;
