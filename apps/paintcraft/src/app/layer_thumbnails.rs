//! Bounded, committed-only layer previews. UI publication validates source identity.
use super::*;
use paintcraft_render::display::DisplayFrame;
use std::collections::{HashMap, HashSet};

type Scope = (Uuid, Uuid);
const SIDE: u16 = 64;
const VISIBLE_MAX: usize = 64;
// 64² output, tile reduction accumulators, bounded 256² native fallback,
// and small isolated-document/cache metadata. Source raster snapshots are shared.
const WORK_BYTES: u64 = 1 << 20;
const SOURCE_LIMIT: u64 = 1 << 30;

struct Cached {
    texture: egui::TextureHandle,
    aspect: f32,
    uv: egui::Rect,
    _lease: paintcraft_core::types::resources::BudgetLease,
}

#[derive(Default)]
pub(super) struct LayerThumbnails {
    scope: Option<Scope>,
    executor: Option<Executor<DisplayFrame>>,
    disabled: bool,
    pending: Option<(Scope, Uuid, JobHandle)>,
    cache: HashMap<Uuid, Cached>,
    failed: HashSet<Uuid>,
}

impl LayerThumbnails {
    pub fn begin(
        &mut self,
        ctx: &egui::Context,
        doc: &paintcraft_document::Document,
        budget: &paintcraft_runtime::SharedBudget,
    ) {
        let scope = (doc.document_id, doc.state_id);
        if self.scope != Some(scope) {
            self.scope = Some(scope);
            self.cache.clear();
            self.failed.clear();
            if let (Some(executor), Some((_, _, handle))) = (&self.executor, self.pending) {
                executor.cancel(handle);
            }
        }
        if let (Some(executor), Some((source, layer, handle))) = (&mut self.executor, self.pending)
            && let Some(result) = executor.poll_for(handle)
        {
            self.pending = None;
            if source == scope {
                match result {
                    Ok(frame) => {
                        let [width, height] = frame.request.size();
                        // Retain a declared CPU image + texture allowance until eviction.
                        let Ok(lease) = budget.reserve(u64::from(width) * u64::from(height) * 8)
                        else {
                            ctx.request_repaint_after(Duration::from_millis(250));
                            return;
                        };
                        let step = frame.request.step() as f32;
                        let uv = egui::Rect::from_min_max(
                            egui::Pos2::ZERO,
                            egui::pos2(
                                doc.canvas.0 as f32 / (width as f32 * step),
                                doc.canvas.1 as f32 / (height as f32 * step),
                            ),
                        );
                        let texture = ctx.load_texture(
                            format!("layer-preview-{layer}"),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [width as usize, height as usize],
                                &frame.pixels,
                            ),
                            egui::TextureOptions::LINEAR,
                        );
                        self.cache.insert(
                            layer,
                            Cached {
                                texture,
                                aspect: doc.canvas.0 as f32 / doc.canvas.1 as f32,
                                uv,
                                _lease: lease,
                            },
                        );
                    }
                    Err(error)
                        if error.code == paintcraft_core::types::ErrorCode::OperationCancelled => {}
                    Err(_) => {
                        self.failed.insert(layer);
                    }
                }
            }
        }
        if self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    pub fn preview(&self, layer: Uuid) -> paintcraft_classic::layers::Preview {
        use paintcraft_classic::layers::Preview;
        if let Some(image) = self.cache.get(&layer) {
            Preview::Ready {
                texture: image.texture.id(),
                aspect: image.aspect,
                uv: image.uv,
            }
        } else if self.disabled || self.failed.contains(&layer) {
            Preview::Unavailable
        } else {
            Preview::Pending
        }
    }

