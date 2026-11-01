//! Source7 visual Print/clipboard only. Foreign downloads remain closed.
use super::*;
use paintcraft_formats::image::{ImageFormat, JpegMatte, bounded_copy};
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Download,
    Print,
    Clipboard,
}
#[derive(Clone, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
struct Spec {
    purpose: Purpose,
    format: ImageFormat,
    quality: u8,
    matte: JpegMatte,
    mask_len: usize,
    mask_sha: [u8; 32],
    digest: [u8; 32],
}
#[derive(Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(super) struct Reservation {
    spec: Spec,
    scratch: BudgetAccount,
    output: BudgetAccount,
}
fn fields(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or_else(|| bad("Flat output object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(bad("Flat output fields"));
    }
    Ok(())
}
fn flag(v: &Value, k: &str) -> Result<bool> {
    field(v, k)?
        .as_bool()
        .ok_or_else(|| bad("Flat output boolean"))
}
impl Spec {
    fn parse(control: &str) -> Result<Self> {
        if control.len() > 4096 {
            return Err(resource("Flat output control exceeds4096"));
        }
        let value = paintcraft_core::types::json::parse(control.as_bytes())
            .map_err(|_| bad("Malformed flat output JSON"))?;
        fields(
            &value,
            &[
                "version",
                "purpose",
                "format",
                "quality",
                "matte",
                "frontend",
                "omitMetadata",
                "bakeFilters",
                "maskLength",
                "maskSha256",
            ],
        )?;
        if integer(field(&value, "version")?)? != 1 {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Flat output version",
            ));
        }
        let purpose = match string(field(&value, "purpose")?)? {
            "download" => Purpose::Download,
            "print" => Purpose::Print,
            "clipboard" => Purpose::Clipboard,
            _ => return Err(bad("Flat output purpose")),
        };
        let format = match string(field(&value, "format")?)? {
            "png" => ImageFormat::Png,
            "jpeg" => ImageFormat::Jpeg,
            "bmp" => ImageFormat::Bmp,
            _ => {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Unpriced flat output codec",
                ));
            }
        };
        if string(field(&value, "frontend")?)? != "studio" {
            return Err(bad("Source7 visual copy is Studio only"));
        }
        let omit = flag(&value, "omitMetadata")?;
        let bake = flag(&value, "bakeFilters")?;
        let quality = u8::try_from(integer(field(&value, "quality")?)?)
            .map_err(|_| bad("Flat output quality"))?;
        if !(1..=100).contains(&quality) {
            return Err(bad("Flat output quality"));
        }
        let values = field(&value, "matte")?
            .as_array()
            .ok_or_else(|| bad("Flat output matte"))?;
        if values.len() != 3 {
            return Err(bad("Flat output matte"));
        }
        let mut matte = [0; 3];
        for (out, v) in matte.iter_mut().zip(values) {
            *out = u8::try_from(integer(v)?).map_err(|_| bad("Flat output matte"))?;
        }
        let mask_len = usize::try_from(integer(field(&value, "maskLength")?)?)
            .map_err(|_| resource("Flat output mask extent"))?;
        if mask_len > OUTPUT_PIXELS_MAX as usize {
            return Err(resource("Flat output mask extent"));
        }
        let spec = Self {
            purpose,
            format,
            quality,
            matte: JpegMatte::new(matte),
            mask_len,
            mask_sha: protocol::scoped_programs::hash(field(&value, "maskSha256")?)?,
            digest: Sha256::digest(paintcraft_core::types::json::canonical(&value)).into(),
        };
        if purpose != Purpose::Clipboard && mask_len != 0 {
            return Err(bad("Only visible clipboard has a selection field"));
        }
        if purpose != Purpose::Download
            && (format != ImageFormat::Png || quality != 90 || matte != [255; 3] || omit || bake)
        {
            return Err(bad(
                "Visual copies use PNG without foreign-copy policy flags",
            ));
        }
        if format != ImageFormat::Jpeg && matte != [255; 3] {
            return Err(bad("JPEG matte requires JPEG"));
        }
        Ok(spec)
    }
    fn validate(&self, _doc: &Document) -> Result<()> {
        if self.purpose == Purpose::Download {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Source7 foreign downloads require separate layer-mask appearance authority",
            ));
        }
        Ok(())
    }
}
impl LayerAppearanceService {
    /// Borrowed response prevents unbounded native metadata escapes under one
    /// fixed controls lease. Wasm serializes synchronously before next mutation.
    pub fn prepare_copy(
        &mut self,
        begin: &str,
        control: &str,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<&Value> {
        let value = self.prepare_copy_inner(begin, control, check)?;
        self.state.last_copy_admission = Some(value);
        self.state
            .last_copy_admission
            .as_ref()
            .ok_or_else(|| bad("Source7 copy binding missing"))
    }

    /// Must run before any mask transfer or render step. Same source budget and
    /// work counter; no fresh realm. The scratch32MiB includes selection input
    /// up to4MiB, JPEG RGB up to12MiB and opaque codec payload up to16MiB.
    fn prepare_copy_inner(
        &mut self,
        begin: &str,
        control: &str,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Value> {
        self.ensure_no_escape()?;
        self.state
            .job
            .spend_control(control::parse_quote(control.len())?)?;
        let spec = Spec::parse(control)?;
        spec.validate(&self.state.base)?;
        self.state
            .job
            .spend_control(control::parse_quote(begin.len())?)?;
        let parsed = wire::begin(begin, check)?;
        parsed.matches_source(&self.state.source, &self.state.profile)?;
        if *field(parsed.value(), "operation")? != Value::Null
            || !parsed.render
            || parsed.view.level != 0
        {
            return Err(bad("Flat output requires level0 BASE"));
        }
        if spec.purpose != Purpose::Clipboard && parsed.view.rect != self.state.base.canvas_rect() {
            return Err(bad("Flat output must cover whole frozen canvas"));
        }
        let count = (parsed.view.rect.width() as u64)
            .checked_mul(parsed.view.rect.height() as u64)
            .ok_or_else(|| resource("Flat output geometry"))?;
        if spec.mask_len != 0 && spec.mask_len as u64 != count {
            return Err(bad("Flat output selection geometry"));
        }
        let q = bounded_copy::quote(
            spec.format,
            parsed.view.rect.width() as u32,
            parsed.view.rect.height() as u32,
        )?;
        let mut scratch = self.state.job.workspace().scope().account();
        scratch.resize(bounded_copy::SCRATCH_MAX)?;
        let mut output = self.state.output.account();
        output.resize(q.output_bytes)?;
        // Accounts remain private/live across normal preparation's retire().
        let mut binding = self.prepare_checked(begin, check)?.clone();
        let result = (|| {
            let frame = self
                .state
                .frame
                .as_mut()
                .ok_or_else(|| bad("Flat output frame"))?;
            let extra = WorkComponents {
                base: q
                    .base_work
                    .checked_add(
                        (spec.mask_len as u64)
                            .checked_mul(2)
                            .ok_or_else(|| resource("Flat selection work"))?,
                    )
                    .ok_or_else(|| resource("Flat output work"))?,
                noise: 0,
            };
            // Frame compile/other work is already debited. Reserve extra codec
            // work only after proving its sum with all still-deferred rendering.
            frame
                .job
                .check_available(frame.remaining.checked_add(extra)?)?;
            let work = frame.total_work.checked_add(extra)?;
            frame.job.spend_control(extra.base)?;
            let Value::Object(object) = &mut binding else {
                return Err(bad("Flat output binding"));
            };
            object.insert(
                "workQuote".into(),
                quote::value(work, frame.mask_actual, frame.mask_upper)?,
            );
            frame.binding = binding.clone();
            frame.copy = Some(Reservation {
                spec: spec.clone(),
                scratch,
                output,
            });
            self.state.last_binding = Some(binding.clone());
            Ok(Value::object([
                ("kind", Value::str("copy-admitted")),
                ("outputVersion", Value::Int(1)),
                ("outputSha256", Value::str(hex(&spec.digest))),
                ("binding", binding),
            ]))
        })();
        if result.is_err() {
            self.retire();
        }
        result
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn encode_copy_for_wasm(
        &mut self,
        mask: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.encode_copy_checked(mask, check)
    }
    /// The same boundary is used by the public Wasm entry and native failure
    /// controls. No failure may leave a complete copy-mutated BASE readable.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn encode_copy_checked(
        &mut self,
        mask: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        let result = self.encode_copy_inner(mask, check);
        if result.is_err() {
            self.retire();
        }
        result
    }
    #[cfg(any(target_arch = "wasm32", test))]
    fn encode_copy_inner(
        &mut self,
        mask: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.ensure_no_escape()?;
        let f = self
            .state
            .frame
            .as_mut()
            .ok_or_else(|| bad("Flat output frame"))?;
        if !f.complete() || f.delivered {
            return Err(bad("Flat output incomplete or delivered"));
        }
        let spec = &f
            .copy
            .as_ref()
            .ok_or_else(|| bad("Flat output was not admitted"))?
            .spec;
        if mask.len() != spec.mask_len || <[u8; 32]>::from(Sha256::digest(mask)) != spec.mask_sha {
            return Err(stale());
        }
        // Entire domain is checked before mutating even the first pixel. A
        // later cancellation/codec error is still handled by the outer retire.
        for chunk in mask.chunks(64) {
            check()?;
            if chunk.iter().any(|value| !matches!(*value, 0 | 255)) {
                return Err(bad("Visible selection is binary"));
            }
        }
        let reservation = f.copy.take().expect("validated copy reservation");
        let spec = &reservation.spec;
        if !mask.is_empty() {
            for (i, pixel) in f.pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                if i.is_multiple_of(64) {
                    check()?;
                }
                if mask[i] == 0 {
                    pixel.fill(0);
                }
            }
        }
        let size = field(&f.binding, "width")?
            .as_i64()
            .zip(field(&f.binding, "height")?.as_i64())
            .ok_or_else(|| bad("Flat output size"))?;
        // Reservation already accounts codec phase. Pass borrowed accounts to the
        // bounded adapter; no second scratch/index capacity is created here.
        let encoded = bounded_copy::encode_with_accounts(
            bounded_copy::Options {
                format: spec.format,
                width: size.0 as u32,
                height: size.1 as u32,
                quality: spec.quality,
                matte: spec.matte,
                output_limit: bounded_copy::OUTPUT_MAX - spec.mask_len as u64,
            },
            &f.pixels,
            reservation.scratch,
            reservation.output,
            check,
        )?;
        let (bytes, account) = encoded.into_parts();
        f.encoded_account = Some(account);
        f.delivered = true;
        Ok(bytes)
    }
}
