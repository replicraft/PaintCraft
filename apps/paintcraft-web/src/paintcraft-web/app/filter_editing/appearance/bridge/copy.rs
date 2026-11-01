//! Visual Source7 PNG copy only; foreign own-mask baking stays unsupported.
use super::*;
use crate::app::filter_editing::{cancel_once::CancelOnce, render::View};
use js_sys::{Object, Reflect};
#[derive(Clone, Debug)]
pub(in crate::app::filter_editing) struct CopyControl7 {
    value: JsValue,
    once: CancelOnce,
}
impl CopyControl7 {
    pub fn cancel(&self) {
        self.once.run_once(|| {
            if let Ok(task) = call(&self.value, "cancel", &["1".into()]) {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = promise(task).await;
                });
            }
        });
    }
}
pub(in crate::app::filter_editing) struct EncodedCopy7 {
    pub bytes: Vec<u8>,
    pub mime: String,
}
impl Client7 {
    pub fn copy_control(&self) -> CopyControl7 {
        CopyControl7 {
            value: self.value.clone(),
            once: CancelOnce::default(),
        }
    }
    pub async fn encode_copy(
        &self,
        region: paintcraft_core::types::Rect,
        spec: &Value,
        mask: &[u8],
    ) -> Result<EncodedCopy7> {
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| bad("Source7 closed copy"))?;
        let prepared = context.get();
        if prepared.capture.operation.is_some() || prepared.copy_spec.as_ref() != Some(spec) {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Source7 copy intent changed",
            ));
        }
        if !matches!(
            spec.get("purpose").and_then(Value::as_str),
            Some("print" | "clipboard")
        ) || spec.get("format").and_then(Value::as_str) != Some("png")
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Layer-mask foreign copies require separate authorization",
            ));
        }
        if mask.len() > 4_194_304 {
            return Err(bad("Source7 copy mask extent"));
        }
        let view = View::new(&prepared.capture.document, region, 0)?;
        let begin = control::begin(prepared, 1, view, true)?;
        let result = promise(call(
            &self.value,
            "encodedCopy",
            &[
                value(&begin)?,
                value(spec)?,
                js_sys::Uint8Array::from(mask).into(),
            ],
        )?)
        .await?;
        let parsed = (|| {
            const KEYS: [&str; 8] = [
                "kind",
                "outputVersion",
                "outputSha256",
                "bytesSha256",
                "mime",
                "binding",
                "bytes",
                "release",
            ];
            let object = result
                .dyn_ref::<Object>()
                .ok_or_else(|| bad("Source7 copy result"))?;
            let keys = Object::keys(object);
            if keys.length() != 8
                || keys
                    .iter()
                    .any(|k| !k.as_string().is_some_and(|s| KEYS.contains(&s.as_str())))
            {
                return Err(bad("Source7 copy closed fields"));
            }
            let metadata = Object::new();
            for key in &KEYS[..6] {
                Reflect::set(&metadata, &(*key).into(), &field(&result, key)?).map_err(error)?;
            }
            let metadata = parse(&metadata.into())?;
            if metadata.get("kind").and_then(Value::as_str) != Some("encoded-copy")
                || metadata.get("outputVersion").and_then(Value::as_i64) != Some(1)
                || metadata.get("mime").and_then(Value::as_str) != Some("image/png")
                || metadata.get("outputSha256").and_then(Value::as_str)
                    != Some(
                        hex(&prepared
                            .copy_sha256
                            .ok_or_else(|| bad("Source7 missing copy SHA"))?)
                        .as_str(),
                    )
            {
                return Err(bad("Source7 copy option binding"));
            }
            let digest = metadata
                .get("bytesSha256")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("Source7 copy byte digest"))?;
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(bad("Source7 copy byte digest grammar"));
            }
            // Actual bytes SHA was verified by the owned JS channel before this
            // synchronous copy. No postseal Main producer hash/ledger is invented.
            let expected = echo::expected(
                &prepared.capture,
                &prepared.frames,
                &prepared.source_sha256,
                &prepared.operation_sha256,
                1,
                view,
                true,
            );
            echo::validate(
                metadata
                    .get("binding")
                    .ok_or_else(|| bad("Source7 copy binding"))?,
                &expected,
                prepared.frames.sealed.admission,
            )?;
            let bytes = field(&result, "bytes")?
                .dyn_into::<js_sys::Uint8Array>()
                .map_err(error)?;
            let limit = (16usize << 20)
                .checked_sub(mask.len())
                .ok_or_else(|| bad("Source7 copy output bound"))?;
            if bytes.length() == 0
                || bytes.length() as usize > limit
                || bytes.byte_offset() != 0
                || bytes.buffer().byte_length() != bytes.byte_length()
            {
                return Err(bad("Source7 encoded span"));
            }
            let mut output = Vec::new();
            output
                .try_reserve_exact(bytes.length() as usize)
                .map_err(|_| {
                    Error::new(ErrorCode::ResourceLimit, "Source7 encoded copy allocation")
                })?;
            if output.capacity() > limit {
                return Err(bad("Source7 encoded capacity"));
            }
            output.resize(bytes.length() as usize, 0);
            bytes.copy_to(&mut output);
            Ok(EncodedCopy7 {
                bytes: output,
                mime: "image/png".into(),
            })
        })();
        let released = call(&result, "release", &[]).map(|_| ());
        match parsed {
            Ok(output) => {
                released?;
                Ok(output)
            }
            Err(error) => {
                let _ = released;
                Err(error)
            }
        }
    }
}
