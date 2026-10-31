//! One source client, one in-flight request, and one replaceable desired request.
#[cfg(target_arch = "wasm32")]
use super::bridge;
use super::state::Revision;
use eframe::egui;
#[cfg(target_arch = "wasm32")]
use paintcraft_core::types::Result;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_document::Document;
use paintcraft_editor::session::Ticket;
use paintcraft_tools::FilterEdit;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(super) struct Capture {
    pub owner: Uuid,
    pub ticket: Ticket,
    pub draft: Uuid,
    pub document: Arc<Document>,
    pub spatial: bool,
}
impl Capture {
    pub fn same(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.ticket == other.ticket
            && self.draft == other.draft
            && Arc::ptr_eq(&self.document, &other.document)
            && self.spatial == other.spatial
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct View {
    pub visible: paintcraft_core::types::Rect,
    pub region: paintcraft_core::types::Rect,
    pub footprint: paintcraft_core::types::Rect,
    pub level: u8,
    pub size: [u32; 2],
}
impl View {
    pub fn new(
        doc: &Document,
        visible: paintcraft_core::types::Rect,
        level: u8,
    ) -> paintcraft_core::types::Result<Self> {
        let request = paintcraft_render::display::DisplayRequest::new(doc, visible, level)?;
        Ok(Self {
            visible,
            region: request.region(),
            footprint: request.footprint(),
            level,
            size: request.size(),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Request {
    pub revision: Revision,
    pub edit: Option<FilterEdit>,
    pub render: bool,
    pub view: View,
}
#[derive(Debug)]
pub(super) struct Reply {
    pub request: Request,
    pub size: (u32, u32),
    pub pixels: Option<Vec<u8>>,
}
#[derive(Clone, Debug)]
struct Work {
    capture: Capture,
    request: Request,
}
impl Work {
    fn same(&self, other: &Self) -> bool {
        self.capture.same(&other.capture) && self.request == other.request
    }
}
#[cfg(target_arch = "wasm32")]
type Outcome<T> = Rc<RefCell<Option<Result<T>>>>;

#[derive(Default, Debug)]
pub(super) struct Renderer {
    wanted: Option<Work>,
    pub accepted: Option<Request>,
    pub error: Option<(Revision, Error)>,
    blocked: bool,
    #[cfg(target_arch = "wasm32")]
    live: Option<Capture>,
    #[cfg(target_arch = "wasm32")]
    opening: Option<(Outcome<bridge::Client>, bridge::CancelSlot)>,
    #[cfg(target_arch = "wasm32")]
    client: Option<bridge::Client>,
    #[cfg(target_arch = "wasm32")]
    flight: Option<(Request, Outcome<Reply>, bool, u64)>,
    #[cfg(target_arch = "wasm32")]
    sequence: u64,
    #[cfg(target_arch = "wasm32")]
    closing: Option<Outcome<()>>,
}
impl Renderer {
    pub fn want(&mut self, mut capture: Capture, request: Request) {
        capture.spatial |= request.edit.as_ref().is_some_and(|edit| matches!(&edit.operation,paintcraft_commands::FilterOp::Add{descriptor,..}|paintcraft_commands::FilterOp::Update{descriptor,..} if descriptor.is_spatial()));
        let work = Work { capture, request };
        if self.wanted.as_ref().is_none_or(|w| !w.same(&work)) {
            self.accepted = None;
            self.error = None;
            self.wanted = Some(work);
        }
    }
    pub fn reset(&mut self) {
        self.wanted = None;
        self.accepted = None;
        self.error = None;
    }
    pub fn retry(&mut self) {
        if !self.blocked && self.error.is_some() {
            self.error = None;
            self.accepted = None;
        }
    }
    pub fn ready(&self, request: &Request) -> bool {
        self.accepted.as_ref() == Some(request)
    }
    pub fn busy(&self) -> bool {
        self.wanted.is_some() && self.accepted.is_none() && self.error.is_none()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn poll(&mut self, _ctx: &egui::Context) -> Option<Reply> {
        if let Some(work) = &self.wanted
            && self.error.is_none()
            && !self.ready(&work.request)
        {
            self.error = Some((
                work.request.revision,
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Filter preview Worker is available in the browser",
                ),
            ));
        }
        None
    }
    #[cfg(target_arch = "wasm32")]
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<Reply> {
        if let Some(closing) = &self.closing {
            let result = closing.borrow_mut().take();
            let result = result?;
            self.closing = None;
            self.live = None;
            self.sequence = 0;
            if let Err(error) = result {
                self.blocked = true;
                if let Some(work) = &self.wanted {
                    self.error = Some((work.request.revision, error));
                }
                return None;
            }
        }
        if self.blocked {
            return None;
        }
        let changed = self.live.as_ref().is_some_and(|source| {
            self.wanted
                .as_ref()
                .is_none_or(|w| !source.same(&w.capture))
        });
        if changed {
            if let Some((_, slot)) = &self.opening
                && let Some(cancel) = slot.borrow().as_ref()
            {
                cancel.cancel();
            }
            if let Some((_, _, cancelled, sequence)) = &mut self.flight
                && !*cancelled
            {
                if let Some(client) = &self.client {
                    client.cancel(*sequence);
                }
                *cancelled = true;
            }
        }
        if let Some((opening, _)) = &self.opening {
            let result = opening.borrow_mut().take();
            let result = result?;
            {
                self.opening = None;
                match result {
                    Ok(client) => self.client = Some(client),
                    Err(error) => {
                        self.live = None;
                        if !changed && let Some(work) = &self.wanted {
                            self.error = Some((work.request.revision, error));
                        }
                    }
                }
            }
        }
        if let Some((request, outcome, cancelled, sequence)) = &mut self.flight {
            if !changed && self.wanted.as_ref().is_none_or(|w| w.request != *request) && !*cancelled
            {
                if let Some(client) = &self.client {
                    client.cancel(*sequence);
                }
                *cancelled = true;
            }
            let result = outcome.borrow_mut().take();
            let result = result?;
            {
                let request = request.clone();
                self.flight = None;
                if !changed && self.wanted.as_ref().is_some_and(|w| w.request == request) {
                    match result {
                        Ok(reply) => {
                            self.accepted = Some(reply.request.clone());
                            return Some(reply);
                        }
                        Err(error) => {
                            self.error = Some((request.revision, error));
                        }
                    }
                }
            }
        }
        if changed {
            if let Some(client) = self.client.take() {
                let outcome = Rc::new(RefCell::new(None));
                self.closing = Some(outcome.clone());
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    *outcome.borrow_mut() = Some(client.close().await);
                    ctx.request_repaint();
                });
                return None;
            }
            self.live = None;
        }
        let work = self.wanted.clone()?;
        if self.ready(&work.request)
            || self
                .error
                .as_ref()
                .is_some_and(|(r, _)| *r == work.request.revision)
        {
            return None;
        }
        if self.live.is_none() {
            self.live = Some(work.capture.clone());
            let outcome = Rc::new(RefCell::new(None));
            let slot = Rc::new(RefCell::new(None));
            self.opening = Some((outcome.clone(), slot.clone()));
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                *outcome.borrow_mut() = Some(bridge::open(work.capture, slot).await);
                ctx.request_repaint();
            });
        } else if let Some(client) = &self.client {
            let Some(sequence) = self.sequence.checked_add(1) else {
                self.blocked = true;
                self.error = Some((
                    work.request.revision,
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "Filter preview generation exhausted",
                    ),
                ));
                return None;
            };
            self.sequence = sequence;
            let task = client.run(work.request.clone(), sequence);
            let outcome = Rc::new(RefCell::new(None));
            self.flight = Some((work.request, outcome.clone(), false, sequence));
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                *outcome.borrow_mut() = Some(task.await);
                ctx.request_repaint();
            });
        }
        None
    }
    #[cfg(test)]
    pub fn accept_test(&mut self, request: Request) {
        self.accepted = Some(request);
        self.error = None;
    }
}

/// Stored descriptors select the source capability even disabled or radius-zero.
pub(super) fn stored_spatial(doc: &Document) -> u64 {
    doc.layers
        .values()
        .map(|layer| {
            layer
                .filters
                .iter()
                .filter(|filter| filter.descriptor().is_spatial())
                .count() as u64
        })
        .sum()
}

#[cfg(test)]
#[path = "render/spatial_tests.rs"]
mod spatial_tests;
