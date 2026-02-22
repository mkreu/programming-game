#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

use crate::log::Log;

pub mod log;
pub mod driving;

pub const SLOT1: usize = 0x100;
pub const SLOT2: usize = 0x200;
pub const SLOT3: usize = 0x300;
pub const SLOT4: usize = 0x400;
pub const SLOT5: usize = 0x500;
pub const SLOT6: usize = 0x600;

pub fn log() -> Log {
    Log::bind(SLOT1)
}

/// Simple bump allocator for single-threaded no_std environments.
/// Uses a fixed-size static buffer. Never frees memory.
struct BumpAllocator {
    pos: UnsafeCell<usize>,
}

unsafe impl Sync for BumpAllocator {}

const HEAP_SIZE: usize = 4096;
static HEAP: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pos = unsafe { &mut *self.pos.get() };
        let aligned = (*pos + layout.align() - 1) & !(layout.align() - 1);
        let new_pos = aligned + layout.size();
        if new_pos > HEAP_SIZE {
            return core::ptr::null_mut();
        }
        *pos = new_pos;
        unsafe { HEAP.as_ptr().add(aligned) as *mut u8 }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator never frees
    }
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator {
    pos: UnsafeCell::new(0),
};
