use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{println, print, gdt};
// static mut is prone to data races
use lazy_static::lazy_static;
// intel 8259 programmable interrupt controller (PIC)
use pic8259::ChainedPics;
use spin;

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

// keyboard interrupt handler
extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    // 首先我们使用 lazy_static 宏创建一个受到Mutex同步锁保护的 Keyboard 对象，初始化参数为美式键盘布局以及Set-1。至于 HandleControl，它可以设定为将 ctrl+[a-z] 映射为Unicode字符 U+0001 至 U+001A，但我们不想这样，所以使用了 Ignore 选项让 ctrl 仅仅表现为一个正常键位。
    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1,
                HandleControl::Ignore)
            );
    }

    // 对于每一个中断，我们都会为 KEYBOARD 加锁，从键盘控制器获取扫描码并将其传入 add_byte 函数，并将其转化为 Option<KeyEvent> 结构。KeyEvent 包括了触发本次中断的按键信息，以及子动作是按下还是释放。
    let mut keyboard = KEYBOARD.lock();
    // Reading the Scancodes from the data port of the PS/2 controller, which is the I/O port with the number 0x60:
    let mut port = Port::new(0x60);
    // We use the Port type of the x86_64 crate to read a byte from the keyboard’s data port. This byte is called the scancode and it represents the key press/release.
    let scancode: u8 = unsafe {
        port.read()
    };

    // Interpreting the Scancodes
    // 要处理KeyEvent，我们还需要将其传入 process_keyevent 函数，将其转换为人类可读的字符，若果有必要，也会对字符进行一些处理。典型例子就是，要判断 A 键按下后输入的是小写 a 还是大写 A，这要取决于shift键是否同时被按下。
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

/// create a test_breakpoint_exception test
#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}