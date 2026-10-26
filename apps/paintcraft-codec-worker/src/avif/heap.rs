//! Executable-only C ABI allocation boundary, sharing the existing Limited heap.
//! Native caller pointers must originate here; no allocator interposition or
//! system-library fallback is used. These functions never unwind across C.
#![allow(unsafe_code)]
use std::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    ptr,
};
#[derive(Clone, Copy)]
#[repr(C)]
struct Header {
    magic: usize,
    size: usize,
    alignment: usize,
    offset: usize,
}
const MAGIC: usize = 0x5043444156314845;
fn layout(size: usize, alignment: usize) -> Option<(Layout, usize)> {
    if !alignment.is_power_of_two() {
        return None;
    }
    let alignment = alignment.max(std::mem::align_of::<Header>());
    if !alignment.is_power_of_two() {
        return None;
    }
    let offset = std::mem::size_of::<Header>().checked_add(alignment - 1)? & !(alignment - 1);
    let total = offset.checked_add(size.max(1))?;
    Some((Layout::from_size_align(total, alignment).ok()?, offset))
}
unsafe fn allocate<A: GlobalAlloc>(allocator: &A, size: usize, alignment: usize) -> *mut u8 {
    let Some((layout, offset)) = layout(size, alignment) else {
        return ptr::null_mut();
    };
    // SAFETY: layout is nonzero and validated; returned pointer remains owned by A.
    let base = unsafe { allocator.alloc(layout) };
    if base.is_null() {
        return base;
    }
    // SAFETY: offset/header are inside the allocated layout and satisfy alignment.
    unsafe {
        let user = base.add(offset);
        user.sub(std::mem::size_of::<Header>())
            .cast::<Header>()
            .write(Header {
                magic: MAGIC,
                size,
                alignment: layout.align(),
                offset,
            });
        user
    }
}
unsafe fn header(pointer: *mut u8) -> Header {
    // SAFETY: only live pointers obtained from allocate may cross this ABI.
    let value = unsafe {
        pointer
            .sub(std::mem::size_of::<Header>())
            .cast::<Header>()
            .read()
    };
    if value.magic != MAGIC {
        crate::allocator::nonallocating_exit(2);
    }
    value
}
unsafe fn release<A: GlobalAlloc>(allocator: &A, pointer: *mut u8) {
    if pointer.is_null() {
        return;
    }
    // SAFETY: native free contract supplies a live allocation from this boundary.
    let value = unsafe { header(pointer) };
    let Some((layout, offset)) = layout(value.size, value.alignment) else {
        crate::allocator::nonallocating_exit(2)
    };
    if offset != value.offset {
        crate::allocator::nonallocating_exit(2);
    }
    // SAFETY: reconstruct the exact original allocator pointer and layout.
    unsafe { allocator.dealloc(pointer.sub(offset), layout) };
}
unsafe fn resize<A: GlobalAlloc>(allocator: &A, pointer: *mut u8, size: usize) -> *mut u8 {
    if pointer.is_null() {
        return unsafe { allocate(allocator, size, 16) };
    }
    if size == 0 {
        unsafe { release(allocator, pointer) };
        return ptr::null_mut();
    }
    // SAFETY: realloc receives an allocation owned by this ABI.
    let old = unsafe { header(pointer) };
    let next = unsafe { allocate(allocator, size, old.alignment) };
    if next.is_null() {
        return next;
    } // The old allocation remains valid on failure.
    // SAFETY: allocations are distinct and at least min(old,new) bytes long.
    unsafe {
        ptr::copy_nonoverlapping(pointer, next, old.size.min(size));
        release(allocator, pointer)
    };
    next
}
#[unsafe(no_mangle)]
unsafe extern "C" fn paintcraft_dav1d_alloc(
    size: usize,
    alignment: usize,
    kind: u32,
) -> *mut c_void {
    fail_kind(kind, size, alignment);
    // SAFETY: all layout construction and ownership stay within this module.
    let value = unsafe { allocate(&crate::HEAP, size, alignment) };
    if value.is_null() {
        crate::allocator::nonallocating_exit(75);
    }
    value.cast()
}
#[unsafe(no_mangle)]
unsafe extern "C" fn paintcraft_dav1d_realloc(
    pointer: *mut c_void,
    size: usize,
    kind: u32,
) -> *mut c_void {
    fail_kind(kind, size, 16);
    // SAFETY: pointers come only from the selected patched static decoder.
    let value = unsafe { resize(&crate::HEAP, pointer.cast(), size) };
    if value.is_null() && size != 0 {
        crate::allocator::nonallocating_exit(75);
    }
    value.cast()
}
#[unsafe(no_mangle)]
unsafe extern "C" fn paintcraft_dav1d_free(pointer: *mut c_void) {
    // SAFETY: the selected decoder frees only pointers obtained here, or NULL.
    unsafe { release(&crate::HEAP, pointer.cast()) };
}
fn fail_kind(kind: u32, size: usize, alignment: usize) {
    #[cfg(test)]
    if FAIL_KIND.load(std::sync::atomic::Ordering::Relaxed) == kind {
        FAIL_KIND.store(u32::MAX, std::sync::atomic::Ordering::Relaxed);
        // Real quota proof, not a stubbed decoder error: consume enough of the
        // same allocator's remaining capacity that the actual next C request
        // exceeds it by one byte. The subprocess must take Limited's real exit.
        if let Some((request, _)) = layout(size, alignment) {
            let remaining = (paintcraft_formats::codec_worker::HEAP_BYTES as usize)
                .saturating_sub(crate::HEAP.live());
            if remaining >= request.size() {
                let fill = remaining - request.size() + 1;
                if let Ok(layout) = Layout::from_size_align(fill, 16) {
                    // SAFETY: valid nonzero layout. Intentionally held until
                    // the imminent quota exit in this private child fixture.
                    let _held = unsafe { crate::HEAP.alloc(layout) };
                }
            }
        }
    }
    let _ = (kind, size, alignment);
}
#[cfg(test)]
pub(super) static FAIL_KIND: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(u32::MAX);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_layout_alignment_realloc_failure_and_zero_are_accounted_once() {
        let allocator = crate::allocator::Limited::<_, false>::new(std::alloc::System, 512);
        // SAFETY: this test follows the private boundary's pointer contracts.
        unsafe {
            let p = allocate(&allocator, 64, 64);
            assert!(!p.is_null());
            assert_eq!(p as usize % 64, 0);
            p.write_bytes(7, 64);
            let charged = allocator.live();
            assert!(resize(&allocator, p, 4096).is_null());
            assert_eq!(allocator.live(), charged);
            assert_eq!(*p, 7);
            let q = resize(&allocator, p, 96);
            assert!(!q.is_null());
            assert_eq!(q as usize % 64, 0);
            assert_eq!(*q, 7);
            assert!(resize(&allocator, q, 0).is_null());
            assert_eq!(allocator.live(), 0);
            let zero = allocate(&allocator, 0, 16);
            assert!(!zero.is_null());
            release(&allocator, zero);
            assert_eq!(allocator.live(), 0);
            assert!(allocate(&allocator, usize::MAX, 64).is_null());
            assert!(allocate(&allocator, 1, 3).is_null());
        }
    }
}
