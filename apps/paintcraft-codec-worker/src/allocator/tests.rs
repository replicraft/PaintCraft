use super::*;
use std::{
    alloc::System,
    sync::{Arc, Barrier, atomic::AtomicBool},
    thread,
};

#[test]
fn failed_reservation_cannot_expose_the_old_fetch_sub_underflow_budget() {
    let broken = AtomicUsize::new(8);
    assert_eq!(broken.fetch_sub(16, Ordering::AcqRel), 8);
    // This is the old check-then-undo interleaving: another thread sees a huge
    // unsigned allowance before the failed requester restores its subtraction.
    assert!(broken.fetch_sub(32, Ordering::AcqRel) >= 32);
    let fixed = Limited::<_, false>::new(System, 8);
    assert!(fixed.reserve(8));
    assert!(!fixed.reserve(16));
    assert!(!fixed.reserve(1));
    assert_eq!(fixed.live(), 8);
    fixed.release(8);
    assert_eq!(fixed.live(), 0);
    assert!(fixed.reserve(1));
    assert!(!fixed.reserve(usize::MAX));
    assert_eq!(fixed.live(), 1);
    fixed.release(1);
}
#[test]
fn allocation_zeroing_growth_shrink_failure_and_deallocation_keep_exact_charges() {
    let allocator = Limited::<_, false>::new(System, 64);
    let small = Layout::from_size_align(8, 8).unwrap();
    // SAFETY: each pointer is checked and freed/reallocated with its live layout.
    unsafe {
        let a = allocator.alloc(small);
        assert!(!a.is_null());
        assert_eq!(allocator.live(), 8);
        let z = allocator.alloc_zeroed(small);
        assert!(!z.is_null());
        assert!((0..8).all(|i| z.add(i).read() == 0));
        assert_eq!(allocator.live(), 16);
        let a = allocator.realloc(a, small, 40);
        assert!(!a.is_null());
        assert_eq!(allocator.live(), 48);
        let large = Layout::from_size_align(40, 8).unwrap();
        assert!(allocator.realloc(a, large, 64).is_null());
        assert_eq!(allocator.live(), 48);
        let a = allocator.realloc(a, large, 16);
        assert!(!a.is_null());
        assert_eq!(allocator.live(), 24);
        allocator.dealloc(z, small);
        allocator.dealloc(a, Layout::from_size_align(16, 8).unwrap());
    }
    assert_eq!(allocator.live(), 0);
    assert!(allocator.peak() <= 64);
}
struct Failing {
    refuse: AtomicBool,
}
// SAFETY: tests forward matching System pointers/layouts; refusals preserve the
// original allocation, exactly as GlobalAlloc requires.
unsafe impl GlobalAlloc for Failing {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if self.refuse.load(Ordering::Relaxed) {
            ptr::null_mut()
        } else {
            unsafe { System.alloc(l) }
        }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if self.refuse.load(Ordering::Relaxed) {
            ptr::null_mut()
        } else {
            unsafe { System.alloc_zeroed(l) }
        }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if self.refuse.load(Ordering::Relaxed) {
            ptr::null_mut()
        } else {
            unsafe { System.realloc(p, l, n) }
        }
    }
}
#[test]
fn underlying_failures_restore_charges_for_alloc_zeroed_grow_and_shrink() {
    let a = Limited::<_, false>::new(
        Failing {
            refuse: AtomicBool::new(true),
        },
        128,
    );
    let l = Layout::from_size_align(16, 8).unwrap();
    // SAFETY: only successful allocations are dereferenced/freed, using their
    // current size; deliberately failed reallocations keep the old pointer.
    unsafe {
        assert!(a.alloc(l).is_null());
        assert!(a.alloc_zeroed(l).is_null());
        assert_eq!(a.live(), 0);
        a.inner.refuse.store(false, Ordering::Relaxed);
        let p = a.alloc(l);
        assert!(!p.is_null());
        a.inner.refuse.store(true, Ordering::Relaxed);
        assert!(a.realloc(p, l, 32).is_null());
        assert!(a.realloc(p, l, 8).is_null());
        assert_eq!(a.live(), 16);
        a.dealloc(p, l);
    }
    assert_eq!(a.live(), 0);
}
#[test]
fn concurrent_allocations_rejections_and_frees_never_exceed_requested_limit() {
    let a = Arc::new(Limited::<_, false>::new(System, 256));
    let barrier = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let a = a.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                for size in 1..=2048 {
                    let layout = Layout::from_size_align(size % 64 + 1, 8).unwrap();
                    // SAFETY: matching successful System allocations are immediately freed.
                    unsafe {
                        let p = a.alloc(layout);
                        assert!(a.live() <= 256);
                        if !p.is_null() {
                            p.write(17);
                            thread::yield_now();
                            a.dealloc(p, layout);
                        }
                    }
                    assert!(!a.reserve(257));
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(a.live(), 0);
    assert!(a.peak() <= 256);
}

#[test]
fn actual_global_failure_exits_with_resource_status_and_is_reaped() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "allocator::tests::global_limit_child",
            "--ignored",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(75));
}
#[test]
#[ignore = "only invoked in a reaped child by actual_global_failure_exits_with_resource_status_and_is_reaped"]
fn global_limit_child() {
    let layout = Layout::from_size_align(crate::HEAP.limit + 1, 1).unwrap();
    // SAFETY: a valid layout is passed directly to the real process allocator.
    // Its nonallocating refusal terminates this isolated test child with 75.
    unsafe {
        crate::HEAP.alloc(layout);
    }
    panic!("global memory cap did not terminate child");
}
