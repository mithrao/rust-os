use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{println, gdt};
// static mut is prone to data races
use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        // double fault (idx: 8) handler
        unsafe {
            // 在IDT中为 double fault 对应的处理函数设置栈序号
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}


pub fn init_idt() {
    // ---loading the IDT---
    // the load method expects a &'static self, that is, a reference valid for the complete runtime of the program. The reason is that the CPU will access this table on every interrupt until we load a different IDT. So using a shorter lifetime than 'static could lead to use-after-free bugs.
    // Our idt is created on the stack, so it is only valid inside the init function. Afterwards, the stack memory is reused for other functions, so the CPU would interpret random stack memory as IDT. 
    // Luckily, the InterruptDescriptorTable::load method encodes this lifetime requirement in its function definition, so that the Rust compiler is able to prevent this possible bug at compile time.
    // In order to fix this problem, we need to store our idt at a place where it has a 'static lifetime. To achieve this, we could allocate our IDT on the heap using Box and then convert it to a 'static reference, but we are writing an OS kernel and thus don’t have a heap (yet). 
    IDT.load();
}

// x86-interrupt calling convention is still unstable
// To use it anyway, we have to explicitly enable it by adding #![feature(abi_x86_interrupt)] at the top of our lib.rs
extern "x86-interrupt" fn  breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

/// create a test_breakpoint_exception test
#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}