//! Owner-local genuine CPU composition; no domain or Arc visitor escapes.
use super::*;
use paintcraft_core::types::Rect;
use paintcraft_render::blend::service::{self, Caller};
const OPERATION: u64 = 32768 + 256;
impl ClosedSource8 {
    pub(in crate::source8) fn render_closed_checked(
        &self,
        auth: &entry::EntryAuth8<'_>,
        required: &caller::RequiredCaller8,
        frame: &frame_inputs::Source8FrameRealms<'_>,
        read_bytes: u64,
        region: Rect,
    ) -> Result<frame_image::Source8RgbaImage> {
        // Loaded-source C phase is separately named; no F allocation yet.
        auth.spend(4096)?;
        owner_ledger::validate_frame_bootstrap()?;
        let (caller, base, retained, ceiling) = match required {
            caller::RequiredCaller8::Base(base) => (
                Caller::Base(base),
                base,
                base.prepared_owner_union_bytes(None)?,
                base.bytes_limit(),
            ),
            caller::RequiredCaller8::Noise { selected, ambient } => (
                Caller::Noise(selected),
                selected.workspace(),
                selected
                    .workspace()
                    .prepared_owner_union_bytes(Some(ambient.workspace()))?,
                selected.limits().bytes,
            ),
        };
        let validation = frame_inputs::FrameValidation8 {
            workspace: base.scope(),
            read_capacity_sum: read_bytes,
            caller_bytes: ceiling,
            prepared_union: retained,
        };
        frame_inputs::validate_declared(&validation, &frame.caps)?;
        let control = frame.admit_control(auth)?;
        control.validate_originals(validation, auth)?;
        auth.spend(64)?;
        if !paintcraft_render::rooted_frame_inline_layout_supported() {
            return Err(frame_inputs::invalid());
        }
        let doc = &self.payload.0.as_deref().ok_or_else(incomplete)?._document;
        // Only Render receives this512 adapter. Reader already billed its own.
        let check = || auth.step(0);
        let spend = |n| auth.spend(n);
        if let caller::RequiredCaller8::Noise { selected, ambient } = required {
            auth.spend(OPERATION)?;
            let original = ambient.child(selected.limits())?;
            // Actual ambient BaseB is still installed: this checks both complete
            // frozen key owners, including originalA which selectedB could hide.
            service::program_quote_checked(doc, Caller::Noise(&original), &check, &spend)?;
        }
        auth.spend(OPERATION)?;
        let quote = service::program_quote_checked(doc, caller, &check, &spend)?;
        auth.spend(OPERATION)?;
        let call = service::rooted_call_quote_checked(doc, region, caller, &check, &spend)?;
        auth.spend(64)?;
        let tables = if quote.new_retained_bytes == 0 {
            0
        } else {
            quote.numerical.unique_programs as u64
        };
        let table_moves = tables
            .checked_mul(256 + 4 * 512 + 4 * std::mem::size_of::<usize>() as u64)
            .ok_or_else(frame_inputs::invalid)?;
        let extra = frame_work::supplement(
            doc,
            region,
            base,
            call.call.general_samples,
            &quote.numerical,
            auth,
            &check,
        )?;
        auth.spend(extra)?;
        auth.spend(64)?;
        let width = u32::try_from(
            region
                .x1
                .checked_sub(region.x0)
                .ok_or_else(frame_inputs::invalid)?,
        )
        .map_err(|_| frame_inputs::invalid())?;
        let height = u32::try_from(
            region
                .y1
                .checked_sub(region.y0)
                .ok_or_else(frame_inputs::invalid)?,
        )
        .map_err(|_| frame_inputs::invalid())?;
        let pending = frame_output::PendingOutput8::admit(width, height, frame, auth)?;
        auth.spend(
            OPERATION
                .checked_add(table_moves)
                .ok_or_else(frame_inputs::invalid)?,
        )?;
        let programs = service::prepare_programs_checked(doc, caller, &check, &spend)?;
        auth.spend(OPERATION)?;
        let pixels = service::render_rooted_call_checked(doc, region, &programs, &check)?;
        // Vec is paired with its output credit before the first fallible finish.
        let image = pending.finish(pixels, auth)?;
        // Reverse locals order retires Programs before F; source remains alive
        // throughout this method. Image owns its independent original O factory.
        Ok(image)
    }
}
