//! Executable-only unsafe boundary: forward the GlobalAlloc contract to System.
//! The CAS charge never wraps or temporarily exposes spare bytes on rejection.
//! This caps live REQUESTED Rust heap bytes through this allocator, not RSS,
//! native allocator overhead, mappings, thread stacks or direct System calls.
#![allow(unsafe_code)]
use std::{
    alloc::{GlobalAlloc, Layout},
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
pub struct Limited<A, const EXIT_ON_FAILURE: bool> {
    inner: A,
    limit: usize,
    live: AtomicUsize,
    peak: AtomicUsize,
}
impl<A, const EXIT_ON_FAILURE: bool> Limited<A, EXIT_ON_FAILURE> {
    pub const fn new(inner: A, limit: usize) -> Self {
        Self {
            inner,
            limit,
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }
    pub fn live(&self) -> usize {
        self.live.load(Ordering::Acquire)
    }
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::Acquire)
    }
    fn reserve(&self, bytes: usize) -> bool {
        let mut used = self.live.load(Ordering::Acquire);
        loop {
            let Some(next) = used.checked_add(bytes).filter(|next| *next <= self.limit) else {
                return false;
            };
            match self
                .live
                .compare_exchange_weak(used, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {
                    self.peak.fetch_max(next, Ordering::Relaxed);
                    return true;
                }
                Err(actual) => used = actual,
            }
        }
    }
    fn release(&self, bytes: usize) {
        if self.live.fetch_sub(bytes, Ordering::AcqRel) < bytes {
            std::process::abort();
        }
    }
}
/// Terminate this dedicated executable without allocation, locks or unwinding.
/// Used for both allocator refusal and loss of the supervising parent's pipe.
pub(crate) fn nonallocating_exit(status: i32) -> ! {
    // SAFETY: no pointer/lifetime invariants are needed to terminate this process.
    // No destructors or buffered logging run, even when another thread holds locks.
    unsafe { libc::_exit(status) }
}
/// A fixed, allocation-free status. _exit skips Rust/C destructors and buffered
/// logging, so failure while holding allocator/stdout locks cannot deadlock.
fn refused<const EXIT: bool>() -> *mut u8 {
    if EXIT {
        nonallocating_exit(75)
    } else {
        ptr::null_mut()
    }
}
// SAFETY: the wrapped allocator owns every pointer. We forward the original
// pointer/layout unchanged; failed allocation/reallocation restores its charge,
// shrinking releases only after success, and deallocation releases exactly the
// caller's original valid layout. Bookkeeping uses no allocations or unwinding.
unsafe impl<A: GlobalAlloc, const EXIT_ON_FAILURE: bool> GlobalAlloc
    for Limited<A, EXIT_ON_FAILURE>
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.reserve(layout.size()) {
            return refused::<EXIT_ON_FAILURE>();
        }
        // SAFETY: GlobalAlloc caller supplied a valid nonzero layout.
        let result = unsafe { self.inner.alloc(layout) };
        if result.is_null() {
            self.release(layout.size());
            return refused::<EXIT_ON_FAILURE>();
        }
        result
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !self.reserve(layout.size()) {
            return refused::<EXIT_ON_FAILURE>();
        }
        // SAFETY: identical valid layout; delegate zero initialization.
        let result = unsafe { self.inner.alloc_zeroed(layout) };
        if result.is_null() {
            self.release(layout.size());
            return refused::<EXIT_ON_FAILURE>();
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: caller promises this pointer/layout came from this allocator.
        unsafe { self.inner.dealloc(pointer, layout) };
        self.release(layout.size());
    }
    unsafe fn realloc(&self, pointer: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        let growth = new_size.saturating_sub(old.size());
        if !self.reserve(growth) {
            return refused::<EXIT_ON_FAILURE>();
        }
        // SAFETY: caller promises a live old allocation and valid nonzero new
        // size. A null result leaves the old allocation owned and charged.
        let result = unsafe { self.inner.realloc(pointer, old, new_size) };
        if result.is_null() {
            self.release(growth);
            return refused::<EXIT_ON_FAILURE>();
        } else if new_size < old.size() {
            self.release(old.size() - new_size);
        }
        result
    }
}
#[cfg(test)]
mod tests;
