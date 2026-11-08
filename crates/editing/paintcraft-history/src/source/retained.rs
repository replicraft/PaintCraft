//! Private storage/account ownership. No unused reservation factory or Weak escape.
use super::*;
use std::{ops::Deref, sync::Arc};
/// Actual retained History precedes its unique assigned declaration. This owner
/// is immutable; funded mutation/publication requires a separately reviewed seam.
#[derive(Debug)]
pub struct RetainedHistoryStorage {
    history: History,
    account: BudgetAccount,
}
impl RetainedHistoryStorage {
    pub(super) fn from_admitted(history: History, account: BudgetAccount) -> Self {
        Self { history, account }
    }
    pub fn history(&self) -> &History {
        &self.history
    }
    pub fn assigned_bytes(&self) -> u64 {
        self.account.bytes()
    }
}
#[derive(Debug)]
pub(crate) struct ForkRejected {
    pub history: History,
    pub account: BudgetAccount,
    pub error: Error,
}
#[allow(
    clippy::result_large_err,
    reason = "Refusal returns original owned storage without allocating an error box"
)]
pub(crate) fn from_fork_parts(
    history: History,
    account: BudgetAccount,
) -> std::result::Result<RetainedHistoryStorage, ForkRejected> {
    let mut storage = RetainedHistoryStorage { history, account };
    let reconciled = (|| {
        let assigned = super::quote::retained(storage.history())?;
        // The qualified fork already admitted its larger owner+factory control.
        // A handoff must only shrink, never silently obtain another grant.
        if assigned > storage.account.bytes() {
            return Err(archive::limit("History handoff would need new capacity"));
        }
        storage.account.resize(assigned)
    })();
    match reconciled {
        Ok(()) => Ok(storage),
        Err(error) => {
            let RetainedHistoryStorage { history, account } = storage;
            Err(ForkRejected {
                history,
                account,
                error,
            })
        }
    }
}
/// All clones retire through into_inner. The last Arc block is released BEFORE
/// its payload/accounts can notify callbacks, matching the qualified Noise owner.
#[derive(Debug)]
pub(super) struct SharedOwner<T: 'static>(Option<Arc<T>>);
impl<T: 'static> SharedOwner<T> {
    pub(super) fn new(value: T) -> Self {
        Self(Some(Arc::new(value)))
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(
            self.0.as_ref().expect("live source owner"),
            other.0.as_ref().expect("live source owner"),
        )
    }
    pub(super) fn try_into_inner(mut self) -> std::result::Result<T, Self> {
        let arc = self.0.take().expect("live source owner");
        match Arc::try_unwrap(arc) {
            Ok(value) => {
                #[cfg(test)]
                observation::deallocated();
                Ok(value)
            }
            Err(arc) => {
                self.0 = Some(arc);
                Err(self)
            }
        }
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
        self.0.as_deref().expect("live source owner")
    }
}
impl<T: 'static> Drop for SharedOwner<T> {
    fn drop(&mut self) {
        if let Some(arc) = self.0.take()
            && let Some(value) = Arc::into_inner(arc)
        {
            #[cfg(test)]
            observation::deallocated();
            drop(value);
        }
    }
}
#[cfg(test)]
pub(super) mod observation {
    use std::{
        cell::RefCell,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    thread_local! {static FLAG: RefCell<Option<Arc<AtomicBool>>> = const {RefCell::new(None)};}
    pub(super) fn deallocated() {
        FLAG.with(|flag| {
            if let Some(flag) = flag.borrow().as_ref() {
                flag.store(true, Ordering::Release);
            }
        });
    }
    pub(crate) struct Restore(Option<Arc<AtomicBool>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            FLAG.with(|flag| *flag.borrow_mut() = self.0.take());
        }
    }
    pub(crate) fn install(flag: Arc<AtomicBool>) -> Restore {
        Restore(FLAG.with(|old| old.replace(Some(flag))))
    }
}
