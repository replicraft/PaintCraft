//! Borrowed execution requirements without retained document payloads.
//!
//! A witness authenticates the complete container once. Its body is never public,
//! and it grants integrity only, not typed validity or execution authority. The
//! worker must resolve requirements before asking this same witness to adopt.
use super::*;
use paintcraft_core::types::{
    filters::{
        BlackWhiteV1, BoxBlurV1, BrightnessContrastV1, Descriptor, PosterizeRgbV1, ThresholdLumaV1,
    },
    resources::BudgetScope,
};
use std::{cell::Cell, fmt};
mod shape;

/// Immutable borrowed bytes with a validated outer PCND trailer.
/// Constructible only by an actual integrity pass; no caller authentication flag.
pub struct CheckedContainer<'a> {
    bytes: &'a [u8],
    body: &'a [u8],
    version: u8,
}
impl fmt::Debug for CheckedContainer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedContainer")
            .field("version", &self.version)
            .field("container_bytes", &self.bytes.len())
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}
impl<'a> CheckedContainer<'a> {
    /// Authenticate the inner trailer and hash the whole immutable source with
    /// one traversal of its body. Existing `verify` keeps its callback contract.
    pub fn verify_with_source_hash(
        bytes: &'a [u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, [u8; 32])> {
        if bytes.len() < MAGIC.len() + 32 || &bytes[..4] != b"PCND" || bytes[5..8] != MAGIC[5..8] {
            return Err(codec("invalid compact PCND header"));
        }
        #[cfg(test)]
        PAYLOAD_HASH_PASSES.with(|passes| passes.set(passes.get() + 1));
        let (body, trailer) = bytes.split_at(bytes.len() - 32);
        let mut hash = Sha256::new();
        for chunk in body.chunks(65_536) {
            check()?;
            hash.update(chunk);
        }
        if hash.clone().finalize().as_slice() != trailer {
            return Err(codec("compact PCND SHA-256 mismatch"));
        }
        hash.update(trailer);
        let whole = hash.finalize().into();
        Ok((
            Self {
                bytes,
                body: &body[MAGIC.len()..],
                version: bytes[4],
            },
            whole,
        ))
    }

    /// Count actual encoded tile records without inflating pixels or adopting a
    /// Document. Includes explicit all-default records: the reader retains them.
    /// This bounded structural assessment is not full typed document validity.
    pub fn snapshot_tile_records_checked(&self, check: &dyn Fn() -> Result<()>) -> Result<u64> {
        shape::tile_records(self, check)
    }
    pub fn verify(bytes: &'a [u8], check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let body = payload(bytes, check)?;
        Ok(Self {
            bytes,
            body,
            version: bytes[4],
        })
    }
    pub fn version(&self) -> u8 {
        self.version
    }
    /// Returns original authenticated bytes; cannot manufacture another witness.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
    pub(super) fn body(&self) -> &'a [u8] {
        self.body
    }
    /// Version/header assessment sharing the verified trailer; unknown versions
    /// remain assessable, exactly as in the public PCND probe.
    pub fn header(&self) -> Result<Header> {
        probe_verified(self)
    }
    /// Typed adoption with explicit host-owned realms. This reuses the verified
    /// inner trailer, but does not by itself grant any worker execution authority.
    pub fn read_with_admission_checked(
        &self,
        canvas_pixels: u64,
        staged_pixels: u64,
        layer_limit: usize,
        admission: ReadAdmission<'_>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Document> {
        check()?;
        if layer_limit > LAYER_COUNT_MAX {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid host PCND layer limit",
            ));
        }
        read_verified_admitted(
            self,
            canvas_pixels,
            staged_pixels,
            layer_limit,
            admission.metadata,
            Some(admission.coverage),
            check,
        )
    }
    /// Registry callback is an independent execution/pricing check, never a
    /// general-reader support declaration. No retained document data is created.
    pub fn resolve_requirements_checked(
        &self,
        limits: RequirementsLimits,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        mut resolve: impl FnMut(&str, &str) -> Result<()>,
    ) -> Result<()> {
        if !matches!(self.version, 1..=6) {
            return Err(unsupported("unknown native document version"));
        }
        // Price fixed assessment controls before entering the scanner. No Vec,
        // String, Layer, ICC, A8 or tile allocation occurs in this traversal.
        let mut control = scope.account();
        control.resize(REQUIREMENTS_CONTROL_BYTES)?;
        let remaining = Cell::new(limits.work);
        let step = || {
            check()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(resource)?);
            Ok(())
        };
        step()?;
        if self.version <= 2 {
            return Ok(());
        }
        let mut input = Input::new(self.body);
        if self.version == 6 {
            // Reuse canonical identifier/order/framing validation without
            // allocating owned manifest Strings. The second borrowed walk is
            // bounded by the same 64KiB prelude; no second source SHA pass.
            catalog::Manifest::read(&mut input, false, &step)?;
            let mut raw = Input::new(self.body);
            let length = raw.number(65_536)? as usize;
            let mut prelude = Input::new(raw.take(length)?);
            let count = prelude.number(256)?;
            for _ in 0..count {
                step()?;
                let length = prelude.number(96)? as usize;
                let profile = std::str::from_utf8(prelude.take(length)?)
                    .map_err(|_| codec("invalid requirement profile"))?;
                let length = prelude.number(64)? as usize;
                let encoding = std::str::from_utf8(prelude.take(length)?)
                    .map_err(|_| codec("invalid requirement codec"))?;
                resolve(profile, encoding)?;
                step()?;
            }
            return Ok(());
        }
        scan_legacy(&mut input, self.version, limits, &step, &mut resolve)
    }
}

