//! MSP uses the existing consent transaction and frozen auxiliary-source guards.
use super::*;
use paintcraft_formats::registry::{self, Family, Platform};
impl App {
    pub(crate) fn stage_msp_import_acquired(
        &mut self,
        source: crate::format_admission::ResourceSource,
        path: PathBuf,
    ) -> Result<()> {
        let family = registry::admit_import(
            self.format_frontend(),
            &path.to_string_lossy(),
            &source.bytes,
            Platform::Native,
        )?;
        if family != Family::Msp {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "MSP source does not contain a historical monochrome raster",
            ));
        }
        self.guard_asset_adoption(&path, Some(&source.identity))?;
        let image = image::decode(&source.bytes)?;
        let digest = hex(&Sha256::digest(&source.bytes));
        self.stage_import(image, path.clone(), digest, true)?;
        self.pending_import
            .as_mut()
            .unwrap()
            .auxiliary_sources
            .push(crate::destination::SourceGuard {
                path,
                identity: source.identity,
            });
        Ok(())
    }
}
#[cfg(test)]
#[path = "msp/tests.rs"]
mod tests;
