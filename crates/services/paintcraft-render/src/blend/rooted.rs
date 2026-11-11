//! Source8-only rooted structural controls. No public boolean/proof escape.
use crate::{Document, Result};
use paintcraft_core::types::{Error, ErrorCode, filters::Descriptor};
const ROOT: u64 = 2048;
/// Borrowed immutable witness, constructed and consumed within one actual call.
#[repr(C)]
pub(crate) struct RootFacts<'a> {
    doc: &'a Document,
    direct: bool,
    coordinate: bool,
    mask: bool,
    capture_work: u64,
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(super::limit)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(super::limit)
}
pub(crate) fn quote_loop_work(doc: &Document) -> Result<u64> {
    mul(ROOT, doc.roots.len() as u64)
}
impl<'a> RootFacts<'a> {
    pub fn capture(
        doc: &'a Document,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let n = doc.roots.len();
        if n > paintcraft_document::LAYER_COUNT_MAX || n != doc.layers.len() {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Rooted appearance shape differs",
            ));
        }
        let pairs = mul(n as u64, n.saturating_sub(1) as u64)? / 2;
        let prefix = add(add(64, quote_loop_work(doc)?)?, mul(18, pairs)?)?;
        spend(prefix)?;
        let mut facts = Self {
            doc,
            direct: false,
            coordinate: false,
            mask: false,
            capture_work: prefix,
        };
        for (i, id) in doc.roots.iter().enumerate() {
            check()?;
            if doc.roots[..i].contains(id) {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Rooted appearance root repeats",
                ));
            }
            let layer = doc.layers.get(id).filter(|l| l.id == *id).ok_or_else(|| {
                Error::new(
                    ErrorCode::InputSchema,
                    "Rooted appearance root identity differs",
                )
            })?;
            if layer.filters.len() > 256 {
                return Err(super::limit());
            }
            let descriptors = mul(16, layer.filters.len() as u64)?;
            spend(descriptors)?;
            facts.capture_work = add(facts.capture_work, descriptors)?;
            for filter in layer.filters.iter() {
                check()?;
                if filter.enabled() {
                    facts.direct |= crate::direct::priced(filter.descriptor());
                    facts.coordinate |= matches!(filter.descriptor(), Descriptor::AddNoiseRgbV1(_));
                }
            }
            // O(1) closed stack fact within the existing paid root lookup;
            // even disabled intentions retain original-Base traversal admission.
            facts.direct |= crate::styles::stored(layer);
            facts.mask |= crate::layer_mask::active(layer);
        }
        check()?;
        Ok(facts)
    }
    pub fn has_direct(&self) -> bool {
        self.direct
    }
    pub fn has_coordinate(&self) -> bool {
        self.coordinate
    }
    pub fn has_mask(&self) -> bool {
        self.mask
    }
    pub fn document(&self) -> &'a Document {
        self.doc
    }
    pub fn pass_work(&self, passes: u64) -> Result<u64> {
        mul(quote_loop_work(self.doc)?, passes)
    }
    pub fn traversal_work(&self) -> Result<u64> {
        add(
            self.capture_work,
            self.pass_work(if self.coordinate { 4 } else { 5 })?,
        )
    }
    pub fn require_current(&self) -> Result<()> {
        if (self.direct || self.mask) && crate::spatial::current().is_none() {
            return Err(super::missing());
        }
        if self.coordinate {
            crate::noise_work::required()?;
        }
        Ok(())
    }
}
pub(crate) const fn control_bytes() -> u64 {
    std::mem::size_of::<RootFacts<'static>>() as u64
}
