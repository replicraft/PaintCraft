//! Image-only raw Source6 ROI consumer. No encoder, new admission, or crop.
use super::*;
#[cfg(any(target_arch = "wasm32", test))]
use paintcraft_core::types::Rgba;
use paintcraft_core::types::picker::SampleSource;
use paintcraft_tools::{Action, ColorPick, ToolOptions};

pub(super) struct Intent {
    pub(super) pick: ColorPick,
    options: ToolOptions,
    target: Option<Uuid>,
    controls_tool: Tool,
    controls_primary: paintcraft_core::types::Rgba,
    controls_secondary: paintcraft_core::types::Rgba,
    controls_options: ToolOptions,
}
impl Intent {
    fn current(&self, app: &WebApp) -> bool {
        app.editor.tools.can_apply_color_pick(&self.pick)
            && app.editor.tools.options == self.options
            && app.editor.target == self.target
            // Forms/toolbars may update Controls after canvas synchronized
            // Tools. The next result pump runs before the next canvas sync.
            && app.controls.tool == self.controls_tool
            && app.controls.primary == self.controls_primary
            && app.controls.secondary == self.controls_secondary
            && app.controls.options == self.controls_options
    }
}
impl Job {
    pub(super) fn current(&self, app: &WebApp) -> bool {
        self.frozen.current(app)
            && match self.purpose.as_ref() {
                Some(Purpose::Picker(intent)) => intent.current(app),
                _ => true,
            }
    }
    fn is_picker(&self) -> bool {
        matches!(self.purpose, Some(Purpose::Picker(_)))
    }
}
/// Identical geometry and straight encoded-channel averaging to sampling.rs.
/// The source Document remains complete so coordinates/Box halos remain true.
pub(super) fn region(doc: &Document, pick: &ColorPick) -> Result<Rect> {
    if !doc.canvas_rect().contains(pick.at.0, pick.at.1) {
        return Err(rejected("Color sample is outside the canvas"));
    }
    let side = i64::from(pick.size.side());
    Rect::new(pick.at.0 - side / 2, pick.at.1 - side / 2, side, side)
        .and_then(|r| r.intersect(&doc.canvas_rect()))
        .ok_or_else(|| rejected("Color sample extent"))
}
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn average(bytes: &[u8], region: Rect) -> Result<Rgba> {
    let count = u64::try_from(region.area()).map_err(|_| rejected("Sample area"))?;
    if count == 0 || count > 51 * 51 || count.checked_mul(4) != Some(bytes.len() as u64) {
        return Err(rejected(
            "Color sample payload differs from its bounded ROI",
        ));
    }
    let mut sums = [0u64; 4];
    for pixel in bytes.as_chunks::<4>().0 {
        for (sum, byte) in sums.iter_mut().zip(pixel) {
            *sum += u64::from(*byte);
        }
    }
    Ok(Rgba(sums.map(|sum| ((sum + count / 2) / count) as u8)))
}
impl WebApp {
    pub(crate) fn noise_picker_pending(&self) -> bool {
        self.filter_display.picker_wanted.is_some()
            || self
                .filter_display
                .copy_action
                .as_ref()
                .is_some_and(Job::is_picker)
    }
    /// Returns true only for the newly asynchronous Image+stored-Noise route.
    /// Existing Layer/raw and old-profile sampling still enter Editor::apply.
    pub(crate) fn queue_noise_picker(&mut self, action: &Action) -> bool {
        let Action::PickColor(pick) = action else {
            return false;
        };
        if pick.source != SampleSource::Image
            || !render::stored_scoped(self.editor.session.document())
        {
            return false;
        }
        if let Err(error) = self.capture_noise_pick(pick.clone()) {
            self.message = format!("Color sample unavailable: {error}");
        }
        true
    }
    fn capture_noise_pick(&mut self, pick: ColorPick) -> Result<()> {
        if self.filter_editing.is_some() || !self.editor.tools.can_apply_color_pick(&pick) {
            return Err(rejected("Color sample intent is no longer current"));
        }
        if self
            .filter_display
            .copy_action
            .as_ref()
            .is_some_and(|j| !j.is_picker())
        {
            return Err(rejected("Another background image action is pending"));
        }
        if !self.ordinary_filter_preview() {
            self.forget_noise_preview();
        }
        let capture = self.filter_capture();
        let area = region(&capture.document, &pick)?;
        let job = Job {
            frozen: Frozen::capture(self),
            capture,
            region: area,
            spec: Value::Null,
            mask: Some(self.empty_copy_mask()),
            purpose: Some(Purpose::Picker(Intent {
                pick,
                options: self.editor.tools.options,
                target: self.editor.target,
                controls_tool: self.controls.tool,
                controls_primary: self.controls.primary,
                controls_secondary: self.controls.secondary,
                // Snapshot independently: canvas intentionally forces picker
                // source/size/after (and eraser color) only in ToolState.
                controls_options: self.controls.options,
            })),
            outcome: None,
            cancelled: Rc::new(Cell::new(false)),
            #[cfg(target_arch = "wasm32")]
            control: Rc::default(),
        };
        // Cancellation flags are per job: two identical clicks can share the
        // ToolState revision/original color without allowing the old reply.
        if let Some(active) = &self.filter_display.copy_action {
            active.cancelled.set(true);
            #[cfg(target_arch = "wasm32")]
            if let Some(control) = active.control.borrow().as_ref() {
                control.cancel();
            }
        }
        self.filter_display.picker_wanted = Some(job);
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) async fn sample(client: &bridge::Client, capture: &Capture, area: Rect) -> Result<Rgba> {
    let reply = client
        .run(
            Request {
                revision: Revision {
                    draft: capture.draft,
                    generation: 1,
                },
                edit: None,
                render: true,
                view: View::new(&capture.document, area, 0)?,
            },
            1,
        )
        .await?;
    // Client::run has already validated owner/source/generation/facts/ROI and
    // synchronously acknowledged consumption of this one bounded raw frame.
    average(
        reply
            .pixels
            .as_deref()
            .ok_or_else(|| rejected("No raw sample"))?,
        area,
    )
}
