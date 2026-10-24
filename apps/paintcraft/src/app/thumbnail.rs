//! Full-canvas overview: admitted private staging and source-checked UI publication.
use super::*;
use paintcraft_classic::thumbnail::Preview as ImagePreview;
use paintcraft_render::display::DisplayFrame;

#[path = "thumbnail/source.rs"]
mod source;
use source::{Scope, Source};
const SIDE: u16 = 256;
const WORK_BYTES: u64 = 8 << 20;
const SOURCE_LIMIT: u64 = 1 << 30;
// Full editor staging quota plus a declared command/geometry allowance. This
// reserves logical capacity, not a promise about allocator or process RSS.
const PREVIEW_BYTES: u64 = paintcraft_core::types::policy::STAGED_PIXELS_MAX * 4 + (256 << 20);

struct Image {
    scope: Scope,
    texture: egui::TextureHandle,
    aspect: f32,
    uv: egui::Rect,
    _lease: paintcraft_core::types::resources::BudgetLease,
}

#[derive(Default)]
pub(super) struct Thumbnail {
    scope: Option<Scope>,
    executor: Option<Executor<DisplayFrame>>,
    pending: Option<(Scope, JobHandle)>,
    image: Option<Image>,
    output: Option<paintcraft_core::types::resources::BudgetLease>,
    failed: bool,
}

impl Thumbnail {
    #[cfg(test)]
    fn update(
        &mut self,
        ctx: &egui::Context,
        doc: Option<Arc<paintcraft_document::Document>>,
        budget: &paintcraft_runtime::SharedBudget,
        admit: bool,
    ) {
        self.update_source(ctx, doc.map(Source::committed), budget, admit);
    }

    fn update_source(
        &mut self,
        ctx: &egui::Context,
        source: Option<Source>,
        budget: &paintcraft_runtime::SharedBudget,
        admit: bool,
    ) {
        let scope = source.as_ref().map(|source| source.scope);
        if self.scope != scope {
            self.scope = scope;
            self.failed = false;
            if !self
                .image
                .as_ref()
                .is_some_and(|image| scope.is_some_and(|next| image.scope.can_progress_to(next)))
            {
                self.image = None;
            }
            // Coalesce motion within one gesture instead of cancelling every
            // useful prefix. Cancellation/base/owner changes invalidate it.
            if let (Some(executor), Some((pending, handle))) = (&self.executor, self.pending)
                && !scope.is_some_and(|next| pending.can_progress_to(next))
            {
                executor.cancel(handle);
            }
        }
        if let (Some(executor), Some((job_scope, handle))) = (&mut self.executor, self.pending)
            && let Some(result) = executor.poll_for(handle)
        {
            self.pending = None;
            let output = self.output.take();
            if scope.is_some_and(|next| job_scope == next || job_scope.can_progress_to(next)) {
                match result {
                    Ok(frame) => {
                        let [width, height] = frame.request.size();
                        if let Some(lease) = output {
                            let doc = &source.as_ref().expect("matching active source").doc;
                            let step = frame.request.step() as f32;
                            self.image = Some(Image {
                                scope: job_scope,
                                texture: ctx.load_texture(
                                    "classic-document-thumbnail",
                                    egui::ColorImage::from_rgba_unmultiplied(
                                        [width as usize, height as usize],
                                        &frame.pixels,
                                    ),
                                    egui::TextureOptions::LINEAR,
                                ),
                                aspect: doc.canvas.0 as f32 / doc.canvas.1 as f32,
                                uv: egui::Rect::from_min_max(
                                    egui::Pos2::ZERO,
                                    egui::pos2(
                                        doc.canvas.0 as f32 / (width as f32 * step),
                                        doc.canvas.1 as f32 / (height as f32 * step),
                                    ),
                                ),
                                _lease: lease,
                            });
                        } else {
                            self.failed = true;
                            self.image = None;
                        }
                    }
                    Err(error)
                        if error.code == paintcraft_core::types::ErrorCode::OperationCancelled => {}
                    Err(_) if Some(job_scope) == scope => {
                        self.failed = true;
                        self.image = None;
                    }
                    Err(_) => {} // A superseded prefix failure cannot poison the new request.
                }
            }
        }
        if self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        if !admit
            || self.failed
            || self
                .image
                .as_ref()
                .is_some_and(|image| Some(image.scope) == scope)
        {
            return;
        }
        let Some(source) = source else { return };
        let doc = &source.doc;
        let source_bytes = doc.layers.values().try_fold(0_u64, |total, layer| {
            total.checked_add(
                (layer.raster.resident_tiles() as u64)
                    .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)?,
            )
        });
        let Some(source_bytes) = source_bytes.filter(|bytes| *bytes <= SOURCE_LIMIT) else {
            self.failed = true;
            self.image = None;
            return;
        };
        let reservation = WORK_BYTES
            + source_bytes
            + if source.has_preview() {
                PREVIEW_BYTES
            } else {
                0
            };
        if self.executor.is_none() {
            match Executor::with_budget(
                WorkerLimits {
                    workers: 1,
                    pending_jobs: 1,
                    transient_bytes: SOURCE_LIMIT + PREVIEW_BYTES + WORK_BYTES,
                },
                budget.clone(),
            ) {
                Ok(executor) => self.executor = Some(executor),
                Err(_) => {
                    self.failed = true;
                    return;
                }
            }
        }
        let request = match paintcraft_render::thumbnail::document_request(doc, SIDE) {
            Ok(request) => request,
            Err(_) => {
                self.failed = true;
                self.image = None;
                return;
            }
        };
        let [width, height] = request.size();
        let Ok(output) = budget.reserve(u64::from(width) * u64::from(height) * 12) else {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        };
        let wake = ctx.clone();
        let job_scope = source.scope;
        match self.executor.as_mut().unwrap().submit(
            reservation,
            move |cancel| source.render(|| cancel.check()),
            move || wake.request_repaint_after(Duration::from_millis(50)),
        ) {
            Ok(handle) => {
                self.pending = Some((job_scope, handle));
                self.output = Some(output);
            }
            Err(_) => ctx.request_repaint_after(Duration::from_millis(250)),
        }
    }

    fn updating(&self) -> bool {
        !self.failed
            && self.scope.is_some()
            && self
                .image
                .as_ref()
                .is_none_or(|image| Some(image.scope) != self.scope)
    }

    fn preview(&self) -> ImagePreview {
        match &self.image {
            Some(image) => ImagePreview::Ready {
                texture: image.texture.id(),
                aspect: image.aspect,
                uv: image.uv,
            },
            None if self.failed => ImagePreview::Unavailable,
            None => ImagePreview::Pending,
        }
    }
}

impl App {
    pub(super) fn update_thumbnail(&mut self, ctx: &egui::Context) {
        let visible = self.experience() == Experience::Classic
            && self.classic.thumbnail
            && self.document_count() > 0;
        let source = visible.then(|| Source::capture(&self.editor));
        self.thumbnail.update_source(
            ctx,
            source,
            &self.worker_budget,
            !self.action_context().blocked,
        );
    }

    pub(super) fn thumbnail_window(&mut self, ctx: &egui::Context) {
        if self.experience() == Experience::Classic
            && self.classic.thumbnail
            && self.document_count() > 0
            && paintcraft_classic::thumbnail::show(
                ctx,
                self.thumbnail.preview(),
                self.thumbnail.updating(),
                !self.action_context().blocked,
            )
        {
            self.classic.thumbnail = false;
            self.thumbnail
                .update_source(ctx, None, &self.worker_budget, false);
        }
    }
}

#[cfg(test)]
#[path = "thumbnail/tests.rs"]
mod tests;
