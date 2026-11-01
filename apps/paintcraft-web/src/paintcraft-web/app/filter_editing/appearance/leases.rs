//! One existing PaintMetadata reservation, backed by the same JS broker credit.
use paintcraft_core::types::{Error, ErrorCode, Result, resources::BudgetScope};
use paintcraft_raster::Admission;

pub(in crate::app::filter_editing) const MAIN_PREPARATION_MAX: u64 = 64 << 20;

/// No new realm: the injected Admission supplies the existing global budget.
/// Caller has synchronously obtained the corresponding JS reserveMain credit
/// BEFORE invoking this, and must hold it until every scope/account/storage dies.
pub(in crate::app::filter_editing) fn reserved_scope(
    admission: &Admission,
    quote: u64,
) -> Result<BudgetScope> {
    if quote > MAIN_PREPARATION_MAX || quote > admission.paint_metadata_limit() as u64 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Source7 Main preparation byte limit",
        ));
    }
    // Preserve the exact injected reservation and shared footprint. Reaching
    // through budget() would escape a caller's pre-reserved capacity.
    Ok(admission.paint_metadata_scope())
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use js_sys::{Function, Reflect};
    use std::rc::Rc;
    use wasm_bindgen::{JsCast, JsValue};

    /// Only shared through Rc. The transport's release is idempotent; this owner
    /// invokes it once after the final retained Main allocation has gone.
    #[derive(Debug)]
    pub(crate) struct Credit7(JsValue);
    impl Credit7 {
        pub fn new(handle: JsValue) -> Result<Rc<Self>> {
            if !Reflect::get(&handle, &"release".into())
                .ok()
                .is_some_and(|f| f.is_function())
            {
                return Err(Error::new(
                    ErrorCode::CodecFailed,
                    "Source7 Main credit handle",
                ));
            }
            Ok(Rc::new(Self(handle)))
        }
    }
    impl Drop for Credit7 {
        fn drop(&mut self) {
            if let Ok(function) =
                Reflect::get(&self.0, &"release".into()).and_then(|f| f.dyn_into::<Function>())
            {
                let _ = function.call0(&self.0);
            }
        }
    }

    /// Field drop order is intentional: actual storage (including its account)
    /// precedes unused reserved capacity, which precedes the broker credit.
    /// No into_parts escape can detach a Vec/account from its shared credit.
    #[derive(Debug)]
    pub(crate) struct Retained7<T> {
        storage: T,
        _scope: BudgetScope,
        _credit: Rc<Credit7>,
    }
    impl<T> Retained7<T> {
        pub fn new(storage: T, scope: BudgetScope, credit: Rc<Credit7>) -> Self {
            Self {
                storage,
                _scope: scope,
                _credit: credit,
            }
        }
        pub fn get(&self) -> &T {
            &self.storage
        }
        pub fn get_mut(&mut self) -> &mut T {
            &mut self.storage
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub(in crate::app::filter_editing) use browser::{Credit7, Retained7};
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    #[test]
    fn metadata_factory_preserves_injected_reservation_without_borrowing_global_headroom() {
        let budget = SharedBudget::new(65536);
        let original = budget
            .reserve_capacity(4096)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let admission = Admission::new(0).with_paint_metadata_scope(original.clone());
        let captured = reserved_scope(&admission, 4096).unwrap();
        let mut first = captured.account();
        first.resize(4096).unwrap();
        assert_eq!(original.footprint().live, 4096);
        let mut next = admission.paint_metadata_scope().account();
        assert_eq!(next.resize(1).unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(budget.usage().reserved, 4096); // much larger global headroom cannot escape reservation
        drop(first);
        next.resize(1).unwrap();
        assert_eq!(original.footprint().live, 1);
        drop(next);
        assert_eq!(original.footprint().live, 0);
    }
}
