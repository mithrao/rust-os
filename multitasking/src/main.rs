#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

// Note that we need to specify the extern crate alloc statement in our main.rs too. This is required because the lib.rs and main.rs parts are treated as separate crates. However, we don’t need to create another #[global_allocator] static because the global allocator applies to all crates in the project. In fact, specifying an additional allocator in another crate would be an error.
extern crate alloc;

use blog_os::{println, allocator};
use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};
use blog_os::task::{executor::Executor, Task, keyboard};

// Since our _start function is called externally from the bootloader, no checking of our function signature occurs. This means that we could let it take arbitrary arguments without any compilation errors, but it would fail or cause undefined behavior at runtime.
// To make sure that the entry point function always has the correct signature that the bootloader expects, the bootloader crate provides an entry_point macro that provides a type-checked way to define a Rust function as the entry point. Let’s rewrite our entry point function to use this macro:
entry_point!(kernel_main);

// We no longer need to use extern "C" or no_mangle for our entry point, as the macro defines the real lower level _start entry point for us. The kernel_main function is now a completely normal Rust function, so we can choose an arbitrary name for it. The important thing is that it is type-checked so that a compilation error occurs when we use a wrong function signature, for example by adding an argument or changing the argument type.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // We can now use [active_level_4_table] (/src/memory.rs) to print the entries of the level 4 table:
    use blog_os::memory;
    use x86_64::VirtAddr;

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

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    // 1. a new instance of our Executor type is created
    let mut executor = Executor::new();
    // 2. call the asynchronous example_task function, which returns a future
    //    this future in the Task type, which moves it to the heap and pins it, and then add the task to the task_queue of the executor through the spawn method.
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses()));
    // 3. The run method will never return
    executor.run();
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

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    // To run the future returned by example_task, we need to call poll on it until it signals its completion by returning Poll::Ready.
    let number = async_number().await;
    println!("async number: {}", number);
}

