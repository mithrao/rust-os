#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

// enable x86-interrupts
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

pub mod serial;
pub mod vga_buffer;
pub mod interrupts;
// create a new TSS that contains a separate double fault stack in its interrupt stack table.
pub mod gdt;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

/// Entry point for `cargo xtest`
/// this _start function is used when running `cargo test --lib`
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // add a test for exception
    init();
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    // 我们使用 initialize 函数进行 8259 PIC 的初始化。正如 ChainedPics::new ，这个函数也是 unsafe 的，因为里面的不安全逻辑可能会导致PIC配置失败，进而出现一些未定义行为。
    unsafe { interrupts::PICS.lock().initialize() };
    // 启用中断
    x86_64::instructions::interrupts::enable();
    // x86_64 crate 中的 interrupts::enable 会执行特殊的 sti (“set interrupts”) 指令来启用外部中断。当我们试着执行 cargo run 后，double fault 异常几乎是立刻就被抛出了
    // 其原因就是硬件计时器（准确的说，是Intel 8253）默认是被启用的，所以在启用中断控制器之后，CPU开始接收到计时器中断信号，而我们又并未设定相对应的处理函数，所以就抛出了 double fault 异常。
}

// 让CPU在下一个中断触发之前休息一下，也就是进入休眠状态来节省一点点能源。[hlt instruction][hlt 指令] 可以让我们做到这一点
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}