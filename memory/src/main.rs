#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use blog_os::{println};
use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};

// Since our _start function is called externally from the bootloader, no checking of our function signature occurs. This means that we could let it take arbitrary arguments without any compilation errors, but it would fail or cause undefined behavior at runtime.
// To make sure that the entry point function always has the correct signature that the bootloader expects, the bootloader crate provides an entry_point macro that provides a type-checked way to define a Rust function as the entry point. Let’s rewrite our entry point function to use this macro:
entry_point!(kernel_main);

// We no longer need to use extern "C" or no_mangle for our entry point, as the macro defines the real lower level _start entry point for us. The kernel_main function is now a completely normal Rust function, so we can choose an arbitrary name for it. The important thing is that it is type-checked so that a compilation error occurs when we use a wrong function signature, for example by adding an argument or changing the argument type.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // We can now use [active_level_4_table] (/src/memory.rs) to print the entries of the level 4 table:
    use blog_os::memory;
    use x86_64::{structures::paging::Page, VirtAddr};

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // initialize a mapper
    let mut mapper = unsafe {
        memory::init(phys_mem_offset)
    };
    
    // create the mapping with BooInfoFrameAllocator
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    // Creating that mapping only worked because the level 1 table responsible for the page at address 0 already exists. 
    let page = Page::containing_address(VirtAddr::new(0xdeadbeaf000));
    // 1. Create the mapping for the page at address 0 by calling our create_example_mapping function with a mutable reference to the mapper and the frame_allocator instances. 
    //    This maps the page to the VGA text buffer frame, so we should see any write to it on the screen.
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    // write the string `New!` to the screen through the new mapping
    // 2. convert the page to a raw pointer and write a value to offset 400
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe {
        // offset 400: We don’t write to the start of the page because the top line of the VGA buffer is directly shifted off the screen by the next println.
        // We write the value 0x_f021_f077_f065_f04e, which represents the string “New!” on a white background. 
        page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e);
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
