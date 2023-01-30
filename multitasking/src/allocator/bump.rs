use alloc::alloc::{GlobalAlloc, Layout};
use super::{align_up, Locked};
use core::ptr;


pub struct BumpAllocator {
    heap_start:  usize,
    heap_end:    usize,
    next:        usize,
    allocations: usize,
}

impl BumpAllocator {
    /// create a new empty bump allocator.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start:  0,
            heap_end:    0,
            next:        0,
            allocations: 0,
        }
    }

    /// Initializes the bump allocator with the given heap bounds.
    ///
    /// This method is unsafe because the caller must ensure that the given
    /// memory range is unused. Also, this method must be called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end   = heap_start + heap_size;
        // The purpose of the next field is to always point to the first unused byte of the heap, i.e., the start address of the next allocation.
        self.next       = heap_start;
    }
}

/// All heap allocators need to implement the GlobalAlloc trait
/// 
/// the alloc and dealloc methods of the GlobalAlloc trait only operate on an immutable &self reference, so updating the next and allocations fields is not possible.
/// This type provides a lock method that performs mutual exclusion and thus safely turns a &self reference to a &mut self reference. We’ve already used the wrapper type multiple times in our kernel, for example for the VGA text buffer.
unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // get a mutable reference
        
        // to round up the next address to the alignment specified by the Layout argument. 
        let alloc_start = align_up(bump.next, layout.align());

        // To prevent integer overflow on large allocations, we use the checked_add method.
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };
        
        if alloc_end > bump.heap_end { 
            ptr::null_mut()  // out of memory
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // get a mutable reference
        let mut bump = self.lock();

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}