// https://os.phil-opp.com/zh-CN/freestanding-rust-binary/

#![no_std]
// 要告诉 Rust 编译器我们不使用预定义的入口点，我们可以添加 #![no_main] 属性。
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
// 类型为 PanicInfo 的参数包含了 panic 发生的文件名、代码行数和可选的错误信息。这个函数从不返回，所以他被标记为发散函数（diverging function）
// 发散函数的返回类型称作 Never 类型（“never” type），记为!
fn panic(_info: &PanicInfo) -> ! {
    loop {
        
    }
}

// start语言项 - 程序entry point
// 我们通常会认为，当运行一个程序时，首先被调用的是 main 函数。但是，大多数语言都拥有一个运行时系统（runtime system），它通常为垃圾回收
// （garbage collection）或绿色线程（software threads，或 green threads）服务，这个运行时系统需要在 main 函数前启动，因为它需要让程序初始化。

// 在一个典型的使用标准库的 Rust 程序中，程序运行是从一个名为 crt0 的运行时库开始的。crt0 意为 C runtime zero，它能建立一个适合运行 C 语言程序的环境，这包含了栈的创建和可执行程序参数的传入。在这之后，这个运行时库会调用 Rust 的运行时入口点，这个入口点被称作 start语言项（“start” language item）。Rust 只拥有一个极小的运行时，它被设计为拥有较少的功能，如爆栈检测和打印栈轨迹（stack trace）。这之后，这个运行时将会调用 main 函数。
// 我们的独立式可执行程序并不能访问 Rust 运行时或 crt0 库，所以我们需要定义自己的入口点。只实现一个 start 语言项并不能帮助我们，因为这之后程序依然要求 crt0 库。所以，我们要做的是，直接重写整个 crt0 库和它定义的入口点。

// 我们移除了 main 函数。原因很显然，既然没有底层运行时调用它，main 函数也失去了存在的必要性。为了重写操作系统的入口点，我们转而编写一个 _start 函数：
#[no_mangle]
// 我们使用 no_mangle 标记这个函数，来对它禁用名称重整（name mangling）——这确保 Rust 编译器输出一个名为 _start 的函数
// 我们还将函数标记为 extern "C"，告诉编译器这个函数应当使用 C 语言的调用约定，而不是 Rust 语言的调用约定。函数名为 _start ，是因为大多数系统默认使用这个名字作为入口点名称。
// 与前文的 panic 函数类似，这个函数的返回值类型为!——它定义了一个发散函数，或者说一个不允许返回的函数。这一点很重要，因为这个入口点不会被任何函数调用，但将直接被操作系统或引导程序（bootloader）调用。所以作为函数返回的替代，这个入口点应该去调用，比如操作系统提供的 exit 系统调用（“exit” system call）函数。在我们编写操作系统的情况下，关机应该是一个合适的选择，因为当一个独立式可执行程序返回时，不会留下任何需要做的事情（there is nothing to do if a freestanding binary returns）。
pub extern "C" fn _start() -> ! {
    loop {
        
    }
}

// linker error
// 链接器（linker）是一个程序，它将生成的目标文件组合为一个可执行文件。不同的操作系统如 Windows、macOS、Linux，规定了不同的可执行文件格式，因此也各有自己的链接器，抛出不同的错误；但这些错误的根本原因还是相同的：链接器的默认配置假定程序依赖于C语言的运行时环境，但我们的程序并不依赖于它。
// 为了解决这个错误，我们需要告诉链接器，它不应该包含（include）C 语言运行环境。我们可以选择提供特定的链接器参数（linker argument），也可以选择编译为裸机目标（bare metal target）。

// 编译为裸机目标
// 在默认情况下，Rust 尝试适配当前的系统环境，编译可执行程序。
// 为了描述不同的环境，Rust 使用一个称为目标三元组（target triple）的字符串。要查看当前系统的目标三元组，我们可以运行 rustc --version --verbose

// Rust 编译器尝试为当前系统的三元组编译，并假定底层有一个类似于 Windows 或 Linux 的操作系统提供C语言运行环境——然而这将导致链接器错误。所以，为了避免这个错误，我们可以另选一个底层没有操作系统的运行环境。
// 这样的运行环境被称作裸机环境，例如目标三元组 thumbv7em-none-eabihf 描述了一个 ARM 嵌入式系统（embedded system）。我们暂时不需要了解它的细节，只需要知道这个环境底层没有操作系统——这是由三元组中的 none 描述的。要为这个目标编译，我们需要使用 rustup 添加它：
// rustup target add thumbv7em-none-eabihf

// 这行命令将为目标下载一个标准库和 core 库。这之后，我们就能为这个目标构建独立式可执行程序了：
// cargo build --target thumbv7em-none-eabihf
// 我们传递了 --target 参数，来为裸机目标系统交叉编译（cross compile）我们的程序。我们的目标并不包括操作系统，所以链接器不会试着链接 C 语言运行环境，因此构建过程成功会完成，不会产生链接器错误。

