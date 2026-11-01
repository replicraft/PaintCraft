//! One coordinator dispatches separate legacy and Source7 clients, never a fallback.
#[cfg(target_arch = "wasm32")]
use super::render::{Reply, Request};
use super::{appearance, render::Capture};
use paintcraft_commands::FilterOp;
use paintcraft_core::types::Result;
#[cfg(any(target_arch = "wasm32", test))]
use paintcraft_core::types::{Error, ErrorCode};
#[cfg(any(target_arch = "wasm32", test))]
use std::{cell::Cell, rc::Rc};
#[derive(Clone, Debug)]
// Keep captured ownership inline until a lease admits the preparation payload.
#[allow(clippy::large_enum_variant)]
pub(super) enum SourceKey {
    Legacy(Capture),
    Appearance(appearance::Key7),
}
impl SourceKey {
    pub fn new(
        capture: Capture,
        operation: Option<&FilterOp>,
        spec: Option<&paintcraft_core::types::json::Value>,
    ) -> Result<Self> {
        if capture.document.has_layer_masks() {
            Ok(Self::Appearance(appearance::Key7::new(
                capture, operation, spec,
            )?))
        } else {
            Ok(Self::Legacy(capture))
        }
    }
    #[cfg(test)]
    pub fn legacy(&self) -> &Capture {
        match self {
            Self::Legacy(capture) => capture,
            Self::Appearance(_) => panic!("Expected the unchanged legacy profile"),
        }
    }
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Legacy(a), Self::Legacy(b)) => a.same(b),
            (Self::Appearance(a), Self::Appearance(b)) => a.same(b),
            _ => false,
        }
    }
}
#[derive(Clone, Debug, Default)]
#[cfg(any(target_arch = "wasm32", test))]
pub(super) struct CancelSlot {
    stopped: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    legacy: super::bridge::CancelSlot,
    #[cfg(target_arch = "wasm32")]
    appearance: appearance::CancelSlot7,
}
#[cfg(any(target_arch = "wasm32", test))]
impl CancelSlot {
    pub fn with_flag(stopped: Rc<Cell<bool>>) -> Self {
        Self {
            stopped,
            #[cfg(target_arch = "wasm32")]
            legacy: Default::default(),
            #[cfg(target_arch = "wasm32")]
            appearance: Default::default(),
        }
    }
    pub fn cancel(&self) {
        self.stopped.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(cancel) = self.legacy.borrow().as_ref() {
                cancel.cancel();
            }
            if let Some(cancel) = self.appearance.borrow().as_ref() {
                cancel.cancel();
            }
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn check(&self) -> Result<()> {
        if self.stopped.get() {
            return Err(Error::new(
                ErrorCode::OperationCancelled,
                "Preview opening cancelled",
            ));
        }
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub(super) enum Client {
    Legacy(super::bridge::Client),
    Appearance(appearance::Client7),
}
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub(super) enum CopyControl {
    Opening(CancelSlot),
    Legacy(super::bridge::CopyControl),
    Appearance(appearance::CopyControl7),
}
#[cfg(target_arch = "wasm32")]
impl CopyControl {
    pub fn cancel(&self) {
        match self {
            Self::Opening(v) => v.cancel(),
            Self::Legacy(v) => v.cancel(),
            Self::Appearance(v) => v.cancel(),
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) struct EncodedCopy {
    pub bytes: Vec<u8>,
    pub mime: String,
}
#[cfg(target_arch = "wasm32")]
pub(super) async fn open(
    source: SourceKey,
    cancel: CancelSlot,
    admission: paintcraft_raster::Admission,
) -> Result<Client> {
    cancel.check()?;
    match source {
        SourceKey::Legacy(capture) => super::bridge::open(capture, cancel.legacy)
            .await
            .map(Client::Legacy),
        SourceKey::Appearance(key) => {
            appearance::open(key, cancel.appearance, admission, cancel.stopped)
                .await
                .map(Client::Appearance)
        }
    }
}
#[cfg(target_arch = "wasm32")]
impl Client {
    pub fn cancel(&self, generation: u64) {
        match self {
            Self::Legacy(v) => v.cancel(generation),
            Self::Appearance(v) => v.cancel(generation),
        }
    }
    pub async fn close(self) -> Result<()> {
        match self {
            Self::Legacy(v) => v.close().await,
            Self::Appearance(v) => v.close().await,
        }
    }
    pub fn run(
        &self,
        request: Request,
        generation: u64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Reply>>>> {
        match self {
            Self::Legacy(v) => Box::pin(v.run(request, generation)),
            Self::Appearance(v) => Box::pin(v.run(request, generation)),
        }
    }
    pub fn copy_control(&self) -> CopyControl {
        match self {
            Self::Legacy(v) => CopyControl::Legacy(v.copy_control()),
            Self::Appearance(v) => CopyControl::Appearance(v.copy_control()),
        }
    }
    pub async fn encode_copy(
        &self,
        region: paintcraft_core::types::Rect,
        spec: &paintcraft_core::types::json::Value,
        mask: &[u8],
    ) -> Result<EncodedCopy> {
        match self {
            Self::Legacy(v) => v
                .encode_copy(region, spec, mask)
                .await
                .map(|v| EncodedCopy {
                    bytes: v.bytes,
                    mime: v.mime,
                }),
            Self::Appearance(v) => v
                .encode_copy(region, spec, mask)
                .await
                .map(|v| EncodedCopy {
                    bytes: v.bytes,
                    mime: v.mime,
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_remains_sticky_even_before_a_transport_handle_exists() {
        let flag = Rc::new(Cell::new(false));
        let slot = CancelSlot::with_flag(flag.clone());
        let other = slot.clone();
        flag.set(true); // Frozen owner may invalidate before RPC exists.
        slot.cancel();
        assert_eq!(
            other.check().unwrap_err().code,
            ErrorCode::OperationCancelled
        );
        other.cancel();
        assert!(flag.get());
    }
}
