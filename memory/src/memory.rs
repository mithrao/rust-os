/// implement page table
/// First, we will take a look at the currently active page tables that our kernel runs on.
/// In the second step, we will create a translation function that returns the physical address that a given virtual address is mapped to
/// As a last step, we will try to modify the page tables in order to create a new mapping.

use x86_64::{
    structures::paging::PageTable,
    VirtAddr,
};

use x86_64::{
    PhysAddr,
    structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator}
};

// Translating virtual to physical addresses is a common task in an OS kernel, therefore the x86_64 crate provides an abstraction for it. The implementation already supports huge pages and several other page table functions apart from translate_addr, so we will use it in the following instead of adding huge page support to our own implementation.
// The OffsetPageTable type assumes that the complete physical memory is mapped to the virtual address space at some offset. 
use x86_64::structures::paging::OffsetPageTable;

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) 
    -> OffsetPageTable<'static> 
{
    let level_4_table = active_level_4_table(physical_memory_offset);
    // returns a new OffsetPageTable instance with a 'static lifetime.
    // This means that the instance stays valid for the complete runtime of our kernel.
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}


/// Returns a mutable reference to the active level 4 table.
///
/// The active_level_4_table function should only be called from the init function from now on because it can easily lead to aliased mutable references when called multiple times, which can cause undefined behavior. For this reason, we make the function private by removing the pub specifier.
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    // 1. read the physical frame of the active level 4 table from the CR3 register.
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    // 2. take its physical start address, convert it to a u64, and add it to physical_memory_offset to get the virtual address where the page table frame is mapped
    let virt = physical_memory_offset + phys.as_u64();
    // 3. convert the virtual address to a *mut PageTable raw pointer through the as_mut_ptr method and then unsafely create a &mut PageTable reference from it.
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

/// Creates a new mapping
/// We will use the map_to function of the Mapper trait for our implementation, so let’s take a look at that function first. The documentation tells us that it takes four arguments: the page that we want to map, the frame that the page should be mapped to, a set of flags for the page table entry, and a frame_allocator. The frame allocator is needed because mapping the given page might require creating additional page tables, which need unused frames as backing storage.
/// 
/// Creates an example mapping for the given page to frame `0xb8000`.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    // The map_to method is unsafe because the caller must ensure that the frame is not already in use. The reason for this is that mapping the same frame twice could result in undefined behavior, for example when two different &mut references point to the same physical memory location. In our case, we reuse the VGA text buffer frame, which is already mapped, so we break the required condition. However, the create_example_mapping function is only a temporary testing function and will be removed after this post, so it is ok. To remind us of the unsafety, we put a FIXME comment on the line.
    let map_to_result = unsafe {
        // use the map_to function of the Mapper trait to create a new mapping
        // it takes four arguments: the page that we want to map, the frame that the page should be mapped to, a set of flags for the page table entry, and a frame_allocator.
        // The frame allocator is needed because mapping the given page might require creating additional page tables, which need unused frames as backing storage.
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to failed").flush();
}


/// To be able to call create_example_mapping, we need to create a type that implements the FrameAllocator trait first. As noted above, the trait is responsible for allocating frames for new page tables if they are needed by map_to.
/// A FrameAllocator that always returns `None`.
pub struct EmptyFrameAllocator;

// Implementing the FrameAllocator is unsafe because the implementer must guarantee that the allocator yields only unused frames. 
unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

// Choosing a Virtual Page
// 
// The graphic shows two candidate pages in the virtual address space, both marked in yellow. One page is at address 0x803fdfd000, which is 3 pages before the mapped page (in blue). While the level 4 and level 3 page table indices are the same as for the blue page, the level 2 and level 1 indices are different (see the previous post). The different index into the level 2 table means that a different level 1 table is used for this page. Since this level 1 table does not exist yet, we would need to create it if we chose that page for our example mapping, which would require an additional unused physical frame. In contrast, the second candidate page at address 0x803fe02000 does not have this problem because it uses the same level 1 page table as the blue page. Thus, all the required page tables already exist.
// the difficulty of creating a new mapping depends on the virtual page that we want to map. In the easiest case, the level 1 page table for the page already exists and we just need to write a single entry. In the most difficult case, the page is in a memory region for which no level 3 exists yet, so we need to create new level 3, level 2 and level 1 page tables first.
// For calling our create_example_mapping function with the EmptyFrameAllocator, we need to choose a page for which all page tables already exist. To find such a page, we can utilize the fact that the bootloader loads itself in the first megabyte of the virtual address space. This means that a valid level 1 table exists for all pages in this region. Thus, we can choose any unused page in this memory region for our example mapping, such as the page at address 0. Normally, this page should stay unused to guarantee that dereferencing a null pointer causes a page fault, so we know that the bootloader leaves it unmapped.






