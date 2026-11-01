//! No raw Arc or Weak escapes. Last storage is freed before its lease can notify.
use super::*;
use std::{ops::Deref, sync::Arc};
#[derive(Debug)]
pub(super) struct Shared<T: 'static>(Option<Arc<T>>);
impl<T: 'static> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(Some(Arc::new(value)))
    }
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(self.0.as_ref().expect("live Source6 owner"))
    }
}
impl<T: 'static> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: 'static> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_deref().expect("live Source6 owner")
    }
}
impl<T: 'static> Drop for Shared<T> {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take()
            && let Some(payload) = Arc::into_inner(owner)
        {
            drop(payload);
        }
    }
}
pub(super) fn arc_bytes<T>() -> Result<u64> {
    let align = std::mem::align_of::<T>().max(std::mem::align_of::<usize>());
    let start = (2 * std::mem::size_of::<usize>())
        .checked_next_multiple_of(std::mem::align_of::<T>())
        .ok_or_else(|| resource("Source6 control alignment"))?;
    let raw = start
        .checked_add(std::mem::size_of::<T>())
        .ok_or_else(|| resource("Source6 control size"))?;
    u64::try_from(
        raw.checked_next_multiple_of(align)
            .ok_or_else(|| resource("Source6 control padding"))?,
    )
    .map_err(|_| resource("Source6 control extent"))
}
