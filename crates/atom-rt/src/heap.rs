use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

const HEAP_SIZE: usize = 128 * 1024;

#[repr(align(16))]
struct Arena(#[allow(dead_code)] [u8; HEAP_SIZE]);

struct SyncArena(UnsafeCell<Arena>);
unsafe impl Sync for SyncArena {}

// `#[used]` keeps the arena in the binary even if LTO decides no live
// alloc sites remain — the kernel loader's memsz>filesz .bss path needs
// a real NOBITS section to exercise.
#[used]
static ARENA: SyncArena = SyncArena(UnsafeCell::new(Arena([0; HEAP_SIZE])));

pub struct BumpAllocator {
    offset: UnsafeCell<usize>,
}

unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            offset: UnsafeCell::new(0),
        }
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = ARENA.0.get() as *mut u8;
        unsafe {
            let cur = *self.offset.get();
            let aligned = (cur + layout.align() - 1) & !(layout.align() - 1);
            let next = aligned + layout.size();
            if next > HEAP_SIZE {
                return core::ptr::null_mut();
            }
            *self.offset.get() = next;
            base.add(aligned)
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_allocates_from_arena() {
        let a = BumpAllocator::new();
        let p = unsafe { a.alloc(Layout::from_size_align(64, 8).unwrap()) };
        assert!(!p.is_null());
        let q = unsafe { a.alloc(Layout::from_size_align(64, 16).unwrap()) };
        assert!(!q.is_null());
        assert_eq!((q as usize) % 16, 0);
        assert!((q as usize) >= (p as usize) + 64);
    }

    #[test]
    fn oom_returns_null() {
        let a = BumpAllocator::new();
        let p = unsafe { a.alloc(Layout::from_size_align(HEAP_SIZE + 1, 8).unwrap()) };
        assert!(p.is_null());
    }
}
