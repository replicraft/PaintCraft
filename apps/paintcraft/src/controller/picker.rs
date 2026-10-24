//! Ordered, read-only native color sampling with late-result ownership guards.
use super::*;
use paintcraft_core::types::picker::SampleSource;
use paintcraft_editor::session::HistoryTicket;
use paintcraft_tools::{ColorPick, PointerButton};
pub(super) const STATUS: &str = "Sample color";
#[derive(Debug)]
pub(super) struct Picked {
    base: HistoryTicket,
    pick: ColorPick,
    color: Rgba,
}
fn sample(doc: &Document, pick: &ColorPick, check: impl Fn() -> Result<()>) -> Result<Rgba> {
    let layer = match pick.source {
        SampleSource::Image => None,
        SampleSource::Layer => Some(doc.layers.get(&pick.layer).ok_or_else(|| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ReferenceUnknownId,
                "sampling layer is unavailable",
            )
        })?),
    };
    paintcraft_render::sampling::sample_color(doc, layer, pick.at, pick.size, check)
}
impl Editor {
    pub(super) fn pick_color(&mut self, pick: ColorPick) {
        if !self.tools.can_apply_color_pick(&pick) {
            self.status = "Color sample cancelled: tool or color slot changed".into();
            return;
        }
        let base = self.session.history_ticket();
        let doc = self.document_snapshot();
        if self.editing.is_some() {
            let task = Box::new(move |cancel: paintcraft_runtime::Cancellation| {
                let color = sample(&doc, &pick, || cancel.check())?;
                Ok(background::Completion::Pick(Box::new(Picked {
                    base,
                    pick,
                    color,
                })))
            }) as dispatch::Task;
            self.dispatch_initial(Ok(task), STATUS.into(), "", None);
        } else {
            match sample(&doc, &pick, || Ok(())) {
                Ok(color) => {
                    let _ = self.accept_pick(Picked { base, pick, color });
                }
                Err(e) => self.status = format!("Color sample failed: {e}"),
            }
        }
    }
    pub(super) fn accept_pick(&mut self, result: Picked) -> Result<()> {
        if self.session.history_ticket() != result.base
            || !self.tools.can_apply_color_pick(&result.pick)
        {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ConflictStaleBase,
                "color sample discarded: document, tool or color slot changed",
            ));
        }
        self.tools.apply_picked_color(&result.pick, result.color);
        self.status = format!(
            "{} color: {}",
            if result.pick.button == PointerButton::Primary {
                "Primary"
            } else {
                "Secondary"
            },
            result.color.to_hex()
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "picker/tests.rs"]
mod tests;
