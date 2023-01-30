use linked_list::LinkedListAllocator;
// use bump::BumpAllocator;

// bump allocator
pub mod bump;
// linked list allocator
pub mod linked_list;

/// creating a kernel heap
/// 
/// Before we can create a proper allocator, we first need to create a heap memory region from which the allocator can allocate memory.
/// To do this, we need to define a virtual memory range for the heap region and then map this region to physical frames.
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE:  usize = 100 * 1024; // 100 KiB

// The #[global_allocator] attribute tells the Rust compiler which allocator instance it should use as the global heap allocator.
// The attribute is only applicable to a static that implements the GlobalAlloc trait.
#[global_allocator]
// The struct is named LockedHeap because it uses the spinning_top::Spinlock type for synchronization. This is required because multiple threads could access the ALLOCATOR static at the same time.
// As always, when using a spinlock or a mutex, we need to be careful to not accidentally cause a deadlock. This means that we shouldn’t perform any allocations in interrupt handlers, since they can run at an arbitrary time and might interrupt an in-progress allocation.
static ALLOCATOR: Locked<LinkedListAllocator> = 
    Locked::new(LinkedListAllocator::new());

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// init_heap: maps the heap pages using the Mapper API
/// 
/// The function takes mutable references to a Mapper and a FrameAllocator instance, both limited to 4 KiB pages by using Size4KiB as the generic parameter
/// The return value of the function is a Result with the unit type () as the success variant and a MapToError as the error variant, which is the error type returned by the Mapper::map_to method.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // 1. Creating the page range
    let page_range = {
        // convert the HEAP_START pointer to a VirtAddr type.
        let heap_start = VirtAddr::new(HEAP_START as u64);
        // calculate the heap end address from it by adding the HEAP_SIZE. We want an inclusive bound (the address of the last byte of the heap), so we subtract 1. 
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        // convert the addresses into Page types using the containing_address function.
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        // create a page range from the start and end pages using the Page::range_inclusive function.
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // 2. Mapping the pages
    // map all pages of the page range we just created. For that, we iterate over these pages using a for loop.
    for page in page_range {
        // allocate a physical frame that the page should be mapped to using the FrameAllocator::allocate_frame method. 
        // This method returns None when there are no more frames left. We deal with that case by mapping it to a MapToError::FrameAllocationFailed error through the Option::ok_or method and then applying the question mark operator to return early in the case of an error.
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        // set the required PRESENT flag and the WRITABLE flag for the page. 
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            // use the Mapper::map_to method for creating the mapping in the active page table.
            // The method can fail, so we use "?" again to forward the error to the caller.
            // On success, the method returns a MapperFlush instance that we can use to update the translation lookaside buffer using the flush method.
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

/// We can't use `unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {...}` 
/// because the Rust compiler does not permit trait implementations for types defined in other crates
/// we need to create our own wrapper type around spin::Mutex
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl <A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked { inner: spin::Mutex::new(inner), }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}


/// Align the given address `addr` upwards to alignment `align`.
fn align_up(addr: usize, align: usize) -> usize {
    //  to create a bitmask to align the address in a very efficient way.
    (addr + align - 1) & !(align - 1)
}

