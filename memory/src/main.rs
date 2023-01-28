#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use blog_os::println;
use x86_64::structures::paging::PageTable;
use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};

// Since our _start function is called externally from the bootloader, no checking of our function signature occurs. This means that we could let it take arbitrary arguments without any compilation errors, but it would fail or cause undefined behavior at runtime.
// To make sure that the entry point function always has the correct signature that the bootloader expects, the bootloader crate provides an entry_point macro that provides a type-checked way to define a Rust function as the entry point. Let’s rewrite our entry point function to use this macro:
entry_point!(kernel_main);

// We no longer need to use extern "C" or no_mangle for our entry point, as the macro defines the real lower level _start entry point for us. The kernel_main function is now a completely normal Rust function, so we can choose an arbitrary name for it. The important thing is that it is type-checked so that a compilation error occurs when we use a wrong function signature, for example by adding an argument or changing the argument type.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // We can now use [active_level_4_table] (/src/memory.rs) to print the entries of the level 4 table:
    use blog_os::memory::active_level_4_table;
    use x86_64::VirtAddr;

    println!("Hello World{}", "!");
    blog_os::init();

    // 1. convert the physical_memory_offset of the BootInfo struct to a VirtAddr and pass it to the active_level_4_table function.
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let l4_table = unsafe {
        active_level_4_table(phys_mem_offset)
    };
    
    // 2. use the iter function to iterate over the page table entries and the enumerate combinator to additionally add an index i to each element.
    for (i, entry) in l4_table.iter().enumerate() {
        // (only print non-empty entries because all 512 entries wouldn’t fit on the screen)
        if !entry.is_unused() {
            println!("L4 Entry {}: {:?}", i, entry);

            // (L4 -> L3 -> L2 -> L1)
            // To traverse the page tables further and take a look at a level 3 table, we can take the mapped frame of an entry and convert it to a virtual address again:
            // get the physical address from the entry and convert it
            let phys = entry.frame().unwrap().start_address();
            let virt = phys.as_u64() + boot_info.physical_memory_offset;
            let ptr = VirtAddr::new(virt).as_mut_ptr();
            let l3_table: &PageTable = unsafe {
                &*ptr
            };

            // print non-empty entry of the level 3 table
            for (i, entry) in l3_table.iter().enumerate() {
                if !entry.is_unused() {
                    println!("  L3 Entry {}: {:?}", i, entry);
                }
            }
        }
    }

    #[cfg(test)]
    test_main();

    println!("It didn't crash");
    blog_os::hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    blog_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
