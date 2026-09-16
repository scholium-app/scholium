//! 堆分配计数器。
//!
//! "内存不爆" 需要可复现的数字，不能靠 `top` 目测。这里给 `System` 分配器包一层原子计数，
//! 记录当前占用与峰值，规模判据用的是 "峰值相对基线的增量"。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// 统计堆占用的分配器。
#[derive(Debug)]
pub(crate) struct CountingAllocator;

/// 当前堆占用字节数。
pub(crate) fn current() -> usize {
    CURRENT.load(Ordering::Relaxed)
}

/// 自上次 [`reset_peak`] 以来的堆占用峰值。
pub(crate) fn peak() -> usize {
    PEAK.load(Ordering::Relaxed)
}

/// 把峰值重置为当前占用，用于测量一段区间的峰值增量。
pub(crate) fn reset_peak() {
    PEAK.store(current(), Ordering::Relaxed);
}

fn grow(size: usize) {
    let now = CURRENT.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

fn shrink(size: usize) {
    CURRENT.fetch_sub(size, Ordering::Relaxed);
}

// SAFETY: 本实现只在 `System` 调用之外增减两个原子计数器，不改变指针、内存布局或生命周期，
// 在分配失败时也返回空指针，因此满足 `GlobalAlloc` 的安全契约。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(layout.size());
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            if new_size >= layout.size() {
                grow(new_size - layout.size());
            } else {
                shrink(layout.size() - new_size);
            }
        }
        new_ptr
    }
}
