//! Copy output on the already admitted Source6 client, then acknowledged close.
use super::*;
#[derive(Clone, Debug)]
pub(in super::super) struct CopyControl {
    value: JsValue,
}
impl CopyControl {
    pub(in super::super) fn cancel(&self) {
        if let Ok(pending) = call(&self.value, "cancel", &["1".into()]) {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = promise(pending).await;
            });
        }
    }
}
pub(in super::super) struct EncodedCopy {
    pub bytes: Vec<u8>,
    pub mime: String,
}
impl Client {
    pub(in super::super) fn copy_control(&self) -> CopyControl {
        CopyControl {
            value: self.value.clone(),
        }
    }
    pub(in super::super) async fn encode_copy(
        &self,
        region: paintcraft_core::types::Rect,
        spec: &json::Value,
        mask: &[u8],
    ) -> Result<EncodedCopy> {
        if !self.capture.scoped {
            return Err(failed("Flat copy requires Source6"));
        }
        let view = View::new(&self.capture.document, region, 0)?;
        let requirements = super::super::requirements::capture(&self.capture.document, None)?;
        let request = object(&[
            ("generation", "1".into()),
            ("operation", JsValue::NULL),
            ("target", JsValue::NULL),
            ("render", true.into()),
            ("view", super::view(view)?),
        ])?;
        scoped::attach(&request, &requirements)?;
        let encoded = json::canonical(spec);
        if encoded.len() > 4096 || mask.len() > 4_194_304 {
            return Err(failed("Flat copy control extent"));
        }
        let expected_mime = match spec.get("format").and_then(json::Value::as_str) {
            Some("png") => "image/png",
            Some("jpeg") => "image/jpeg",
            Some("bmp") => "image/bmp",
            _ => return Err(failed("Flat copy MIME")),
        };
        let spec = js_sys::JSON::parse(
            std::str::from_utf8(&encoded).map_err(|_| failed("Flat copy encoding"))?,
        )
        .map_err(error)?;
        let result = promise(call(
            &self.value,
            "encodedCopy",
            &[request, spec, Uint8Array::from(mask).into()],
        )?)
        .await?;
        let parsed = (|| {
            string(&result, "kind", "encoded-copy")?;
            number(&result, "outputVersion", 1)?;
            string(&result, "outputSha256", &hex(&Sha256::digest(&encoded)))?;
            let binding = field(&result, "binding")?;
            validate_owner(&field(&binding, "owner")?, &self.capture)?;
            string(&binding, "draft", &self.capture.draft.to_string())?;
            string(&binding, "sourceSha256", &self.sha)?;
            string(&binding, "generation", "1")?;
            string(&binding, "operationSha256", &hex(&Sha256::digest(b"null")))?;
            if !field(&binding, "target")?.is_null() {
                return Err(failed("Flat copy target"));
            }
            scoped::validate(&binding, &requirements)?;
            number(&binding, "width", view.size[0])?;
            number(&binding, "height", view.size[1])?;
            number(&binding, "level", 0)?;
            validate_region(&field(&binding, "region")?, view.region)?;
            let echoed_view = field(&binding, "view")?;
            number(&echoed_view, "level", 0)?;
            validate_region(&field(&echoed_view, "region")?, region)?;
            let output = field(&result, "bytes")?
                .dyn_into::<Uint8Array>()
                .map_err(error)?;
            if output.length() == 0 || output.length() as usize > (16 << 20) - mask.len() {
                return Err(failed("Flat copy output extent"));
            }
            let bytes = output.to_vec();
            string(&result, "bytesSha256", &hex(&Sha256::digest(&bytes)))?;
            let mime = field(&result, "mime")?
                .as_string()
                .filter(|m| matches!(m.as_str(), "image/png" | "image/jpeg" | "image/bmp"))
                .ok_or_else(|| failed("Flat copy MIME"))?;
            if mime != expected_mime {
                return Err(failed("Flat copy MIME changed"));
            }
            Ok(EncodedCopy { bytes, mime })
        })();
        // Consume transfer before close. Normal close retires+ACKs the source
        // and releases the one existing admission; no parallel Worker is born.
        let _ = call(&result, "release", &[]);
        parsed
    }
}
