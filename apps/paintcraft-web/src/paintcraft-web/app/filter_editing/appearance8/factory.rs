//! Exact original Main factory. No available-footprint or JS scalar grant.
use super::denied;
use paintcraft_core::types::{
    Result,
    resources::{AllocationClass, BudgetAccount, BudgetScope},
};
use paintcraft_raster::Admission;
use std::rc::Rc;

pub(super) const MAX: u64 = 64 << 20;

pub(super) struct Credit8 {
    #[cfg(target_arch = "wasm32")]
    value: Option<wasm_bindgen::JsValue>,
    #[cfg(all(test, not(target_arch = "wasm32")))]
    observed: Rc<std::cell::Cell<u32>>,
}
impl std::fmt::Debug for Credit8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainCredit8").finish_non_exhaustive()
    }
}
#[cfg(target_arch = "wasm32")]
impl Credit8 {
    pub fn new(value: wasm_bindgen::JsValue) -> Result<Self> {
        use wasm_bindgen::JsCast;
        if !js_sys::Reflect::get(&value, &"release".into())
            .ok()
            .is_some_and(|f| f.dyn_ref::<js_sys::Function>().is_some())
        {
            return Err(denied());
        }
        Ok(Self { value: Some(value) })
    }
}
impl Drop for Credit8 {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(value) = self.value.take()
                && super::js::defer_credit(&value).is_err()
            {
                // Scheduling failure never falls back to synchronous release.
                // Preserve the actual grant handle and fail closed until page teardown.
                std::mem::forget(value);
            }
        }
        #[cfg(all(test, not(target_arch = "wasm32")))]
        self.observed.set(self.observed.get() + 1);
    }
}
#[derive(Debug)]
pub(super) struct FactoryData8 {
    // All prospective Main owned structures are named by the containing guard.
    // Data accounts die before the original factory, which precedes JS credit.
    _controls: BudgetAccount,
    scope: BudgetScope,
    cap: u64,
    quarantined: std::cell::Cell<bool>,
    _credit: Credit8,
}
#[derive(Debug)]
pub(super) struct Factory8(Option<Rc<FactoryData8>>);
impl Clone for Factory8 {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for Factory8 {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take()
            && let Some(data) = Rc::into_inner(owner)
        {
            // No Weak/raw Rc escapes. The final Rc control block retires before
            // original accounts/factory/credit can notify release observers.
            drop(data);
        }
    }
}
/// The caller already holds the genuine single broker grant for this cap.
/// Factory/reservation/bootstrap bookkeeping is the explicitly bounded setup
/// boundary; variable Main data cannot precede the returned original factory.
pub(super) fn original_scope(admission: &Admission, cap: u64) -> Result<BudgetScope> {
    if cap == 0 || cap > MAX || cap > admission.paint_metadata_limit() as u64 {
        return Err(denied());
    }
    let original = admission.paint_metadata_scope();
    let capacity = original.capacity_snapshot();
    if capacity.reserved {
        if capacity.ceiling != cap {
            return Err(denied());
        }
        return Ok(original);
    }
    if cap > capacity.ceiling {
        return Err(denied());
    }
    // ONLY the genuinely unreserved trusted host branch may acquire a new
    // reservation, and only from the original supplied budget. No new pool.
    Ok(original
        .budget()
        .reserve_capacity(cap)?
        .scope(AllocationClass::PaintMetadata))
}
impl Factory8 {
    pub fn new(admission: &Admission, cap: u64, credit: Credit8) -> Result<Self> {
        let scope = original_scope(admission, cap)?;
        let mut controls = scope.account();
        controls.resize(super::controls::factory_bytes()?)?;
        Ok(Self(Some(Rc::new(FactoryData8 {
            _controls: controls,
            scope,
            cap,
            quarantined: std::cell::Cell::new(false),
            _credit: credit,
        }))))
    }
    pub fn scope(&self) -> &BudgetScope {
        &self.0.as_ref().expect("live Main factory").scope
    }
    pub fn quarantine(&self) {
        self.0
            .as_ref()
            .expect("live Main factory")
            .quarantined
            .set(true);
    }
    pub fn is_quarantined(&self) -> bool {
        self.0
            .as_ref()
            .expect("live Main factory")
            .quarantined
            .get()
    }
    pub fn cap(&self) -> u64 {
        self.0.as_ref().expect("live Main factory").cap
    }
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub fn observed(
        admission: &Admission,
        cap: u64,
        releases: Rc<std::cell::Cell<u32>>,
    ) -> Result<Self> {
        Self::new(admission, cap, Credit8 { observed: releases })
    }
}
