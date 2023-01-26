#![no_std]
#![no_main]
// 由于集成测试处于完全独立的运行环境，也记得在测试入口文件的头部再次加入 #![feature(abi_x86_interrupt)] 开关
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use blog_os::{serial_print};
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;
use blog_os::{exit_qemu, QemuExitCode};
use x86_64::structures::idt::InterruptStackFrame;

lazy_static! {
    // 注册stack overflow测试中的自定义double fault异常处理函数
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(blog_os::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_print!("[ok]");
    // 自定义的 double fault 处理函数，在被触发的时候调用 exit_qemu(QemuExitCode::Success) 函数，而非使用默认的逻辑。
    exit_qemu(QemuExitCode::Success);
    loop {}
}

pub fn init_test_idt() {
    TEST_IDT.load();
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    blog_os::gdt::init();
    init_test_idt();

    // tirgger a stack overflow
    stack_overflow();

    panic!("Execution continued after stack overflow");
}

// 为了关闭编译器针对递归的安全警告，我们也需要为这个函数加上 allow(unconditional_recursion) 开关。
#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow(); // for each recursion, the return address is pushed
    // 使用 Volatile 类型 加入了一个 volatile 读取操作，用来阻止编译器进行 尾调用优化。除却其他乱七八糟的效果，这个优化最主要的影响就是会让编辑器将最后一行是递归语句的函数转化为普通的循环。由于没有通过递归创建新的栈帧，所以栈自然也不会出问题。
    volatile::Volatile::new(0).read(); // prevent tail recursion optimizations
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}