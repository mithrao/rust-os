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
    unsafe { *ptr = 42; }
    println!("write worked!");

    // by running `cargo run`, we see that the read access works, but the write access causes a page fault

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
