//! Source7-only Main transport; all legacy bridge bodies remain separate.
use super::{
    capture::Capture7,
    control, echo,
    joined::Joined7,
    leases::{self, Credit7, Retained7},
    preparation::Preparation7,
    prepare::{self, Prepared7},
    preplan,
    runtime::Runtime7,
};
use crate::app::filter_editing::render::{Reply, Request};
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value, policy::hex};
use paintcraft_raster::Admission;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue};
mod copy;
mod js;
pub(in crate::app::filter_editing) use copy::CopyControl7;
use js::*;
#[wasm_bindgen::prelude::wasm_bindgen(module = "/src/lib/filter-preview-appearance-bridge.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch,js_name=beginLayerAppearancePreview)]
    fn begin(plan: &JsValue) -> std::result::Result<JsValue, JsValue>;
}
#[derive(Clone, Debug)]
pub(in crate::app::filter_editing) struct Cancellation7 {
    value: JsValue,
    stopped: Rc<Cell<bool>>,
    sent: Rc<Cell<bool>>,
}
impl Cancellation7 {
    pub fn cancel(&self) {
        self.stopped.set(true);
        if !self.sent.replace(true) {
            let _ = call(&self.value, "cancel", &[]);
        }
    }
}
pub(in crate::app::filter_editing) type CancelSlot7 = Rc<RefCell<Option<Cancellation7>>>;
struct Pending7 {
    value: JsValue,
    consumed: bool,
    context: Option<Retained7<Prepared7>>,
}
impl Drop for Pending7 {
    fn drop(&mut self) {
        if !self.consumed {
            let value = self.value.clone();
            let owner = Joined7::new(self.context.take());
            wasm_bindgen_futures::spawn_local(async move {
                if close_ack(&value).await.is_ok() {
                    owner.acknowledged();
                }
            });
        }
    }
}
#[derive(Debug)]
pub(in crate::app::filter_editing) struct Client7 {
    value: JsValue,
    context: Option<Rc<Retained7<Prepared7>>>,
    closed: bool,
}
impl Drop for Client7 {
    fn drop(&mut self) {
        if !self.closed {
            let value = self.value.clone();
            let owner = Joined7::new(self.context.take());
            wasm_bindgen_futures::spawn_local(async move {
                if close_ack(&value).await.is_ok() {
                    owner.acknowledged();
                }
            });
        }
    }
}
pub(in crate::app::filter_editing) async fn open(
    capture: Capture7,
    prefix: Preparation7,
    admission: Admission,
    cancel: CancelSlot7,
    stopped: Rc<Cell<bool>>,
    copy_spec: Option<Value>,
) -> Result<Client7> {
    let plan = value(&preplan::encode(&capture)?)?;
    let mut pending = Pending7 {
        value: begin(&plan).map_err(error)?,
        consumed: false,
        context: None,
    };
    *cancel.borrow_mut() = Some(Cancellation7 {
        value: pending.value.clone(),
        stopped: stopped.clone(),
        sent: Rc::new(Cell::new(false)),
    });
    let ready = promise(field(&pending.value, "ready")?).await?;
    let runtime = Runtime7::parse(
        &parse(&ready)?,
        capture.admission.mode == paintcraft_formats::export::layer_appearance::Mode7::Noise,
    )?;
    let check = || {
        if stopped.get() {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "Source7 preparation cancelled",
            ))
        } else {
            Ok(())
        }
    };
    check()?;
    // Actual compiled fixed control quote was admitted by the same broker.
    // It is not a second Main credit or variable payload allowance.
    if runtime.control_bytes > leases::MAIN_PREPARATION_MAX {
        return Err(bad("Source7 runtime control quote"));
    }
    let bytes = (admission.paint_metadata_limit() as u64).min(leases::MAIN_PREPARATION_MAX);
    if bytes == 0 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Source7 Main byte allowance",
        ));
    }
    let credit = Credit7::new(call(
        &pending.value,
        "reserveMain",
        &[JsValue::from_f64(bytes as f64)],
    )?)?;
    let scope = leases::reserved_scope(&admission, bytes)?;
    let prepared = prepare::prepare(capture, prefix, admission, &scope, &check, copy_spec)?;
    pending.context = Some(Retained7::new(prepared, scope, credit));
    let prepared = pending
        .context
        .as_mut()
        .expect("attached Main owner")
        .get_mut();
    let wrapper = control::final_wrapper(prepared)?;
    let expected_plan = wrapper
        .get("plan")
        .ok_or_else(|| bad("Source7 final plan"))?;
    let snapshot = prepared
        .snapshot
        .as_ref()
        .ok_or_else(|| bad("Source7 source snapshot"))?;
    // Safe copy uses the already-admitted broker's separate forward-source64
    // slice, not a second Main metadata account or unsafe Wasm-memory view.
    let source = js_sys::Uint8Array::from(snapshot.bytes());
    let source_sha = hex(&prepared.source_sha256);
    drop(prepared.snapshot.take());
    let task = call(
        &pending.value,
        "open",
        &[source.into(), source_sha.into(), value(&wrapper)?],
    )?;
    let session = promise(task).await?;
    check()?;
    if &parse(&field(&session, "binding")?)? != expected_plan {
        return Err(bad("Source7 OPEN binding changed"));
    }
    // No source cache. Changed intent must reencode under a new admitted job.
    let context = pending.context.take().map(Rc::new);
    pending.consumed = true;
    Ok(Client7 {
        value: session,
        context,
        closed: false,
    })
}
impl Client7 {
    pub fn cancel(&self, generation: u64) {
        if let Ok(task) = call(&self.value, "cancel", &[generation.to_string().into()]) {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = promise(task).await;
            });
        }
    }
    pub async fn close(mut self) -> Result<()> {
        self.closed = true; // one RPC, no Drop retry/reset on failure
        let owner = Joined7::new(self.context.take());
        close_ack(&self.value).await?;
        owner.acknowledged();
        Ok(())
    }
    pub fn run(
        &self,
        request: Request,
        generation: u64,
    ) -> impl std::future::Future<Output = Result<Reply>> + 'static {
        let session = self.value.clone();
        let context = self.context.as_ref().expect("open Source7 context").clone();
        async move {
            let prepared = context.get();
            control::same_operation(prepared, request.edit.as_ref().map(|e| &e.operation))?;
            if request.edit.as_ref().is_some_and(|e| {
                !e.matches_base(
                    prepared.capture.owner,
                    prepared.capture.public_ticket.state_id,
                    prepared.capture.public_ticket.publication_epoch,
                )
            }) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Source7 public filter intent changed",
                ));
            }
            let begin = control::begin(prepared, generation, request.view, request.render)?;
            let result = promise(call(
                &session,
                if prepared.capture.operation.is_some() {
                    "update"
                } else {
                    "base"
                },
                &[value(&begin)?],
            )?)
            .await?;
            let parsed = (|| {
                let metadata = frame_metadata(&result, request.render)?;
                let expected = echo::expected(
                    &prepared.capture,
                    &prepared.frames,
                    &prepared.source_sha256,
                    &prepared.operation_sha256,
                    generation,
                    request.view,
                    request.render,
                );
                echo::validate(&metadata, &expected, prepared.frames.sealed.admission)?;
                let pixels = if request.render {
                    let array = field(&result, "pixels")?
                        .dyn_into::<js_sys::Uint8Array>()
                        .map_err(error)?;
                    let expected = u64::from(request.view.size[0])
                        .checked_mul(u64::from(request.view.size[1]))
                        .and_then(|n| n.checked_mul(4))
                        .filter(|n| *n <= 16 << 20)
                        .ok_or_else(|| bad("Source7 frame extent"))?;
                    if array.byte_offset() != 0
                        || u64::from(array.length()) != expected
                        || array.buffer().byte_length() != array.byte_length()
                    {
                        return Err(bad("Source7 frame full span"));
                    }
                    let mut bytes = Vec::new();
                    bytes
                        .try_reserve_exact(expected as usize)
                        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Source7 frame copy"))?;
                    if bytes.capacity() > 16 << 20 {
                        return Err(bad("Source7 frame copy capacity"));
                    }
                    bytes.resize(expected as usize, 0);
                    array.copy_to(&mut bytes);
                    Some(bytes)
                } else {
                    None
                };
                Ok(Reply {
                    size: (request.view.size[0], request.view.size[1]),
                    request,
                    pixels,
                })
            })();
            let released = call(&result, "release", &[]).map(|_| ());
            match parsed {
                Ok(reply) => {
                    released?;
                    Ok(reply)
                }
                Err(error) => {
                    let _ = released;
                    Err(error)
                }
            }
        }
    }
}

async fn close_ack(value: &JsValue) -> Result<()> {
    let ack = promise(call(value, "close", &[])?).await?;
    let ack = parse(&ack)?;
    if ack.get("kind").and_then(Value::as_str) != Some("closed") {
        return Err(bad("Source7 close acknowledgement"));
    }
    Ok(())
}
