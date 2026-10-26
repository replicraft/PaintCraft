use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, SharedBudget};
use std::sync::Arc;

#[derive(Debug)]
struct Storage {
    alpha: Vec<u8>,
    width: u32,
    height: u32,
    anchor: (u32, u32),
    empty: bool,
    // Storage drops before its declaration releases capacity.
    account: BudgetAccount,
}
#[derive(Clone, Debug)]
pub struct SampledTip(Arc<Storage>);
impl PartialEq for SampledTip {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.width() == other.width()
                && self.height() == other.height()
                && self.anchor() == other.anchor()
                && self.alpha() == other.alpha())
    }
}
impl Eq for SampledTip {}
impl SampledTip {
    /// A sample value is coverage: zero does not paint, 255 is full coverage.
    /// `anchor` identifies the source pixel centered on an integer path point.
    /// The caller supplies the existing resource-admission realm; clones share
    /// one immutable allocation and one declaration, not duplicate pixel data.
    pub fn from_alpha(
        width: u32,
        height: u32,
        anchor: (u32, u32),
        alpha: &[u8],
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if width == 0
            || height == 0
            || width > TIP_SIDE_MAX
            || height > TIP_SIDE_MAX
            || anchor.0 >= width
            || anchor.1 >= height
        {
            return Err(invalid("invalid sampled-tip dimensions or anchor"));
        }
        let pixels = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| limit("sampled-tip extent overflow"))?;
        if pixels > TIP_PIXELS_MAX {
            return Err(limit("sampled-tip pixel allowance exceeded"));
        }
        if alpha.len() != pixels {
            return Err(invalid(
                "sampled-tip alpha length disagrees with dimensions",
            ));
        }
        let metadata = std::mem::size_of::<Storage>() + 2 * std::mem::size_of::<usize>();
        let mut account = budget.account(AllocationClass::SharedBudget);
        account.resize((pixels + metadata) as u64)?;
        let mut data = Vec::new();
        data.try_reserve_exact(pixels)
            .map_err(|_| limit("cannot allocate sampled-tip alpha"))?;
        account.resize((data.capacity() + metadata) as u64)?;
        let mut empty = true;
        for chunk in alpha.chunks(65536) {
            check()?;
            empty &= chunk.iter().all(|value| *value == 0);
            data.extend_from_slice(chunk);
        }
        check()?;
        Ok(Self(Arc::new(Storage {
            alpha: data,
            width,
            height,
            anchor,
            empty,
            account,
        })))
    }
    pub fn width(&self) -> u32 {
        self.0.width
    }
    pub fn height(&self) -> u32 {
        self.0.height
    }
    pub fn anchor(&self) -> (u32, u32) {
        self.0.anchor
    }
    pub fn alpha(&self) -> &[u8] {
        &self.0.alpha
    }
    pub fn is_empty(&self) -> bool {
        self.0.empty
    }
    pub fn declared_bytes(&self) -> u64 {
        self.0.account.bytes()
    }
    pub fn shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
