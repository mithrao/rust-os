use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

// The struct does not need any fields, so we create it as a zero-sized type. 
pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    // always return the null pointer from alloc, which corresponds to an allocation error.
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }
    
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }

    //  The alloc_zeroed and realloc methods have default implementations, so we don’t need to provide implementations for them.
}

// The #[global_allocator] attribute tells the Rust compiler which allocator instance it should use as the global heap allocator.
// The attribute is only applicable to a static that implements the GlobalAlloc trait.
#[global_allocator]
// Since the Dummy allocator is a zero-sized type, we don’t need to specify any fields in the initialization expression.
static ALLOCATOR: Dummy = Dummy;


