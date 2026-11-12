//! CPU delta ownership; GPU handles remain the existing renderer boundary.
use super::frame::Funding8;
use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use paintcraft_core::types::Result;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct CpuPublication8 {
    image: Option<Arc<ColorImage>>,
    funding: Option<Funding8>,
}
impl CpuPublication8 {
    pub fn new(image: ColorImage, funding: Funding8) -> Self {
        Self {
            image: Some(Arc::new(image)),
            funding: Some(funding),
        }
    }
    /// Private strong clone is handed only to the selected real egui path.
    pub fn install(&self, ctx: &Context) -> TextureHandle {
        ctx.load_texture(
            "web-source8-document",
            self.image.as_ref().expect("live Main image").clone(),
            TextureOptions::NEAREST,
        )
    }
    fn fence(&mut self) -> Result<bool> {
        let Some(image) = self.image.take() else {
            return Ok(true);
        };
        // Required even though selected code never creates a Weak: try_unwrap
        // alone does not prove the logical Arc block has actually deallocated.
        if Arc::weak_count(&image) != 0 {
            self.image = Some(image);
            return Ok(false);
        }
        match Arc::try_unwrap(image) {
            Ok(image) => {
                drop(image); // all Color32 bytes BEFORE any refund callback.
                if let Some(funding) = &self.funding {
                    funding.image_retired()?;
                }
                drop(self.funding.take());
                Ok(true)
            }
            Err(image) => {
                self.image = Some(image);
                Ok(false)
            }
        }
    }
    pub fn poll_retired(&mut self) -> Result<bool> {
        if let Some(funding) = &self.funding {
            funding.spend(1024)?;
        }
        self.fence()
    }
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub fn image(&self) -> &Arc<ColorImage> {
        self.image.as_ref().unwrap()
    }
}
impl Drop for CpuPublication8 {
    fn drop(&mut self) {
        // One consuming fence is prepaid in initial formation. If a real egui
        // alias survives, retain the COMPLETE paired owner, not just its guard.
        if self.fence().ok() != Some(true) {
            if let Some(funding) = &self.funding {
                funding.factory().quarantine();
            }
            std::mem::forget((self.image.take(), self.funding.take()));
        }
    }
}
#[derive(Debug, Default)]
pub(super) struct Retirement8 {
    pending: Option<CpuPublication8>,
    blocked: bool,
}
impl Retirement8 {
    pub fn admit(&mut self, publication: CpuPublication8) -> Result<()> {
        if self.pending.is_some() || self.blocked {
            return Err(super::denied());
        }
        self.pending = Some(publication);
        Ok(())
    }
    pub fn poll(&mut self, ctx: &Context) -> Result<bool> {
        if self.blocked {
            return Err(super::denied());
        }
        let result = match self.pending.as_mut() {
            None => return Ok(true),
            Some(value) => value.poll_retired(),
        };
        match result {
            Ok(true) => {
                drop(self.pending.take());
                Ok(true)
            }
            Ok(false) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(25));
                Ok(false)
            }
            Err(error) => {
                self.blocked = true;
                Err(error)
            }
        }
    }
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}