/// All limits constrain assessment, not retained-resource admission. Full typed
/// adoption still validates source metadata, masks, IDs and tile codecs normally.
#[derive(Clone, Copy, Debug)]
pub struct RequirementsLimits {
    pub layers: usize,
    pub tiles: u64,
    pub metadata_bytes: usize,
    pub coverage: paintcraft_raster::coverage_mask::Limits,
    pub work: u64,
}
/// Includes the fixed 256-entry borrowed catalog plus scanner controls. No
/// allocation estimates are inferred from untrusted lengths. The caller includes
/// this control quote in its existing operation reservation, not a fresh realm.
pub const REQUIREMENTS_CONTROL_BYTES: u64 =
    std::mem::size_of::<catalog::Manifest<'static>>() as u64 + 1024;
fn resource() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "PCND requirements assessment limit",
    )
}
fn flag(input: &mut Input<'_>) -> Result<bool> {
    match input.byte()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(codec("invalid PCND assessment flag")),
    }
}
fn string(input: &mut Input<'_>, chars: usize) -> Result<()> {
    let length = input.number(STRING_BYTES_MAX as u64)? as usize;
    let value =
        std::str::from_utf8(input.take(length)?).map_err(|_| codec("invalid compact UTF-8"))?;
    if value.chars().count() > chars {
        return Err(codec("compact string character limit"));
    }
    Ok(())
}
fn source(input: &mut Input<'_>) -> Result<()> {
    let fixed = input.take(paintcraft_document::source_metadata::WIRE_HEADER_BYTES)?;
    let profile = u64::from_be_bytes(fixed[112..120].try_into().unwrap());
    if profile > paintcraft_document::source_metadata::BYTES_MAX {
        return Err(resource());
    }
    input.take(usize::try_from(profile).map_err(|_| resource())?)?;
    Ok(())
}
fn scalar(input: &mut Input<'_>, version: u8) -> Result<(u8, Descriptor)> {
    let tag = input.byte()?;
    let invalid = || codec("invalid legacy filter parameters");
    let descriptor = match tag {
        1 => Descriptor::InvertRgbV1,
        2 => {
            let b = i16::try_from(input.signed()?).map_err(|_| invalid())?;
            let c = i16::try_from(input.signed()?).map_err(|_| invalid())?;
            Descriptor::BrightnessContrastV1(
                BrightnessContrastV1::new(b, c).map_err(|_| invalid())?,
            )
        }
        3 => Descriptor::BlackWhiteRgbV1(
            BlackWhiteV1::new(i16::from(input.byte()?)).map_err(|_| invalid())?,
        ),
        4 => Descriptor::ThresholdLumaV1(
            ThresholdLumaV1::new(i16::from(input.byte()?)).map_err(|_| invalid())?,
        ),
        5 => Descriptor::PosterizeRgbV1(
            PosterizeRgbV1::new(i16::from(input.byte()?)).map_err(|_| invalid())?,
        ),
        6 => Descriptor::SolarizeRgbV1,
        7 if version == 5 => {
            Descriptor::BoxBlurV1(BoxBlurV1::new(i16::from(input.byte()?)).map_err(|_| invalid())?)
        }
        _ => return Err(unsupported("unknown PCND filter algorithm/version")),
    };
    Ok((tag, descriptor))
}
fn scan_legacy(
    input: &mut Input<'_>,
    version: u8,
    limits: RequirementsLimits,
    check: &dyn Fn() -> Result<()>,
    resolve: &mut impl FnMut(&str, &str) -> Result<()>,
) -> Result<()> {
    let initial = input
        .remaining()
        .checked_add(MAGIC.len())
        .ok_or_else(resource)?;
    input.take(32)?; // Document/state UUIDs, borrowed only.
    input.dimension()?;
    input.dimension()?;
    if flag(input)? {
        string(input, 4096)?;
    }
    if flag(input)? {
        source(input)?;
    }
    metadata(initial, input.remaining(), 0, limits.metadata_bytes)?;
    let count = input.number(LAYER_COUNT_MAX as u64)? as usize;
    if count > limits.layers {
        return Err(resource());
    }
    let mut mask_scan = MaskScan::new(limits.coverage);
    let mut seen = 0u8;
    let mut tiles = 0u64;
    let mut payload_bytes = 0usize;
    for _ in 0..count {
        check()?;
        input.take(16)?;
        string(input, 1024)?;
        flag(input)?;
        input.byte()?;
        input.signed()?;
        input.signed()?;
        let width = input.dimension()?;
        let height = input.dimension()?;
        input.take(4)?;
        let filters = input.number(paintcraft_document::filters::FILTERS_MAX as u64)?;
        for _ in 0..filters {
            check()?;
            input.take(16)?;
            flag(input)?;
            let (tag, descriptor) = scalar(input, version)?;
            let bit = 1u8 << (tag - 1);
            if seen & bit == 0 {
                resolve(descriptor.id(), "params-v1")?;
                check()?;
                seen |= bit;
            }
            if version >= 4 {
                mask_scan.skip(input, check)?;
            }
            metadata(
                initial,
                input.remaining(),
                payload_bytes,
                limits.metadata_bytes,
            )?;
        }
        loop {
            check()?;
            let encoding = input.byte()?;
            if encoding == 0 {
                break;
            }
            if encoding > 5 {
                return Err(codec("unknown compact block encoding"));
            }
            let x = input.number(u64::from(width.div_ceil(TILE_EDGE) - 1))? as u32;
            let y = input.number(u64::from(height.div_ceil(TILE_EDGE) - 1))? as u32;
            let decoded = ((width - x * TILE_EDGE).min(TILE_EDGE)
                * (height - y * TILE_EDGE).min(TILE_EDGE)
                * 4) as usize;
            let length = input.number(BLOCK_BYTES_MAX)? as usize;
            if length == 0 || (encoding == 1 && length != decoded) {
                return Err(codec("invalid stored block length"));
            }
            tiles = tiles
                .checked_add(1)
                .filter(|n| *n <= limits.tiles)
                .ok_or_else(resource)?;
            input.take(length)?;
            payload_bytes = payload_bytes.checked_add(length).ok_or_else(resource)?;
            metadata(
                initial,
                input.remaining(),
                payload_bytes,
                limits.metadata_bytes,
            )?;
        }
        metadata(
            initial,
            input.remaining(),
            payload_bytes,
            limits.metadata_bytes,
        )?;
    }
    if input.remaining() != 0 {
        return Err(codec("trailing compact PCND data"));
    }
    check()
}
fn metadata(initial: usize, remaining: usize, pixels: usize, limit: usize) -> Result<()> {
    if initial
        .checked_sub(remaining)
        .and_then(|n| n.checked_sub(pixels))
        .is_none_or(|n| n > limit.min(DOCUMENT_MAX as usize))
    {
        return Err(resource());
    }
    Ok(())
}

// This borrowed mask scanner shares the adopting reader's canonical inspection
// parser without entering materialization or allocating a coverage realm.
struct MaskScan {
    limits: paintcraft_raster::coverage_mask::Limits,
}
impl MaskScan {
    fn new(limits: paintcraft_raster::coverage_mask::Limits) -> Self {
        Self { limits }
    }
    fn skip(&mut self, input: &mut Input<'_>, check: &dyn Fn() -> Result<()>) -> Result<()> {
        masks::skip_requirements_checked(input, DOCUMENT_MAX as usize, &mut self.limits, check)
    }
}

#[cfg(test)]
mod tests;
