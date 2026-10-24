//! Committed-document overview: bounded background work with source-checked publication.
use super::*;
use paintcraft_classic::thumbnail::Preview;
use paintcraft_render::display::DisplayFrame;

type Scope = (Uuid, Uuid);
const SIDE: u16 = 256;
const WORK_BYTES: u64 = 8 << 20;
const SOURCE_LIMIT: u64 = 1 << 30;

struct Image {
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
    fn update(
        &mut self,
        ctx: &egui::Context,
        source: Option<Arc<paintcraft_document::Document>>,
        budget: &paintcraft_runtime::SharedBudget,
        admit: bool,
    ) {
        let scope = source.as_ref().map(|doc| (doc.document_id, doc.state_id));
        if self.scope != scope {
            self.scope = scope;
            self.image = None;
            self.failed = false;
            if let (Some(executor), Some((_, handle))) = (&self.executor, self.pending) {
                executor.cancel(handle);
            }
        }
        if let (Some(executor), Some((job_scope, handle))) = (&mut self.executor, self.pending)
            && let Some(result) = executor.poll_for(handle)
        {
            self.pending = None;
            let output = self.output.take();
            if Some(job_scope) == scope {
                match result {
                    Ok(frame) => {
                        let [width, height] = frame.request.size();
                        // Ownership was admitted before the worker started.
                        match output {
                            Some(lease) => {
                                let doc = source.as_ref().expect("matching active scope");
                                let step = frame.request.step() as f32;
                                self.image = Some(Image {
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
                            }
                            None => self.failed = true,
                        }
                    }
                    Err(error)
                        if error.code == paintcraft_core::types::ErrorCode::OperationCancelled => {}
                    Err(_) => self.failed = true,
                }
            }
        }
        if self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        if !admit || self.image.is_some() || self.failed {
            return;
        }
        let Some(doc) = source else { return };
        let reservation = doc.layers.values().try_fold(WORK_BYTES, |total, layer| {
            total.checked_add(
                (layer.raster.resident_tiles() as u64)
                    .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)?,
            )
        });
        let Some(reservation) = reservation.filter(|bytes| *bytes <= SOURCE_LIMIT) else {
            self.failed = true;
            return;
        };
        if self.executor.is_none() {
            match Executor::with_budget(
                WorkerLimits {
                    workers: 1,
                    pending_jobs: 1,
                    transient_bytes: SOURCE_LIMIT,
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
        let request = match paintcraft_render::thumbnail::document_request(&doc, SIDE) {
            Ok(request) => request,
            Err(_) => {
                self.failed = true;
                return;
            }
        };
        let [width, height] = request.size();
        // Keep the eventual frame, ColorImage and texture upload allowance
        // charged through completion and ownership transfer to the UI cache.
        let Ok(output) = budget.reserve(u64::from(width) * u64::from(height) * 12) else {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        };
        let wake = ctx.clone();
        let job_scope = (doc.document_id, doc.state_id);
        match self.executor.as_mut().unwrap().submit(
            reservation,
            move |cancel| {
                paintcraft_render::thumbnail::render_document(&doc, SIDE, || cancel.check())
            },
            move || wake.request_repaint_after(Duration::from_millis(50)),
        ) {
            Ok(handle) => {
                self.pending = Some((job_scope, handle));
                self.output = Some(output);
            }
            Err(_) => ctx.request_repaint_after(Duration::from_millis(250)),
        }
    }

    fn preview(&self) -> Preview {
        match &self.image {
            Some(image) => Preview::Ready {
                texture: image.texture.id(),
                aspect: image.aspect,
                uv: image.uv,
            },
            None if self.failed => Preview::Unavailable,
            None => Preview::Pending,
        }
    }
}

impl App {
    pub(super) fn update_thumbnail(&mut self, ctx: &egui::Context) {
        let visible = self.experience() == Experience::Classic
            && self.classic.thumbnail
            && self.document_count() > 0;
        let source = visible.then(|| self.editor.document_snapshot());
        // Background overview work yields admission while interactive edits run.
        let admit = !self.editor.gesture_in_progress() && !self.editor.has_pending_edits();
        self.thumbnail
            .update(ctx, source, &self.worker_budget, admit);
    }

    pub(super) fn thumbnail_window(&mut self, ctx: &egui::Context) {
        if self.experience() == Experience::Classic
            && self.classic.thumbnail
            && self.document_count() > 0
            && paintcraft_classic::thumbnail::show(
                ctx,
                self.thumbnail.preview(),
                !self.action_context().blocked,
            )
        {
            self.classic.thumbnail = false;
            // Closing releases the texture immediately; in-flight work stays
            // charged and is drained on subsequent frames after cancellation.
            self.thumbnail.update(ctx, None, &self.worker_budget, false);
        }
    }
}

#[cfg(test)]
#[path = "thumbnail/tests.rs"]
mod tests;
