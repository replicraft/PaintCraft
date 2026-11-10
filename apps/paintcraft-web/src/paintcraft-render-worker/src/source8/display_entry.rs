//! One read/Entry is reused for every native ROI in this display operation.
use super::*;
impl Entry8<'_> {
    pub(in crate::source8) fn render_display_checked(
        &self,
        frame: &frame_inputs::Source8FrameRealms<'_>,
        display: Source8DisplayPatch,
    ) -> Result<Source8RgbaImage> {
        let SourceSlot8::Loaded(source) = &self.source else {
            return Err(incomplete());
        };
        self.auth.spend(64)?;
        let read_bytes = self.realms.declared_union()?;
        self.auth
            .spend(display_public::ORIGINAL_CONTROL_MOVE_WORK)?;
        let original_c = display_public::OriginalControl8 {
            scope: self.realms.c,
            capacity: self.realms.caps.c,
        };
        source.render_display_closed_checked(
            &self.auth,
            &self.auth.caller,
            frame,
            original_c,
            read_bytes,
            display,
        )
    }
}
#[cfg(test)]
#[path = "tests/display_cases.rs"]
mod tests;
