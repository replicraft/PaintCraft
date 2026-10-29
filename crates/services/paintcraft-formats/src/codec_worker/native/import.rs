//! Resident adoption after bounded isolated decoding, never public read admission.
use super::*;
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};
use paintcraft_document::Document;
use paintcraft_raster::Admission;

#[derive(Debug)]
pub struct PreparedImport {
    pub(super) document: Document,
    pub(super) input: PathBuf,
    pub(super) identity: SourceIdentity,
    pub(super) length: u64,
    pub(super) sha256: [u8; 32],
    pub(super) owner: Owner,
    pub(super) profile: PreparedProfile,
    // The dead helper/source/output charge stays held until adoption or discard.
    // Document drops first on stale/error, before this final preparation charge.
    pub(super) _lease: ResourceLease,
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
    pub heif_assessment: Option<crate::heif::ImportAssessment>,
    pub avif_assessment: Option<crate::avif::ImportAssessment>,
    pub float_assessment: Option<Arc<crate::float_import::WorkerAssessment>>,
    pub avif_sdr_assessment: Option<crate::avif::sdr::ImportAssessment>,
    pub jxl_integer_assessment: Option<crate::codec_worker::jxl_integer::Report>,
}
#[derive(Debug)]
pub(super) enum PreparedProfile {
    Float {
        codec: Codec,
        assessment: Arc<crate::float_import::WorkerAssessment>,
    },
    Jxl,
    JxlInteger(crate::codec_worker::jxl_integer::Report),
    Jxr(crate::jxr::ImportAssessment),
    JxrWriter(crate::jxr::ImportAssessment),
    Heif(crate::heif::ImportAssessment),
    Avif(crate::avif::ImportAssessment),
    AvifSdr(crate::avif::sdr::ImportAssessment),
}
pub(super) enum Selection {
    Fixed,
    Avif(crate::avif::FamilyAssessment),
    Jxr(crate::jxr::NativeImportAssessment),
}
impl Selection {
    fn writer(&self) -> bool {
        matches!(self, Self::Jxr(value) if value.is_writer_compatible())
    }
}
impl PreparedProfile {
    fn jxr(&self) -> Option<&crate::jxr::ImportAssessment> {
        match self {
            Self::Jxl
            | Self::JxlInteger(_)
            | Self::Heif(_)
            | Self::Avif(_)
            | Self::AvifSdr(_)
            | Self::Float { .. } => None,
            Self::Jxr(assessment) | Self::JxrWriter(assessment) => Some(assessment),
        }
    }
    fn losses(&self) -> &[&'static str] {
        match self {
            Self::Jxl => &[],
            Self::JxlInteger(report) => report.losses(),
            Self::Float { assessment, .. } => assessment.assessment().losses(),
            Self::Jxr(assessment) | Self::JxrWriter(assessment) => assessment.losses(),
            Self::Heif(assessment) => assessment.losses(),
            Self::Avif(assessment) => assessment.losses(),
            Self::AvifSdr(assessment) => assessment.losses(),
        }
    }
    fn require_losses(&self, allowed: &[String]) -> Result<()> {
        match self {
            Self::Jxl => {}
            Self::JxlInteger(report) => jxl_integer::require_losses(report, allowed)?,
            Self::Float { assessment, .. } => assessment.assessment().require_losses(allowed)?,
            Self::Jxr(assessment) | Self::JxrWriter(assessment) => {
                assessment.require_losses(allowed)?
            }
            Self::Heif(assessment) => assessment.require_losses(allowed)?,
            Self::Avif(assessment) => assessment.require_losses(allowed)?,
            Self::AvifSdr(assessment) => assessment.require_losses(allowed)?,
        }
        Ok(())
    }
}
fn heif_profile(
    request: &Request,
    frame: &Frame<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<crate::heif::ImportAssessment> {
    let assessment = crate::heif::assess_import(
        request.source.as_slice(),
        u64::from(request.header.pixels),
        check,
    )?;
    let Pixels::SrgbRgba8(rgba) = frame.pixels else {
        return Err(unsupported("HEIF frame lacks sRGB RGBA8 interpretation"));
    };
    if assessment.sha256() != request.header.sha256
        || assessment.size() != (frame.width, frame.height)
        || rgba.len() as u64 != u64::from(frame.width) * u64::from(frame.height) * 4
        || (!assessment.alpha() && rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255))
    {
        return Err(invalid(
            "HEIF frame differs from captured source declarations",
        ));
    }
    Ok(assessment)
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
    /// Prepare an explicitly selected qualified sample profile. Identity AVIF
    /// and SDR AVIF remain distinct codecs and source assessments; the latter
    /// always requires reconstruction consent. Gray planes have no interpretation.
    pub fn prepare_import(
        self,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        self.prepare_selected(
            false,
            residency,
            lease,
            host_check,
            current_owner,
            |_, _, _| Ok(Selection::Fixed),
            launch,
        )
    }
    /// Explicit family selection from the same acquired source bytes. The caller
    /// must admit the selected frontend/variant before helper discovery or launch.
    /// This does not enable any registry route or retry failed decoding.
    pub fn prepare_avif_family(
        self,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        admit: impl FnOnce(Codec, &crate::avif::FamilyAssessment) -> Result<()>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if !matches!(self.codec, Codec::AvifSrgb8 | Codec::AvifSdr8) {
            return Err(unsupported(
                "AVIF family selection requires an explicit AVIF job",
            ));
        }
        self.prepare_selected(
            false,
            residency,
            lease,
            host_check,
            current_owner,
            |bytes, pixels, check| {
                let selected = crate::avif::assess_family(bytes, pixels, check)?;
                let codec = match &selected {
                    crate::avif::FamilyAssessment::Identity(_) => Codec::AvifSrgb8,
                    crate::avif::FamilyAssessment::Sdr(_) => Codec::AvifSdr8,
                };
                admit(codec, &selected)?;
                check()?;
                Ok(Selection::Avif(selected))
            },
            launch,
        )
    }
    #[allow(clippy::too_many_arguments)] // Closed profile selection shares all resident adoption logic.
    pub(super) fn prepare_selected(
        self,
        writer: bool,
        residency: SharedBudget,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        select: impl FnOnce(&[u8], u64, &dyn Fn() -> Result<()>) -> Result<Selection>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if !matches!(
            self.codec,
            Codec::JxlRgba8
                | Codec::JxlIntegerRgba8
                | Codec::JxrRgb8
                | Codec::HeifSrgb8
                | Codec::AvifSrgb8
                | Codec::AvifSdr8
        ) {
            return Err(unsupported(
                "Native samples have no qualified document color interpretation",
            ));
        }
        let input = self.input.clone();
        let identity = self.identity.clone();
        let length = self.length;
        let owner = self.owner;
        let fixed_codec = self.codec;
        let (document, sha256, profile) = self.consume_profile(
            host_check,
            current_owner,
            |bytes, pixels, check| {
                let selected = select(bytes, pixels, check)?;
                let codec = match &selected {
                    Selection::Avif(crate::avif::FamilyAssessment::Identity(_)) => Codec::AvifSrgb8,
                    Selection::Avif(crate::avif::FamilyAssessment::Sdr(_)) => Codec::AvifSdr8,
                    _ => fixed_codec,
                };
                let mode = writer || selected.writer();
                Ok((codec, selected, mode))
            },
            launch,
            |request, frame, selected, _, check| {
                let writer = writer || selected.writer();
                let Pixels::SrgbRgba8(rgba) = frame.pixels else {
                    return Err(unsupported(
                        "Native gray encoding has no assigned document color conversion",
                    ));
                };
                let profile = match request.header.codec {
                    Codec::JxlRgba8 => PreparedProfile::Jxl,
                    Codec::JxlIntegerRgba8 => {
                        let report = frame
                            .jxl_report
                            .ok_or_else(|| invalid("Integer JXL result lacks its assessment"))?;
                        report.validate_payload(request.header, frame.width, frame.height, rgba)?;
                        PreparedProfile::JxlInteger(report)
                    }
                    Codec::JxrRgb8 => {
                        let assessment = match selected {
                            Selection::Jxr(value) => value.assessment().clone(),
                            _ => {
                                let assess = if writer {
                                    crate::jxr::assess_writer_import
                                } else {
                                    crate::jxr::assess_import
                                };
                                assess(
                                    request.source.as_slice(),
                                    u64::from(request.header.pixels),
                                    check,
                                )?
                            }
                        };
                        let source = assessment.source();
                        if (frame.width, frame.height) != (source.width, source.height)
                            || rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255)
                        {
                            return Err(invalid(
                                "JXR frame geometry or opacity differs from admitted RGB8 source",
                            ));
                        }
                        if writer {
                            PreparedProfile::JxrWriter(assessment)
                        } else {
                            PreparedProfile::Jxr(assessment)
                        }
                    }
                    Codec::AvifSrgb8 => {
                        let assessment = match selected {
                            Selection::Avif(crate::avif::FamilyAssessment::Identity(
                                assessment,
                            )) => assessment,
                            Selection::Avif(_) | Selection::Jxr(_) => {
                                return Err(invalid("AVIF selected assessment/profile mismatch"));
                            }
                            Selection::Fixed => crate::avif::assess_import(
                                request.source.as_slice(),
                                u64::from(request.header.pixels),
                                check,
                            )?,
                        };
                        if assessment.sha256() != request.header.sha256
                            || assessment.size() != (frame.width, frame.height)
                            || (!assessment.alpha()
                                && rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255))
                        {
                            return Err(invalid(
                                "AVIF frame differs from captured source declarations",
                            ));
                        }
                        PreparedProfile::Avif(assessment)
                    }
                    Codec::AvifSdr8 => {
                        let assessment = match selected {
                            Selection::Avif(crate::avif::FamilyAssessment::Sdr(assessment)) => {
                                assessment
                            }
                            Selection::Avif(_) | Selection::Jxr(_) => {
                                return Err(invalid("AVIF selected assessment/profile mismatch"));
                            }
                            Selection::Fixed => crate::avif::sdr::assess_import(
                                request.source.as_slice(),
                                u64::from(request.header.pixels),
                                check,
                            )?,
                        };
                        if assessment.sha256() != request.header.sha256
                            || assessment.source_len() != request.header.length
                            || assessment.size() != (frame.width, frame.height)
                            || (!assessment.alpha()
                                && rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255))
                        {
                            return Err(invalid(
                                "AVIF SDR frame differs from captured source declarations",
                            ));
                        }
                        PreparedProfile::AvifSdr(assessment)
                    }
                    Codec::HeifSrgb8 => {
                        PreparedProfile::Heif(heif_profile(request, &frame, check)?)
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
    pub fn float_assessment(&self) -> Option<&crate::float_import::Assessment> {
        match &self.profile {
            PreparedProfile::Float { assessment, .. } => Some(assessment.assessment()),
            _ => None,
        }
    }
    /// Float consent binds the exact captured source, settings and worker pixels.
    pub fn adopt_float(
        self,
        expected_digest: [u8; 32],
        allowed: &[String],
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
    ) -> Result<AdoptedImport> {
        let assessment = self
            .float_assessment()
            .ok_or_else(|| unsupported("Float adoption requires a float result"))?;
        if assessment.digest() != expected_digest {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Float import assessment/settings changed since consent",
            ));
        }
        self.adopt_checked(allowed, host_check, current_owner)
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn jxr_assessment(&self) -> Option<&crate::jxr::ImportAssessment> {
        self.profile.jxr()
    }
    pub fn heif_assessment(&self) -> Option<&crate::heif::ImportAssessment> {
        if let PreparedProfile::Heif(assessment) = &self.profile {
            Some(assessment)
        } else {
            None
        }
    }
    pub fn avif_assessment(&self) -> Option<&crate::avif::ImportAssessment> {
        if let PreparedProfile::Avif(assessment) = &self.profile {
            Some(assessment)
        } else {
            None
        }
    }
    pub fn avif_sdr_assessment(&self) -> Option<&crate::avif::sdr::ImportAssessment> {
        if let PreparedProfile::AvifSdr(assessment) = &self.profile {
            Some(assessment)
        } else {
            None
        }
    }
    pub fn losses(&self) -> &[&'static str] {
        self.profile.losses()
    }
    pub fn decoder_profile(&self) -> &'static str {
        match self.profile {
            PreparedProfile::Jxl => NATIVE_JXL_IMPORT_VARIANT,
            PreparedProfile::JxlInteger(_) => {
                crate::codec_worker::jxl_integer::contracts::IMPORT_VARIANT
            }
            PreparedProfile::Float {
                codec: Codec::HdrSdr8,
                ..
            } => NATIVE_HDR_SDR_IMPORT_VARIANT,
            PreparedProfile::Float {
                codec: Codec::ExrSdr8,
                ..
            } => NATIVE_EXR_SDR_IMPORT_VARIANT,
            PreparedProfile::Float { .. } => {
                unreachable!("private float profile requires a float codec")
            }
            PreparedProfile::Jxr(_) => NATIVE_JXR_IMPORT_VARIANT,
            PreparedProfile::JxrWriter(_) => {
                super::super::jxr_encode::qualified::WRITER_IMPORT_VARIANT
            }
            PreparedProfile::Heif(_) => NATIVE_HEIF_IMPORT_VARIANT,
            PreparedProfile::Avif(_) => NATIVE_AVIF_IMPORT_VARIANT,
            PreparedProfile::AvifSdr(_) => NATIVE_AVIF_SDR_IMPORT_VARIANT,
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
        if self.float_assessment().is_some() || self.jxl_integer_assessment().is_some() {
            return Err(unsupported(
                "Assessed import adoption requires its assessment digest and losses",
            ));
        }
        self.adopt_checked(allowed, host_check, current_owner)
    }
    fn adopt_checked(
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
        let float_assessment = match &self.profile {
            PreparedProfile::Float { assessment, .. } => Some(assessment.clone()),
            _ => None,
        };
        let jxl_integer_assessment = self.jxl_integer_assessment().copied();
        let (jxr_assessment, heif_assessment, avif_assessment, avif_sdr_assessment) =
            match self.profile {
                PreparedProfile::Jxl
                | PreparedProfile::JxlInteger(_)
                | PreparedProfile::Float { .. } => (None, None, None, None),
                PreparedProfile::Jxr(assessment) | PreparedProfile::JxrWriter(assessment) => {
                    (Some(assessment), None, None, None)
                }
                PreparedProfile::Heif(assessment) => (None, Some(assessment), None, None),
                PreparedProfile::Avif(assessment) => (None, None, Some(assessment), None),
                PreparedProfile::AvifSdr(assessment) => (None, None, None, Some(assessment)),
            };
        Ok(AdoptedImport {
            document: self.document,
            input: self.input,
            identity: self.identity,
            sha256: self.sha256,
            jxr_assessment,
            heif_assessment,
            avif_assessment,
            avif_sdr_assessment,
            float_assessment,
            jxl_integer_assessment,
        })
    }
}
#[cfg(test)]
mod family_tests;
#[cfg(test)]
mod heif_tests;
mod jxl_integer;
#[cfg(test)]
mod sdr_installed_tests;
#[cfg(test)]
mod tests;
