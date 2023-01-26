#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use blog_os::println;
use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    // initialize os (now only IDT) 
    blog_os::init();

    // 0x204f76: a code page (read-only)
    let ptr = 0x204f76 as *mut u32;

    // read from the code page
    unsafe { let x = *ptr; }
    println!("read worked!");

    // write to the code page
    // unsafe { *ptr = 42; }
    // println!("write worked!");
    // by running `cargo run`, we see that the read access works, but the write access causes a page fault

    use x86_64::registers::control::Cr3;

    // Accessing the Page Tables
    // x86_64 crate 中的 Cr3::read 函数可以返回 CR3 寄存器中的当前使用的4级页表，它返回的是 PhysFrame 和 Cr3Flags 两个类型组成的元组结构。不过此时我们只关心页帧信息，所以第二个元素暂且不管。
    let (level_4_page_table, _) = Cr3::read();
    println!("Level 4 page table at: {:?}", level_4_page_table.start_address());
    // 当前的4级页表存储在 物理地址 0x1000 处，而且地址的外层数据结构是 PhysAddr

    #[cfg(test)]
    test_main();

    println!("It didn't crash");

    // save energy
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
