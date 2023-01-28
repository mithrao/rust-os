/// implement page table
/// First, we will take a look at the currently active page tables that our kernel runs on.
/// In the second step, we will create a translation function that returns the physical address that a given virtual address is mapped to
/// As a last step, we will try to modify the page tables in order to create a new mapping.

use x86_64::{
    structures::paging::{PageTable},
    VirtAddr,
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

// We can now use the Translate::translate_addr method instead of our own memory::translate_addr function. We only need to change a few lines in our kernel_main