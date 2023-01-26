use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor};
use x86_64::structures::gdt::SegmentSelector;


// 我们将IST的0号位定义为 double fault 的专属栈（其他IST序号也可以如此施为）
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            
            let stack_start = VirtAddr::from_ptr(unsafe {
                &STACK
            });
            let stack_end   = stack_start + STACK_SIZE;
            // 将栈的高地址指针写入0号位，之所以这样做，那是因为 x86 的栈内存分配是从高地址到低地址的
            stack_end
        };
        tss
    };
    // 我们已经创建了一个TSS，现在的问题就是怎么让CPU使用它。不幸的是这事有点繁琐，因为TSS用到了分段系统（历史原因）。但我们可以不直接加载，而是在全局描述符表（GDT）中添加一个段描述符，然后我们就可以通过ltr 指令加上GDT序号加载我们的TSS。（这也是为什么我们将模块取名为 gdt。）
}

// GDT
lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector  = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, tss_selector })
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector:  SegmentSelector,
}


// create a new GDT with a code segment and a TSS segment
// loading the GDT
pub fn init() {
    use x86_64::instructions::tables::load_tss;
    use x86_64::instructions::segmentation::{CS, Segment};

    GDT.0.load();
    unsafe {
        // 我们通过 set_reg 覆写了代码段寄存器(cs)，然后使用 load_tss 来重载了TSS
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}