#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use blog_os::{println, memory::translate_addr};
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

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let addresses = [
        // the identity-mapped vga buffer page
        0xb8000,
        // The code page and the stack page translate to some arbitrary physical addresses, which depend on how the bootloader created the initial mapping for our kernel.
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
        // virtual address mapped to physical address 0
        boot_info.physical_memory_offset,
    ];
    // the last 12 bits always stay the same after translation, which makes sense because these bits are the page offset and not part of the translation.

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = unsafe {
            translate_addr(virt, phys_mem_offset)
        };
        println!("{:?} -> {:?}", virt, phys);
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
