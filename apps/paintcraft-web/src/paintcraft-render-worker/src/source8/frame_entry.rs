//! Real entry delegation, not a domain visitor or independently renewed caller.
use super::*;
use paintcraft_core::types::Rect;
impl Entry8<'_> {
    pub(in crate::source8) fn render_closed_checked(
        &self,
        frame: &frame_inputs::Source8FrameRealms<'_>,
        region: Rect,
    ) -> Result<frame_image::Source8RgbaImage> {
        let SourceSlot8::Loaded(source) = &self.source else {
            return Err(incomplete());
        };
        self.auth.spend(64)?;
        let read_bytes = self.realms.declared_union()?;
        source.render_closed_checked(&self.auth, &self.auth.caller, frame, read_bytes, region)
    }
}

#[cfg(test)]
#[path = "tests/frame_control_cases.rs"]
mod controls;
