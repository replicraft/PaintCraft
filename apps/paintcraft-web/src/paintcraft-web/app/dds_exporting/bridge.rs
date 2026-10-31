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
    let getter = if plan.protocol_version() == 7 {
        "ddsPreparedExportProtocolVersion"
    } else if plan.protocol_version() == 6 {
        "ddsCatalogExportProtocolVersion"
    } else if plan.protocol_version() == 5 {
        "ddsSpatialExportProtocolVersion"
    } else {
        "ddsExportProtocolVersion"
    };
    if call(&service, getter, &js_sys::Array::new())?.as_f64()
        != Some(f64::from(plan.protocol_version()))
    {
        return Err(failed("Browser export protocol mismatch"));
    }
    let size = plan.size();
    let metadata = object(&[
        ("width", size.0.into()),
        ("protocolVersion", u32::from(plan.protocol_version()).into()),
        ("height", size.1.into()),
        ("sourceFlags", u32::from(plan.source_flags()).into()),
        ("bakeIntent", u32::from(plan.bake_intent()).into()),
        (
            "filterDigest",
            Uint8Array::from(plan.filter_digest().as_slice()).into(),
        ),
        (
            "maskContext",
            Uint8Array::from(plan.mask_context().encode().as_slice()).into(),
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
    let request = Request::capture(&plan, &source, owner, nonce)?;
    // Same MainWasm ABI as the isolated codec Worker; never infer native sizes.
    let final_plan = if matches!(plan.protocol_version(), 5..=7) {
        // Header7 already authenticated and assessed this exact snapshot once.
        // Reuse its source-aware wire facts rather than rehashing or finalizing
        // from the live document's allocation history.
        let final_region = match &request {
            Request::V7(header) => header.spatial.encode_programs(),
            _ => plan.spatial_context(source.len())?,
        };
        js_sys::Reflect::set(
            &metadata,
            &"spatialContext".into(),
            &Uint8Array::from(final_region.as_slice()),
        )
        .map_err(error)?;
        if plan.protocol_version() == 6 {
            js_sys::Reflect::set(
                &metadata,
                &"catalogContext".into(),
                &Uint8Array::from(plan.catalog_context(source.len())?.as_slice()),
            )
            .map_err(error)?;
        }
        if let Request::V7(header) = &request {
            js_sys::Reflect::set(
                &metadata,
                &"programContext".into(),
                &Uint8Array::from(header.programs.encode().as_slice()),
            )
            .map_err(error)?;
        }
        Some(metadata.clone())
    } else {
        None
    };
    let args = js_sys::Array::new();
    args.push(&Uint8Array::from(source.as_slice()));
    if let Some(value) = final_plan {
        args.push(&value);
    }
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
    let echo = bytes(&lease.value, "requestHeader", request.header_bytes())?;
    let digest = bytes(&lease.value, "sourceSha256", 32)?;
    let response = bytes(&lease.value, "responseBytes", request.response_max())?;
    if digest != request.source_sha256() {
        return Err(failed("DDS source binding changed"));
    }
    let mut prepared =
        response::validate_request(&request, &echo, response, || lease.valid = true)?;
    prepared.lease = Some(lease);
    Ok(prepared)
}
pub(super) async fn download(mut prepared: Prepared, name: String) -> Result<()> {
    let bytes = &prepared.response[prepared.encoded.clone()];
    common::download(bytes, &name, "image/vnd-ms.dds", prepared.lease.as_mut()).await
}
