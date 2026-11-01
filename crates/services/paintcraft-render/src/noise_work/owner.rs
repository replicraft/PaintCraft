//! Private paired storage ownership. No Weak or raw Arc escapes this module.
use super::*;
use std::{ops::Deref, sync::Arc};
#[derive(Debug)]
pub(super) struct SharedOwner<T: 'static>(Option<Arc<T>>);
impl<T: 'static> SharedOwner<T> {
    pub(super) fn new(value: T) -> Self {
        Self(Some(Arc::new(value)))
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(
            self.0.as_ref().expect("live owner"),
            other.0.as_ref().expect("live owner"),
        )
    }
}
impl<T: 'static> Clone for SharedOwner<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: 'static> Deref for SharedOwner<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_deref().expect("live owner")
    }
}
impl<T: 'static> Drop for SharedOwner<T> {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            // ALL clones take this path; Arc::into_inner gives exactly one owner
            // the payload even under concurrent drops. No Weak can retain its
            // allocation. Both the Arc block and implicit weak are released
            // BEFORE the returned payload/account is dropped (and can notify).
            if let Some(value) = Arc::into_inner(owner) {
                #[cfg(test)]
                observation::deallocated::<T>();
                drop(value);
            }
        }
    }
}
pub(super) fn arc_bytes<T>() -> Result<u64> {
    let align = std::mem::align_of::<T>().max(std::mem::align_of::<usize>());
    let header = 2usize
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(limit)?;
    let start = header
        .checked_next_multiple_of(std::mem::align_of::<T>())
        .ok_or_else(limit)?;
    let raw = start
        .checked_add(std::mem::size_of::<T>())
        .ok_or_else(limit)?;
    u64::try_from(raw.checked_next_multiple_of(align).ok_or_else(limit)?).map_err(|_| limit())
}
#[cfg(test)]
pub(super) mod observation {
    use std::{
        any::TypeId,
        cell::RefCell,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    type Flags = (Arc<AtomicBool>, Arc<AtomicBool>);
    thread_local! {static FLAGS: RefCell<Option<Flags>>=const{RefCell::new(None)};}
    pub(super) fn deallocated<T: 'static>() {
        FLAGS.with(|f| {
            if let Some((counter, charge)) = f.borrow().as_ref() {
                if TypeId::of::<T>()
                    == TypeId::of::<paintcraft_core::types::resources::BudgetAccount>()
                {
                    charge.store(true, Ordering::Release);
                } else {
                    counter.store(true, Ordering::Release);
                }
            }
        });
    }
    pub(crate) struct Restore(Option<Flags>);
    impl Drop for Restore {
        fn drop(&mut self) {
            FLAGS.with(|f| *f.borrow_mut() = self.0.take());
        }
    }
    pub(crate) fn install(counter: Arc<AtomicBool>, charge: Arc<AtomicBool>) -> Restore {
        Restore(FLAGS.with(|f| f.replace(Some((counter, charge)))))
    }
}
