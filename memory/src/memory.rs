/// implement page table
/// First, we will take a look at the currently active page tables that our kernel runs on.
/// In the second step, we will create a translation function that returns the physical address that a given virtual address is mapped to
/// As a last step, we will try to modify the page tables in order to create a new mapping.

use x86_64::{
    structures::paging::{PageTable},
    VirtAddr,
    PhysAddr,
};

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
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


/// To translate a virtual to a physical address, we have to traverse the four-level page table until we reach the mapped frame.
/// Translates the given virtual address to the mapped physical address, or
/// `None` if the address is not mapped.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offse
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    // We forward the function to a safe translate_addr_inner function to limit the scope of unsafe. As we noted above, Rust treats the complete body of an unsafe fn like a large unsafe block. By calling into a private safe function, we make each unsafe operation explicit again.
    translate_addr_inner(addr, physical_memory_offset)
}

/// Private function that is called by `translate_addr`.
///
/// This function is safe to limit the scope of `unsafe` because Rust treats
/// the whole body of unsafe functions as an unsafe block. This function must
/// only be reachable through `unsafe fn` from outside of this module.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // read the active level-4 frame from the CR4 register
    let (level_4_table_frame, _) = Cr3::read();

    // The VirtAddr struct already provides methods to compute the indexes into the page tables of the four levels. We store these indexes in a small array because it allows us to traverse the page tables using a for loop. 
    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // traverse the mutli-level page table
    for &index in &table_indexes {
        // convert the frame into a page table reference
        // 1. use the physical_memory_offset to convert the frame into a page table reference
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        // the pointed next level page table
        let table = unsafe {
            &*table_ptr
        };

        // read the page table entry and update `frame`
        let entry = &table[index];
        // The frame points to page table frames while iterating and to the mapped frame after the last iteration, i.e., after following the level 1 entry.
        // 2. read the entry of the current page table and use the PageTableEntry::frame function to retrieve the mapped frame.
        frame = match entry.frame() {
            Ok(frame) => frame,
            //  If the entry is not mapped to a frame, we return None
            Err(FrameError::FrameNotPresent) => return None,
            // If the entry maps a huge 2 MiB or 1 GiB page, we panic for now.
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    // calculate the physical address by adding the page offset
    Some(frame.start_address() + u64::from(addr.page_offset()))
}
