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
        launch: impl FnOnce(&Path, &Request, Duration, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedImport> {
        if self.codec != Codec::JxlRgba8 {
            return Err(unsupported(
                "Document adoption requires qualified straight sRGB JXL samples",
            ));
        }
        let input = self.input.clone();
        let identity = self.identity.clone();
        let length = self.length;
        let owner = self.owner;
        let (document, sha256) = self.consume(
            host_check,
            current_owner,
            launch,
            |request, frame, _, check| {
                let Pixels::SrgbRgba8(rgba) = frame.pixels else {
                    return Err(unsupported(
                        "Native gray encoding has no assigned document color conversion",
                    ));
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
                Ok((document, request.header.sha256))
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
            _lease: lease,
        })
    }
}
impl PreparedImport {
    /// Revalidate before the host installs a new Session/tab. On refusal the old
    /// host state remains untouched and private document+both charges are dropped.
    pub fn adopt(
        self,
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
        })
    }
}
#[cfg(test)]
mod tests;
