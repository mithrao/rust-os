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

use bootloader::bootinfo::MemoryMap;
use bootloader::bootinfo::MemoryRegionType;

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

/// Allocating Frames
/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    // 'static reference to the memory map passed by the bootloader
    // the memory map is provided by the BIOS/UEFI firmware. It can only be queried very early in the boot process, so the bootloader already calls the respective functions for us. 
    memory_map: &'static MemoryMap,
    // next field that keeps track of the number of the next frame that the allocator should return
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused
    /// 
    /// The init function initializes a BootInfoFrameAllocator with a given memory map. 
    /// Since we don’t know if the usable frames of the memory map were already used somewhere else, our init function must be unsafe to require additional guarantees from the caller.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            // The next field is initialized with 0 and will be increased for every frame allocation to avoid returning the same frame twice.
            next: 0,
        }
    }
}

impl BootInfoFrameAllocator {
    /// Returns an iterator over the usable frames specified in the memory map.
    /// This function uses iterator combinator methods to transform the initial MemoryMap into an iterator of usable physical frames:
    /// The return type of the function uses the impl Trait feature. This way, we can specify that we return some type that implements the Iterator trait with item type PhysFrame but don’t need to name the concrete return type. This is important here because we can’t name the concrete type since it depends on unnamable closure types.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        // 1. call the iter method to convert the memory map to an iterator of MemoryRegions.
        let regions = self.memory_map.iter();
        // 2. use the filter method to skip any reserved or otherwise unavailable regions.
        let usable_regions = regions
                .filter(|r| r.region_type == MemoryRegionType::Usable);
        // map each region to its address range
        // 3.  use the map combinator and Rust’s range syntax to transform our iterator of memory regions to an iterator of address ranges.
        let addr_ranges = usable_regions
                .map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        // 4. use flat_map to transform the address ranges into an iterator of frame start addresses, choosing every 4096th address using step_by. 
        //    Since 4096 bytes (= 4 KiB) is the page size, we get the start address of each frame. 
        //    The bootloader page-aligns all usable memory areas so that we don’t need any alignment or rounding code here. 
        //    By using flat_map instead of map, we get an Iterator<Item = u64> instead of an Iterator<Item = Iterator<Item = u64>>.
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        // 5.  convert the start addresses to PhysFrame types to construct an Iterator<Item = PhysFrame>.
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

/// Implementing the FrameAllocator Trait
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // 1. use the usable_frames method to get an iterator of usable frames from the memory map.
        let frame = self.usable_frames().nth(self.next);
        // 2. increase self.next by one so that we return the following frame on the next call.
        self.next += 1;
        frame
    }
}

