//! Original singleton host-control credit; close never waits on its own guard.
use super::super::{bridge, controls, denied, display, js};
use js_sys::Function;
use paintcraft_core::types::Result;
use std::{
    cell::Cell,
    future::Future,
    mem::size_of,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use wasm_bindgen::{JsCast, JsValue};

#[derive(Clone, Copy, Debug)]
pub(in crate::app::filter_editing::appearance8) enum Slot8 {
    Open,
    Close,
    Cleanup,
}
impl Slot8 {
    fn index(self) -> usize {
        match self {
            Self::Open => 0,
            Self::Close => 1,
            Self::Cleanup => 2,
        }
    }
    fn limit(self) -> u8 {
        match self {
            Self::Cleanup => 2,
            Self::Open | Self::Close => 1,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct HostQuote8 {
    open: u64,
    close: u64,
    cleanup: u64,
    pub total: u64,
}
#[derive(Debug)]
struct HostData8 {
    credit: Option<JsValue>,
    quote: HostQuote8,
    live: [Cell<u8>; 3],
}
impl Drop for HostData8 {
    fn drop(&mut self) {
        if let Some(credit) = self.credit.take()
            && js::defer_credit(&credit).is_err()
        {
            // No synchronous fallback or replacement grant on scheduling failure.
            std::mem::forget(credit);
        }
    }
}
pub(in crate::app::filter_editing::appearance8) struct Host8(Option<Rc<HostData8>>);
impl std::fmt::Debug for Host8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainHostControl8").finish_non_exhaustive()
    }
}
impl Clone for Host8 {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for Host8 {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take()
            && let Some(data) = Rc::into_inner(owner)
        {
            // Shared block first, boxed application F earlier, deferred guard last.
            drop(data);
        }
    }
}
#[derive(Debug)]
struct Lease8 {
    host: Host8,
    slot: Slot8,
}
impl Drop for Lease8 {
    fn drop(&mut self) {
        let active = &self.host.data().live[self.slot.index()];
        active.set(active.get().saturating_sub(1));
    }
}
struct HostFuture8<F: Future> {
    inner: Pin<Box<F>>,
    _lease: Lease8,
}
impl<F: Future> Future for HostFuture8<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.as_mut().poll(cx)
    }
}
fn shape<A, F: Future>(_: fn(A) -> F) -> Result<u64> {
    (size_of::<F>() as u64)
        .checked_add(size_of::<HostFuture8<F>>() as u64)
        .ok_or_else(denied)
}
fn quote() -> Result<HostQuote8> {
    let open = shape(display::open_task8)?;
    let close = shape(display::close_task8)?;
    let cleanup = shape(bridge::cleanup_task8)?;
    let mut total = 0u64;
    for value in [
        open,
        close,
        cleanup,
        cleanup,
        controls::shared::<HostData8>()?,
        display::control_bytes()?,
        bridge::control_bytes()?,
        6 * size_of::<Host8>() as u64,
        4 * size_of::<Result<Host8>>() as u64,
        4 * size_of::<Result<HostQuote8>>() as u64,
    ] {
        total = total.checked_add(value).ok_or_else(denied)?;
    }
    if !matches!(size_of::<usize>(), 4 | 8) || total > super::super::factory::MAX {
        return Err(denied());
    }
    Ok(HostQuote8 {
        open,
        close,
        cleanup,
        total,
    })
}
impl Host8 {
    fn data(&self) -> &HostData8 {
        self.0.as_ref().expect("live host control")
    }
    pub(in crate::app::filter_editing::appearance8) fn begin() -> Result<(JsValue, Self)> {
        let quote = quote()?;
        // The defining C quote is admitted before Worker construction, slots or F.
        let pair = js::begin_host(quote.total as f64).map_err(js::error)?;
        let controller = js::data(&pair, "controller")?;
        let credit = js::data(&pair, "hostControl")?;
        if js::data(&credit, "byteLength")?.as_f64() != Some(quote.total as f64)
            || !js::data(&credit, "release")?.is_instance_of::<Function>()
        {
            // Invalid trusted handshake cannot manufacture a replacement credit.
            std::mem::forget(pair);
            return Err(denied());
        }
        Ok((
            controller,
            Self(Some(Rc::new(HostData8 {
                credit: Some(credit),
                quote,
                live: [Cell::new(0), Cell::new(0), Cell::new(0)],
            }))),
        ))
    }
    pub(in crate::app::filter_editing::appearance8) fn spawn<A, F>(
        &self,
        slot: Slot8,
        args: A,
        make: fn(A) -> F,
    ) -> std::result::Result<(), (paintcraft_core::types::Error, A)>
    where
        F: Future<Output = ()> + 'static,
    {
        let accepted = (|| {
            let actual = shape(make)?;
            let expected = match slot {
                Slot8::Open => self.data().quote.open,
                Slot8::Close => self.data().quote.close,
                Slot8::Cleanup => self.data().quote.cleanup,
            };
            let active = &self.data().live[slot.index()];
            if actual != expected || active.get() >= slot.limit() {
                return Err(denied());
            }
            active.set(active.get() + 1);
            Ok(Lease8 {
                host: self.clone(),
                slot,
            })
        })();
        let lease = match accepted {
            Ok(lease) => lease,
            Err(e) => return Err((e, args)),
        };
        wasm_bindgen_futures::spawn_local(HostFuture8 {
            inner: Box::pin(make(args)),
            _lease: lease,
        });
        Ok(())
    }
    pub(in crate::app::filter_editing::appearance8) fn quarantine(self) {
        // The broker remains busy because the actual host guard cannot reach0.
        std::mem::forget(self);
    }
}
