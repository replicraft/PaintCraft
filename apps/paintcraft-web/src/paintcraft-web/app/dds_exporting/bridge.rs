//! Typed DDS capture over the shared admitted browser export bridge.
use super::super::codec_export_bridge::{self as common, Admission, bytes, call, error, object};
pub(super) use super::super::codec_export_bridge::{Cancellation, Lease, quarantined};
use super::*;
use js_sys::Uint8Array;
use paintcraft_formats::codec_worker::Owner;
use std::sync::Arc;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
pub(super) async fn prepare(
    service: JsValue,
    doc: Arc<paintcraft_document::Document>,
    plan: wire::SourcePlan,
    owner: Owner,
    nonce: uuid::Uuid,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) -> Result<Prepared> {
    let owner_value = object(&[
        ("session", owner.session.to_string().into()),
        ("state", owner.state.to_string().into()),
        ("epoch", js_sys::BigInt::from(owner.epoch).into()),
        ("frontend", 3u32.into()),
    ])?;
    let args = js_sys::Array::new();
    args.push(&owner_value);
    call(&service, "setOwner", &args)?;
    let size = plan.size();
    let metadata = object(&[
        ("width", size.0.into()),
        ("height", size.1.into()),
        ("sourceFlags", u32::from(plan.source_flags()).into()),
        ("bakeIntent", u32::from(plan.bake_intent()).into()),
        (
            "filterDigest",
            Uint8Array::from(plan.filter_digest().as_slice()).into(),
        ),
        (
            "metadataContext",
            Uint8Array::from(plan.metadata_context().encode().as_slice()).into(),
        ),
        (
            "settings",
            Uint8Array::from(plan.settings().as_slice()).into(),
        ),
    ])?;
    let args = js_sys::Array::new();
    args.push(&owner_value);
    args.push(&metadata);
    args.push(&nonce.to_string().into());
    // Synchronous admission MUST precede serialization and all snapshot copies.
    let mut admission = Admission {
        value: call(&service, "beginDdsExport", &args)?,
        used: false,
    };
    *cancellation.borrow_mut() = Some(Cancellation(admission.value.clone()));
    let source = plan.snapshot(&doc)?;
    let request = wire::Header::capture(&plan, &source, owner, nonce)?;
    let args = js_sys::Array::new();
    args.push(&Uint8Array::from(source.as_slice()));
    let promise = call(&admission.value, "run", &args)?
        .dyn_into::<js_sys::Promise>()
        .map_err(error)?;
    admission.used = true;
    drop(source);
    drop(args);
    drop(doc);
    let value = JsFuture::from(promise).await.map_err(error)?;
    let mut lease = Lease {
        value,
        valid: false,
    };
    let echo = bytes(&lease.value, "requestHeader", wire::REQUEST_HEADER)?;
    let digest = bytes(&lease.value, "sourceSha256", 32)?;
    let response = bytes(&lease.value, "responseBytes", wire::RESPONSE_BYTES_MAX)?;
    if digest != request.source_sha256 {
        return Err(failed("DDS source binding changed"));
    }
    let mut prepared = response::validate(&request, &echo, response, || lease.valid = true)?;
    prepared.lease = Some(lease);
    Ok(prepared)
}
pub(super) async fn download(mut prepared: Prepared, name: String) -> Result<()> {
    let bytes = &prepared.response[prepared.encoded.clone()];
    common::download(bytes, &name, "image/vnd-ms.dds", prepared.lease.as_mut()).await
}
