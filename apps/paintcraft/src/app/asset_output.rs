//! Noise-only asset jobs. Old synchronous non-Noise routes remain unchanged.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, resources::BudgetLease};
use paintcraft_document::Document;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
#[path = "asset_output/write.rs"]
mod write;
pub(super) use write::{WriteEvent, WriteJob};
#[derive(Clone)]
pub(super) struct Owner {
    source: Arc<Document>,
    _lease: BudgetLease,
}
struct Shared<T>(Option<Arc<T>>);
impl<T> Shared<T> {
    fn new(value: T) -> Self {
        Self(Some(Arc::new(value)))
    }
}
impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Some(self.0.as_ref().unwrap().clone()))
    }
}
impl<T> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_ref().unwrap()
    }
}
impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        if let Some(arc) = self.0.take()
            && let Some(value) = Arc::into_inner(arc)
        {
            drop(value);
        }
    }
}
/// Every cloned handle carries the same paired ownership. No Weak/raw Arc escapes.
/// Fields drop value allocation first, then retention allocation/source/lease.
pub(super) struct Prepared<P> {
    value: Shared<P>,
    owner: Option<Shared<Owner>>,
}
impl<P> Prepared<P> {
    pub(super) fn legacy(value: P) -> Self {
        Self {
            value: Shared::new(value),
            owner: None,
        }
    }
}
impl<P> Clone for Prepared<P> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            owner: self.owner.clone(),
        }
    }
}
impl<P> std::ops::Deref for Prepared<P> {
    type Target = P;
    fn deref(&self) -> &P {
        &self.value
    }
}
enum Event<P> {
    Prepared(Prepared<P>, Owner),
    Encoded(Result<Vec<u8>>, Owner),
}
struct Pending<P> {
    receiver: mpsc::Receiver<Event<P>>,
    encode: mpsc::SyncSender<Vec<String>>,
    cancel: Arc<AtomicBool>,
    requested: bool,
}
impl<P> Drop for Pending<P> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
pub(super) enum Update<P> {
    Prepared(Prepared<P>),
    Encoded(Vec<u8>),
}
pub(super) struct Request<'a, C> {
    pub config: &'a C,
    pub omit: bool,
    pub bake: bool,
    pub source: Arc<Document>,
    pub lease: BudgetLease,
}
struct Key<C> {
    config: C,
    omit: bool,
    bake: bool,
}
/// The dialog declares cache/bytes BEFORE this owner, so they drop first.
pub(super) struct Async<C, P> {
    key: Option<Key<C>>,
    pending: Option<Pending<P>>,
    hold: Option<Owner>,
}
impl<C, P> Default for Async<C, P> {
    fn default() -> Self {
        Self {
            key: None,
            pending: None,
            hold: None,
        }
    }
}
pub(super) fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Native asset preparation limit")
}
pub(super) fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(Error::new(
            ErrorCode::OperationCancelled,
            "Native asset preparation cancelled",
        ))
    } else {
        Ok(())
    }
}
/// Independent payload reservation, not opaque encoder allocator/RSS accounting.
/// Source/entry bounds are the EXISTING icon preparation caps, never larger.
pub(super) fn quote<P>(
    doc: &Document,
    sizes: impl IntoIterator<Item = (u32, u32)>,
    encoded_max: u64,
) -> Result<u64> {
    let source_pixels = u64::from(doc.canvas.0)
        .checked_mul(u64::from(doc.canvas.1))
        .filter(|n| *n <= paintcraft_formats::icons::WRITE_PIXELS_MAX)
        .ok_or_else(limit)?;
    let mut count = 0usize;
    let mut pixels = 0u64;
    for (w, h) in sizes {
        count += 1;
        if count > 256 || w == 0 || h == 0 {
            return Err(limit());
        }
        pixels = pixels
            .checked_add(u64::from(w) * u64::from(h))
            .filter(|n| *n <= paintcraft_formats::icons::WRITE_PIXELS_MAX)
            .ok_or_else(limit)?;
    }
    if count == 0 {
        return Err(limit());
    }
    // Cumulative entry RGBA, sampler/intermediate, all possible UI texture copies;
    // two encoded capacity allowances for per-entry buffers and final container.
    let control = (std::mem::size_of::<P>()
        + std::mem::size_of::<Owner>()
        + 4 * std::mem::size_of::<usize>()) as u64;
    let mut bytes = (64u64 << 20)
        .checked_add(1 << 20)
        .and_then(|n| n.checked_add(control))
        .and_then(|n| n.checked_add(source_pixels.checked_mul(24)?))
        .and_then(|n| n.checked_add(pixels.checked_mul(24)?))
        .and_then(|n| n.checked_add(encoded_max.checked_mul(4)?))
        .ok_or_else(limit)?;
    for layer in doc.layers.values() {
        bytes = bytes
            .checked_add(
                (layer.raster.resident_tiles() as u64)
                    .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
                    .ok_or_else(limit)?,
            )
            .and_then(|n| n.checked_add(layer.filters.storage_bytes() as u64))
            .and_then(|n| n.checked_add(layer.name.len() as u64 + 4096))
            .ok_or_else(limit)?;
        for (_, mask_bytes) in layer.filters.mask_storage_usage() {
            bytes = bytes.checked_add(mask_bytes).ok_or_else(limit)?;
        }
    }
    bytes
        .checked_add(
            doc.source_metadata
                .as_ref()
                .map_or(0, |m| m.storage_usage().1),
        )
        .ok_or_else(limit)
}
impl<C: Clone + PartialEq, P: Send + Sync + 'static> Async<C, P> {
    pub(super) fn matches(&self, config: &C, omit: bool, bake: bool) -> bool {
        self.key
            .as_ref()
            .is_some_and(|k| &k.config == config && k.omit == omit && k.bake == bake)
    }
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }
    /// Caller clears Prepared/bytes/AssetCopy's cached config BEFORE this call.
    pub(super) fn retire(&mut self) {
        self.key = None;
        self.hold = None;
        if let Some(p) = &self.pending {
            p.cancel.store(true, Ordering::Release);
        }
    }
    pub(super) fn retention(&self) -> Result<Owner> {
        self.hold.clone().ok_or_else(limit)
    }
    pub(super) fn request_encode(&mut self, losses: &[String]) -> Result<()> {
        let pending = self.pending.as_mut().ok_or_else(limit)?;
        if !pending.requested {
            pending
                .encode
                .try_send(losses.to_vec())
                .map_err(|_| limit())?;
            pending.requested = true;
        }
        Ok(())
    }
    pub(super) fn poll(&mut self) -> Option<Result<Update<P>>> {
        let pending = self.pending.take()?;
        match pending.receiver.try_recv() {
            Ok(Event::Prepared(value, owner)) => {
                if pending.cancel.load(Ordering::Acquire) || self.key.is_none() {
                    self.pending = Some(pending);
                    return None;
                }
                self.hold = Some(owner);
                self.pending = Some(pending);
                Some(Ok(Update::Prepared(value)))
            }
            Ok(Event::Encoded(result, owner)) => {
                if pending.cancel.load(Ordering::Acquire) || self.key.is_none() {
                    return None;
                }
                self.hold = Some(owner);
                Some(result.map(Update::Encoded))
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.pending = Some(pending);
                None
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.key = None;
                Some(Err(limit()))
            }
        }
    }
    pub(super) fn start(
        &mut self,
        request: Request<'_, C>,
        ctx: &egui::Context,
        prepare: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<P> + Send + 'static,
        encode: impl FnOnce(&P, &[String], &dyn Fn() -> Result<()>) -> Result<Vec<u8>> + Send + 'static,
    ) -> Result<()> {
        if self.pending.is_some() {
            return Err(limit());
        }
        let Request {
            config,
            omit,
            bake,
            source,
            lease,
        } = request;
        self.key = Some(Key {
            config: config.clone(),
            omit,
            bake,
        });
        let policy = spatial_workspace::JobPolicy::capture(
            &source,
            None,
            spatial_workspace::needed(&source),
        );
        let owner = Owner {
            source,
            _lease: lease,
        };
        // Failed spawn/control keys still retain their declaration until the
        // dialog clears its AssetCopy/config/cache, before retire drops this.
        self.hold = Some(owner.clone());
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let wake = ctx.clone();
        let (sender, receiver) = mpsc::channel();
        let (encode_sender, wait) = mpsc::sync_channel::<Vec<String>>(1);
        std::thread::Builder::new()
            .name("paintcraft-asset-prepare".into())
            .spawn(move || {
                let result = policy.run(|| {
                    let check = || check(&flag);
                    check()?;
                    let prepared = Prepared {
                        value: Shared::new(prepare(&owner.source, &check)?),
                        owner: Some(Shared::new(owner.clone())),
                    };
                    check()?;
                    sender
                        .send(Event::Prepared(prepared.clone(), owner.clone()))
                        .map_err(|_| limit())?;
                    wake.request_repaint();
                    let losses = loop {
                        check()?;
                        match wait.recv_timeout(Duration::from_millis(25)) {
                            Ok(losses) => break losses,
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(limit()),
                        }
                    };
                    check()?;
                    let bytes = encode(&prepared, &losses, &check)?;
                    check()?;
                    if bytes.len() > 64 << 20 || bytes.capacity() > 128 << 20 {
                        return Err(limit());
                    }
                    Ok(bytes)
                });
                let _ = sender.send(Event::Encoded(result, owner));
                wake.request_repaint();
            })
            .map_err(|e| Error::new(ErrorCode::ResourceLimit, e.to_string()))?;
        self.pending = Some(Pending {
            receiver,
            encode: encode_sender,
            cancel,
            requested: false,
        });
        Ok(())
    }
}
#[cfg(test)]
#[path = "asset_output/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "asset_output/write_tests.rs"]
mod write_tests;