    pub fn request(
        &mut self,
        ctx: &egui::Context,
        doc: Arc<paintcraft_document::Document>,
        visible: &[Uuid],
        budget: &paintcraft_runtime::SharedBudget,
    ) {
        let visible = &visible[..visible.len().min(VISIBLE_MAX)];
        self.cache.retain(|id, _| visible.contains(id));
        self.failed.retain(|id| visible.contains(id));
        if let (Some(executor), Some((_, layer, handle))) = (&self.executor, self.pending)
            && !visible.contains(&layer)
        {
            executor.cancel(handle);
        }
        if self.disabled || self.pending.is_some() {
            return;
        }
        let Some(layer) = visible
            .iter()
            .copied()
            .find(|id| !self.cache.contains_key(id) && !self.failed.contains(id))
        else {
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
                    self.disabled = true;
                    return;
                }
            }
        }
        let Some(source_layer) = doc.layers.get(&layer) else {
            return;
        };
        let reservation = WORK_BYTES.saturating_add(
            source_layer.raster.resident_tiles() as u64
                * paintcraft_raster::RasterLayer::TILE_BYTES as u64,
        );
        if reservation > SOURCE_LIMIT {
            self.failed.insert(layer);
            return;
        }
        // Pin only the requested layer, not every raster in an old document.
        let source = paintcraft_document::Document {
            source_metadata: None,
            document_id: doc.document_id,
            state_id: doc.state_id,
            canvas: doc.canvas,
            roots: vec![layer],
            layers: [(layer, source_layer.clone())].into(),
            title: None,
        };
        let executor = self.executor.as_mut().unwrap();
        let scope = (doc.document_id, doc.state_id);
        let wake = ctx.clone();
        match executor.submit(
            reservation,
            move |cancel| {
                let layer = source.layers.get(&layer).ok_or_else(|| {
                    paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::TargetInvalid,
                        "thumbnail layer no longer exists",
                    )
                })?;
                paintcraft_render::thumbnail::render_layer(&source, layer, SIDE, || cancel.check())
            },
            move || wake.request_repaint_after(Duration::from_millis(50)),
        ) {
            Ok(handle) => self.pending = Some((scope, layer, handle)),
            Err(_) => ctx.request_repaint_after(Duration::from_millis(250)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_classic::layers::Preview;
    use paintcraft_document::{POLICY, new_document};
    use std::time::Instant;

    fn drain(
        service: &mut LayerThumbnails,
        ctx: &egui::Context,
        doc: &paintcraft_document::Document,
        budget: &paintcraft_runtime::SharedBudget,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while service.pending.is_some() {
            service.begin(ctx, doc, budget);
            assert!(
                Instant::now() < deadline,
                "thumbnail worker did not complete"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn completed_previews_retain_only_bounded_image_charges_until_hidden() {
        let ctx = egui::Context::default();
        let budget = paintcraft_runtime::SharedBudget::new(4 << 20);
        let doc = Arc::new(new_document(1000, 486, Some(Rgba([255; 4])), 0, POLICY).unwrap());
        let id = doc.roots[0];
        let before = doc.clone();
        let mut service = LayerThumbnails::default();
        service.begin(&ctx, &doc, &budget);
        service.request(&ctx, doc.clone(), &[id], &budget);
        drain(&mut service, &ctx, &doc, &budget);
        assert!(matches!(service.preview(id), Preview::Ready { .. }));
        assert!(
            service.cache[&id]
                .texture
                .size()
                .into_iter()
                .all(|size| size <= 64)
        );
        assert!(budget.usage().reserved > 0 && budget.usage().reserved <= 64 * 64 * 8);
        assert!(doc.same_semantics(&before));
        assert!(service.cache[&id].uv.max.x <= 1.0 && service.cache[&id].uv.max.y <= 1.0);
        let mut portrait = (*doc).clone();
        portrait.state_id = Uuid::new_v4();
        portrait.canvas = (3, 5);
        service.begin(&ctx, &portrait, &budget);
        assert!(
            service.cache.is_empty(),
            "an old-state texture must not be painted"
        );
        assert_eq!(budget.usage().reserved, 0);
        let portrait = Arc::new(portrait);
        service.request(&ctx, portrait.clone(), &[id], &budget);
        drain(&mut service, &ctx, &portrait, &budget);
        assert_eq!(service.cache[&id].aspect, 3.0 / 5.0);
        service.request(&ctx, portrait, &[], &budget);
        assert_eq!(budget.usage().reserved, 0);
        assert!(service.cache.is_empty());
    }

    #[test]
    fn stale_worker_cannot_publish_and_keeps_its_charge_until_drained() {
        let ctx = egui::Context::default();
        let budget = paintcraft_runtime::SharedBudget::new(4 << 20);
        let doc = Arc::new(new_document(5, 3, Some(Rgba([255; 4])), 0, POLICY).unwrap());
        let id = doc.roots[0];
        let mut service = LayerThumbnails::default();
        service.begin(&ctx, &doc, &budget);
        let mut executor = Executor::with_budget(
            WorkerLimits {
                workers: 1,
                pending_jobs: 1,
                transient_bytes: WORK_BYTES,
            },
            budget.clone(),
        )
        .unwrap();
        let (release, gate) = std::sync::mpsc::channel();
        let (started, running) = std::sync::mpsc::channel();
        let source = doc.clone();
        let handle = executor
            .submit(
                WORK_BYTES,
                move |_| {
                    started.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(5)).unwrap();
                    paintcraft_render::thumbnail::render_layer(
                        &source,
                        &source.layers[&id],
                        SIDE,
                        || Ok(()),
                    )
                },
                || {},
            )
            .unwrap();
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        service.executor = Some(executor);
        service.pending = Some(((doc.document_id, doc.state_id), id, handle));
        let mut newer = (*doc).clone();
        newer.state_id = Uuid::new_v4();
        service.begin(&ctx, &newer, &budget);
        assert_eq!(budget.usage().reserved, WORK_BYTES);
        release.send(()).unwrap();
        drain(&mut service, &ctx, &newer, &budget);
        assert!(service.cache.is_empty());
        assert!(service.failed.is_empty());
        assert_eq!(budget.usage().reserved, 0);
        // New source remains eligible after an old source was cancelled.
        service.request(&ctx, Arc::new(newer.clone()), &[id], &budget);
        drain(&mut service, &ctx, &newer, &budget);
        assert!(matches!(service.preview(id), Preview::Ready { .. }));
    }
}
