//! Prospective Main JS/stage/image overlap in one original factory account.
use super::{denied, operation::Operation8, publication::CpuPublication8};
use eframe::egui::{Color32, ColorImage};
use paintcraft_core::types::{Result, resources::BudgetAccount};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Debug)]
pub(super) struct FundingData8 {
    account: RefCell<Option<BudgetAccount>>,
    operation: Operation8,
    bytes: u64,
    transport: u64,
    controls: u64,
    js: Cell<bool>,
    stage: Cell<bool>,
    image: Cell<bool>,
}
#[derive(Debug)]
pub(super) struct Funding8(Option<Rc<FundingData8>>);
impl Clone for Funding8 {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for Funding8 {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take()
            && let Some(data) = Rc::into_inner(owner)
        {
            drop(data);
        }
    }
}
impl Funding8 {
    fn data(&self) -> &FundingData8 {
        self.0.as_ref().expect("live Main frame funding")
    }
    fn resize(&self) -> Result<()> {
        let data = self.data();
        let copies =
            u64::from(data.js.get()) + u64::from(data.stage.get()) + u64::from(data.image.get());
        let amount = data
            .bytes
            .checked_mul(copies)
            .and_then(|n| n.checked_add(if data.js.get() { data.transport } else { 0 }))
            .and_then(|n| n.checked_add(data.controls))
            .ok_or_else(denied)?;
        // No RefCell borrow crosses budget callbacks. Reentry cannot take the
        // same account while temporarily absent; it gets a typed refusal.
        let mut account = data.account.borrow_mut().take().ok_or_else(denied)?;
        let result = account.resize(amount);
        *data.account.borrow_mut() = Some(account);
        result
    }
    pub fn js_retired(&self) -> Result<()> {
        self.data().js.set(false);
        self.resize()
    }
    fn stage_retired(&self) -> Result<()> {
        self.data().stage.set(false);
        self.resize()
    }
    pub fn image_retired(&self) -> Result<()> {
        self.data().image.set(false);
        self.resize()
    }
    pub fn spend(&self, n: u64) -> Result<()> {
        self.data().operation.spend(n)
    }
    pub fn factory(&self) -> &super::factory::Factory8 {
        self.data().operation.factory()
    }
    pub fn js_is_retired(&self) -> bool {
        !self.data().js.get()
    }
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub fn used(&self) -> u64 {
        self.data().operation.used()
    }
}
#[derive(Debug)]
pub(super) struct FrameStage8 {
    bytes: Option<Vec<u8>>,
    funding: Funding8,
    size: [usize; 2],
    converted: bool,
}
impl FrameStage8 {
    /// Called before JS creates its final ArrayBuffer. No perpatch counter.
    pub fn admit(operation: Operation8, size: [u32; 2]) -> Result<Self> {
        if operation.factory().is_quarantined() {
            return Err(denied());
        }
        operation.spend(131072)?;
        super::controls::validate()?;
        operation.spend(64)?;
        let pixels = u64::from(size[0])
            .checked_mul(u64::from(size[1]))
            .filter(|n| *n != 0)
            .ok_or_else(denied)?;
        let bytes = pixels.checked_mul(4).ok_or_else(denied)?;
        let columns = u64::from(size[0]).div_ceil(64);
        let patches = columns
            .checked_mul(u64::from(size[1]).div_ceil(64))
            .ok_or_else(denied)?;
        let chunks = bytes.div_ceil(65536);
        let matches = patches
            .checked_mul(4)
            .and_then(|n| n.checked_add(8))
            .ok_or_else(denied)?;
        let checks = columns
            .checked_mul(u64::from(size[1]))
            .and_then(|n| n.checked_add(2 * chunks))
            .ok_or_else(denied)?;
        //43D consumer pixels +3D trusted coordinator producertransport. This
        // does NOT claim those JS actions are in WorkerRust reported counters.
        let work = bytes
            .checked_mul(46)
            .and_then(|n| n.checked_add(matches.checked_mul(super::controls::match_work())?))
            .and_then(|n| n.checked_add(checks.checked_mul(1024)?))
            .ok_or_else(denied)?;
        operation.spend(work)?;
        let controls = super::controls::frame_bytes()?;
        let total = bytes
            .checked_mul(3)
            .and_then(|n| n.checked_add(bytes.min(16384)))
            .and_then(|n| n.checked_add(controls))
            .ok_or_else(denied)?;
        let mut account = operation.factory().scope().account();
        account.resize(total)?;
        if operation.factory().is_quarantined() {
            return Err(denied());
        }
        let mut raw = Vec::new();
        let length = usize::try_from(bytes).map_err(|_| denied())?;
        raw.try_reserve_exact(length).map_err(|_| denied())?;
        if raw.capacity() != length {
            return Err(denied());
        }
        raw.resize(length, 0);
        let funding = Funding8(Some(Rc::new(FundingData8 {
            account: RefCell::new(Some(account)),
            operation,
            bytes,
            transport: bytes.min(16384),
            controls,
            js: Cell::new(true),
            stage: Cell::new(true),
            image: Cell::new(true),
        })));
        Ok(Self {
            bytes: Some(raw),
            funding,
            size: [size[0] as usize, size[1] as usize],
            converted: false,
        })
    }
    pub fn funding(&self) -> Funding8 {
        self.funding.clone()
    }
    pub fn byte_len(&self) -> usize {
        self.bytes.as_ref().map_or(0, Vec::len)
    }
    /// No owning Vec is accepted/extracted across the JS ABI. The genuine
    /// bridge uses Uint8Array::copy_to on this already initialized slice.
    #[cfg(target_arch = "wasm32")]
    pub fn fill(&mut self, bytes: &js_sys::Uint8Array) -> Result<()> {
        if bytes.length() as usize != self.byte_len() {
            return Err(denied());
        }
        bytes.copy_to(self.bytes.as_mut().ok_or_else(denied)?.as_mut_slice());
        Ok(())
    }
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub fn fill_test(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() != self.byte_len() {
            return Err(denied());
        }
        self.bytes
            .as_mut()
            .ok_or_else(denied)?
            .copy_from_slice(bytes);
        Ok(())
    }
    pub fn into_publication(mut self) -> Result<CpuPublication8> {
        let raw = self.bytes.take().ok_or_else(denied)?;
        let length = raw.len() / 4;
        let mut colors = Vec::new();
        colors.try_reserve_exact(length).map_err(|_| denied())?;
        if colors.capacity() != length {
            return Err(denied());
        }
        for pixel in raw.as_chunks::<4>().0 {
            colors.push(Color32::from_rgba_unmultiplied(
                pixel[0], pixel[1], pixel[2], pixel[3],
            ));
        }
        let image = ColorImage::new(self.size, colors);
        drop(raw);
        self.funding.stage_retired()?;
        self.converted = true;
        Ok(CpuPublication8::new(image, self.funding.clone()))
    }
}
impl Drop for FrameStage8 {
    fn drop(&mut self) {
        drop(self.bytes.take());
        let _ = self.funding.stage_retired();
        if !self.converted {
            let _ = self.funding.image_retired();
        }
    }
}
