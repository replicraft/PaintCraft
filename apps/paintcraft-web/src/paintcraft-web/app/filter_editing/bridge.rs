//! Calls an admitted, real dedicated Worker; no rendering runs in spawn_local.
use super::render::{Capture, Reply, Request, View};
use js_sys::{Array, Function, Reflect, Uint8Array};
use paintcraft_core::types::{Error, ErrorCode, Result, json};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen::prelude::wasm_bindgen(module = "/src/lib/filter-preview-bridge.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name=beginFilterPreview)]
    fn begin(plan: &JsValue) -> std::result::Result<JsValue, JsValue>;
}
fn failed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn error(value: JsValue) -> Error {
    let code = Reflect::get(&value, &"code".into())
        .ok()
        .and_then(|v| v.as_string());
    Error::new(
        match code.as_deref() {
            Some("OperationCancelled" | "operation.cancelled") => ErrorCode::OperationCancelled,
            Some("ResourceLimit" | "resource.limit") => ErrorCode::ResourceLimit,
            Some("ConflictStaleBase" | "conflict.stale_base") => ErrorCode::ConflictStaleBase,
            Some("CapabilityUnsupported" | "capability.unsupported") => {
                ErrorCode::CapabilityUnsupported
            }
            _ => ErrorCode::CodecFailed,
        },
        "Filter preview Worker failed",
    )
}
fn field(v: &JsValue, key: &str) -> Result<JsValue> {
    Reflect::get(v, &key.into()).map_err(error)
}
fn call(v: &JsValue, key: &str, args: &[JsValue]) -> Result<JsValue> {
    let array = Array::new();
    for value in args {
        array.push(value);
    }
    field(v, key)?
        .dyn_into::<Function>()
        .map_err(error)?
        .apply(v, &array)
        .map_err(error)
}
async fn promise(value: JsValue) -> Result<JsValue> {
    JsFuture::from(value.dyn_into::<js_sys::Promise>().map_err(error)?)
        .await
        .map_err(error)
}
fn object(fields: &[(&str, JsValue)]) -> Result<JsValue> {
    let object = js_sys::Object::new();
    for (key, value) in fields {
        Reflect::set(&object, &(*key).into(), value).map_err(error)?;
    }
    Ok(object.into())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn string(v: &JsValue, key: &str, expected: &str) -> Result<()> {
    if field(v, key)?.as_string().as_deref() != Some(expected) {
        return Err(failed("Filter preview echo changed"));
    }
    Ok(())
}
fn number(v: &JsValue, key: &str, expected: u32) -> Result<()> {
    if field(v, key)?.as_f64() != Some(f64::from(expected)) {
        return Err(failed("Filter preview geometry changed"));
    }
    Ok(())
}
fn owner(c: &Capture) -> Result<JsValue> {
    object(&[
        ("session", c.owner.to_string().into()),
        ("state", c.ticket.state_id.to_string().into()),
        ("epoch", c.ticket.publication_epoch.to_string().into()),
        ("frontend", 3u32.into()),
    ])
}
fn validate_owner(value: &JsValue, capture: &Capture) -> Result<()> {
    string(value, "session", &capture.owner.to_string())?;
    string(value, "state", &capture.ticket.state_id.to_string())?;
    string(
        value,
        "epoch",
        &capture.ticket.publication_epoch.to_string(),
    )?;
    number(value, "frontend", 3)
}
fn dimensions(size: (u32, u32)) -> JsValue {
    Array::of2(&size.0.into(), &size.1.into()).into()
}
fn region_value(region: paintcraft_core::types::Rect) -> JsValue {
    Array::of4(
        &(region.x0 as u32).into(),
        &(region.y0 as u32).into(),
        &(region.width() as u32).into(),
        &(region.height() as u32).into(),
    )
    .into()
}
fn view(request: View) -> Result<JsValue> {
    object(&[
        ("region", region_value(request.visible)),
        ("level", u32::from(request.level).into()),
    ])
}
fn validate_region(value: &JsValue, region: paintcraft_core::types::Rect) -> Result<()> {
    let array = value
        .dyn_ref::<Array>()
        .ok_or_else(|| failed("Filter preview region type"))?;
    if array.length() != 4 {
        return Err(failed("Filter preview region length"));
    }
    for (index, expected) in [region.x0, region.y0, region.width(), region.height()]
        .into_iter()
        .enumerate()
    {
        if array.get(index as u32).as_f64() != Some(expected as f64) {
            return Err(failed("Filter preview region changed"));
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub(super) struct Cancellation(JsValue);
impl Cancellation {
    pub fn cancel(&self) {
        let _ = call(&self.0, "cancel", &[]);
    }
}
pub(super) type CancelSlot = Rc<RefCell<Option<Cancellation>>>;
struct Admission {
    value: JsValue,
    used: bool,
}
impl Drop for Admission {
    fn drop(&mut self) {
        if !self.used {
            let _ = call(&self.value, "cancel", &[]);
        }
    }
}
#[derive(Debug)]
pub(super) struct Client {
    value: JsValue,
    capture: Capture,
    sha: String,
    closed: bool,
}
impl Drop for Client {
    fn drop(&mut self) {
        if !self.closed {
            let value = self.value.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(p) = call(&value, "close", &[]) {
                    let _ = promise(p).await;
                }
            });
        }
    }
}
pub(super) async fn open(mut capture: Capture, cancellation: CancelSlot) -> Result<Client> {
    let spatial_count = super::render::stored_spatial(&capture.document);
    capture.spatial |= spatial_count > 0;
    let catalog_count = super::render::stored_catalog(&capture.document);
    capture.catalog |= catalog_count > 0;
    let doc = &capture.document;
    let resident = doc
        .layers
        .values()
        .try_fold(0u64, |n, l| {
            n.checked_add(
                l.raster.resident_tiles() as u64
                    * paintcraft_raster::RasterLayer::TILE_BYTES as u64,
            )
        })
        .ok_or_else(|| failed("Filter source resident overflow"))?;
    let inventory = paintcraft_formats::export::filter_bake::inspect(doc, &|| Ok(()))?;
    let (metadata_bytes, metadata_charge) =
        doc.source_metadata.as_ref().map_or((0usize, 0u64), |s| {
            (s.profile().map_or(0, <[u8]>::len), s.storage_usage().1)
        });
    let masks = paintcraft_formats::pcnd::coverage_facts(doc, &|| Ok(()))?;
    let plan = object(&[
        ("owner", owner(&capture)?),
        ("draft", capture.draft.to_string().into()),
        ("document", doc.document_id.to_string().into()),
        ("size", dimensions(doc.canvas)),
        ("layers", (doc.layers.len() as u32).into()),
        ("residentBytes", (resident as f64).into()),
        ("metadataBytes", (metadata_bytes as f64).into()),
        ("metadataCharge", (metadata_charge as f64).into()),
        ("filterDigest", hex(&inventory.digest()).into()),
        ("maskCount", (masks.masks as f64).into()),
        ("maskBands", (masks.bands as f64).into()),
        ("maskRuns", (masks.runs as f64).into()),
        ("maskDecodeBytes", (masks.decode_bytes as f64).into()),
    ])?;
    if capture.spatial || capture.catalog {
        for (key, value) in [
            ("protocol", if capture.catalog { 4u64 } else { 3u64 }),
            ("spatialCount", spatial_count),
            ("workspaceBytes", 64 << 20),
            ("workLimit", 268435456),
        ] {
            Reflect::set(&plan, &key.into(), &JsValue::from_f64(value as f64)).map_err(error)?;
        }
    }
    if capture.catalog {
        Reflect::set(
            &plan,
            &"catalogCount".into(),
            &JsValue::from_f64(catalog_count as f64),
        )
        .map_err(error)?;
    }
    // Admission precedes serialization, bridge copying and Worker construction.
    let mut admission = Admission {
        value: begin(&plan).map_err(error)?,
        used: false,
    };
    *cancellation.borrow_mut() = Some(Cancellation(admission.value.clone()));
    let source = super::snapshot::encode(doc, &|| Ok(()))?;
    let sha = hex(&Sha256::digest(&source));
    let pending = call(
        &admission.value,
        "open",
        &[
            Uint8Array::from(source.as_slice()).into(),
            sha.clone().into(),
        ],
    )?;
    admission.used = true;
    drop(source);
    let value = promise(pending).await?;
    let client = Client {
        value,
        capture,
        sha,
        closed: false,
    };
    let binding = field(&client.value, "binding")?;
    validate_owner(&field(&binding, "owner")?, &client.capture)?;
    string(&binding, "draft", &client.capture.draft.to_string())?;
    string(
        &binding,
        "document",
        &client.capture.document.document_id.to_string(),
    )?;
    string(&binding, "sourceSha256", &client.sha)?;
    string(&binding, "filterDigest", &hex(&inventory.digest()))?;
    for (name, expected) in [
        ("maskCount", masks.masks),
        ("maskBands", masks.bands),
        ("maskRuns", masks.runs),
        ("maskDecodeBytes", masks.decode_bytes),
    ] {
        if field(&binding, name)?.as_f64() != Some(expected as f64) {
            return Err(failed("Filter mask source admission changed"));
        }
    }
    if client.capture.spatial || client.capture.catalog {
        for (key, expected) in [
            ("protocol", if client.capture.catalog { 4u64 } else { 3u64 }),
            ("spatialCount", spatial_count),
            ("workspaceBytes", 64 << 20),
            ("workLimit", 268435456),
            ("layers", client.capture.document.layers.len() as u64),
            ("residentBytes", resident),
            ("metadataBytes", metadata_bytes as u64),
            ("metadataCharge", metadata_charge),
        ] {
            if field(&binding, key)?.as_f64() != Some(expected as f64) {
                return Err(failed("Spatial source workspace binding changed"));
            }
        }
    }
    if client.capture.catalog
        && field(&binding, "catalogCount")?.as_f64() != Some(catalog_count as f64)
    {
        return Err(failed("Catalog source binding changed"));
    }
    let size = field(&binding, "size")?
        .dyn_into::<Array>()
        .map_err(error)?;
    if size.length() != 2
        || size.get(0).as_f64() != Some(f64::from(client.capture.document.canvas.0))
        || size.get(1).as_f64() != Some(f64::from(client.capture.document.canvas.1))
    {
        return Err(failed("Filter source geometry changed"));
    }
    Ok(client)
}
impl Client {
    pub fn cancel(&self, sequence: u64) {
        if let Ok(p) = call(&self.value, "cancel", &[sequence.to_string().into()]) {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = promise(p).await;
            });
        }
    }
    pub async fn close(mut self) -> Result<()> {
        let reply = promise(call(&self.value, "close", &[])?).await?;
        string(&reply, "kind", "closed")?;
        self.closed = true;
        Ok(())
    }
    pub fn run(
        &self,
        request: Request,
        sequence: u64,
    ) -> impl std::future::Future<Output = Result<Reply>> + 'static {
        let value = self.value.clone();
        let capture = self.capture.clone();
        let sha = self.sha.clone();
        async move {
            let size = (request.view.size[0], request.view.size[1]);
            let generation = sequence.to_string();
            let target = request
                .edit
                .as_ref()
                .map(|e| super::target(&e.operation))
                .transpose()?;
            let mut operation_sha = hex(&Sha256::digest(b"null"));
            let operation = if let Some(edit) = &request.edit {
                if !edit.matches_base(
                    capture.owner,
                    capture.ticket.state_id,
                    capture.ticket.publication_epoch,
                ) {
                    return Err(failed("Filter operation base changed"));
                }
                let bytes = json::canonical(&edit.operation.to_value()?);
                operation_sha = hex(&Sha256::digest(&bytes));
                if bytes.len() > 4096 {
                    return Err(failed("Filter operation allowance"));
                }
                Some(
                    js_sys::JSON::parse(
                        std::str::from_utf8(&bytes)
                            .map_err(|_| failed("Filter operation encoding"))?,
                    )
                    .map_err(error)?,
                )
            } else {
                None
            };
            let pending = if let Some(operation) = operation {
                call(
                    &value,
                    "update",
                    &[object(&[
                        ("generation", generation.clone().into()),
                        ("operation", operation),
                        (
                            "target",
                            target
                                .ok_or_else(|| failed("Filter target missing"))?
                                .to_string()
                                .into(),
                        ),
                        ("view", view(request.view)?),
                        ("render", request.render.into()),
                    ])?],
                )?
            } else {
                call(
                    &value,
                    "base",
                    &[object(&[
                        ("generation", generation.clone().into()),
                        ("view", view(request.view)?),
                    ])?],
                )?
            };
            let result = promise(pending).await?;
            let parsed = (|| {
                string(&result, "generation", &generation)?;
                string(&result, "operationSha256", &operation_sha)?;
                validate_owner(&field(&result, "owner")?, &capture)?;
                string(&result, "draft", &capture.draft.to_string())?;
                string(&result, "sourceSha256", &sha)?;
                match target {
                    Some(id) => string(&result, "target", &id.to_string())?,
                    None => {
                        if !field(&result, "target")?.is_null() {
                            return Err(failed("Filter base target changed"));
                        }
                    }
                }
                let viewport = field(&result, "view")?;
                validate_region(&field(&viewport, "region")?, request.view.visible)?;
                number(&viewport, "level", u32::from(request.view.level))?;
                validate_region(&field(&result, "region")?, request.view.region)?;
                number(&result, "level", u32::from(request.view.level))?;
                number(&result, "width", size.0)?;
                number(&result, "height", size.1)?;
                let pixels = if request.render {
                    string(&result, "kind", "frame")?;
                    let bytes = field(&result, "pixels")?
                        .dyn_into::<Uint8Array>()
                        .map_err(error)?;
                    let length = u64::from(size.0) * u64::from(size.1) * 4;
                    if length > crate::editor::MAX_PIXELS * 4 || u64::from(bytes.length()) != length
                    {
                        return Err(failed("Filter output allowance"));
                    }
                    let mut output = Vec::new();
                    output.try_reserve_exact(length as usize).map_err(|_| {
                        Error::new(ErrorCode::ResourceLimit, "Filter preview allocation")
                    })?;
                    output.resize(length as usize, 0);
                    bytes.copy_to(&mut output);
                    Some(output)
                } else {
                    string(&result, "kind", "prepared")?;
                    None
                };
                Ok(Reply {
                    request,
                    pixels,
                    size,
                })
            })();
            // The service retains its output allowance until this consumption ack.
            let released = call(&result, "release", &[]);
            parsed.and_then(|reply| released.map(|_| reply))
        }
    }
}
