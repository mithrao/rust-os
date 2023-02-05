use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{println, print, gdt};
// static mut is prone to data races
use lazy_static::lazy_static;
// intel 8259 programmable interrupt controller (PIC)
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::PageFaultErrorCode;
use crate::hlt_loop;

// 将PIC的中断编号范围设定为了32–47
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// 我们使用 Mutex 容器包裹了 ChainedPics，这样就可以通过（lock 函数）拿到被定义为安全的变量修改权限
pub static PICS: spin::Mutex<ChainedPics> = 
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
// repr(u8) 开关使枚举值对应的数值以 u8 格式进行存储，这样未来我们可以在这里加入更多的中断枚举。
#[repr(u8)]
pub enum InterruptIndex {
    // 我们已经知道 计时器组件 使用了主PIC的0号管脚，根据上文中我们定义的序号偏移量32，所以计时器对应的中断序号也是32。但是不要将32硬编码进去，我们将其存储到枚举类型 InterruptIndex 中:
    Timer = PIC_1_OFFSET,
    // 键盘使用的是主PIC的1号管脚，在CPU的中断编号为33（1 + 偏移量32）。我们需要在 InterruptIndex 枚举类型里添加一个 Keyboard，但是无需显式指定对应值，因为在默认情况下，它的对应值是上一个枚举对应值加一也就是33。
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8 
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

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

        // InterruptDescriptorTable 结构实现了 IndexMut trait，所以我们可以通过序号来单独修改某一个条目。
        idt[InterruptIndex::Timer.as_usize()]
            .set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(keyboard_interrupt_handler);
        
        // register the page fault handler
        idt.page_fault.set_handler_fn(page_fault_handler);

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
// breakpoint interrupt handler
extern "x86-interrupt" fn  breakpoint_handler(
    stack_frame: InterruptStackFrame) 
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

// double fault exception handler
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

// timer interrupt handler
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!(".");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

/// keyboard interrupt handler
/// 
/// Currently, we handle the keyboard input directly in the interrupt handler. This is not a good idea for the long term because interrupt handlers should stay as short as possible as they might interrupt important work.
/// Instead, interrupt handlers should only perform the minimal amount of work necessary (e.g., reading the keyboard scancode) and leave the rest of the work (e.g., interpreting the scancode) to a background task.
extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe {
        port.read()
    };
    // removed all the keyboard handling code from this functio
    // call `add_scancode` (in task/keyboard.rs) on keyboard interrupts
    crate::task::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

// page fault handler
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode)
{
    // CR2 寄存器会在 page fault 发生时，被CPU自动写入导致异常的虚拟地址
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

/// create a test_breakpoint_exception test
#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}