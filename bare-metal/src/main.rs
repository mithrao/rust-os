// ch1. 独立式可执行程序
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

// VGA

// 我们移除了 main 函数。原因很显然，既然没有底层运行时调用它，main 函数也失去了存在的必要性。为了重写操作系统的入口点，我们转而编写一个 _start 函数：
#[no_mangle]
// 我们使用 no_mangle 标记这个函数，来对它禁用名称重整（name mangling）——这确保 Rust 编译器输出一个名为 _start 的函数
// 我们还将函数标记为 extern "C"，告诉编译器这个函数应当使用 C 语言的调用约定，而不是 Rust 语言的调用约定。函数名为 _start ，是因为大多数系统默认使用这个名字作为入口点名称。
// 与前文的 panic 函数类似，这个函数的返回值类型为!——它定义了一个发散函数，或者说一个不允许返回的函数。这一点很重要，因为这个入口点不会被任何函数调用，但将直接被操作系统或引导程序（bootloader）调用。所以作为函数返回的替代，这个入口点应该去调用，比如操作系统提供的 exit 系统调用（“exit” system call）函数。在我们编写操作系统的情况下，关机应该是一个合适的选择，因为当一个独立式可执行程序返回时，不会留下任何需要做的事情（there is nothing to do if a freestanding binary returns）。

pub extern "C" fn _start() -> ! {
    // 因为编译器会寻找一个名为 `_start` 的函数，所以这个函数就是入口点
    // 默认命名为 `_start`
    
    // VGA
    vga_buffer::print_something();
    loop {}
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


// =============================
// ch2. 最小内核
// https://os.phil-opp.com/zh-CN/minimal-rust-kernel/

// 引导启动 (The Boot Process)
// 当我们启动电脑时，主板 ROM内存储的固件（firmware）将会运行：它将负责电脑的加电自检（power-on self test），可用内存（available RAM）的检测，以及 CPU 和其它硬件的预加载。这之后，它将寻找一个可引导的存储介质（bootable disk），并开始引导启动其中的内核（kernel）。
// x86 架构支持两种固件标准： BIOS（Basic Input/Output System）和 UEFI（Unified Extensible Firmware Interface）。其中，BIOS 标准显得陈旧而过时，但实现简单，并为 1980 年代后的所有 x86 设备所支持；相反地，UEFI 更现代化，功能也更全面，但开发和构建更复杂（至少从我的角度看是如此）。

// BIOS启动
// 几乎所有的 x86 硬件系统都支持 BIOS 启动，这也包含新型的、基于 UEFI、用模拟 BIOS（emulated BIOS）的方式向后兼容的硬件系统。这可以说是一件好事情，因为无论是上世纪还是现在的硬件系统，你都只需编写同样的引导启动逻辑；但这种兼容性有时也是 BIOS 引导启动最大的缺点，因为这意味着在系统启动前，你的 CPU 必须先进入一个 16 位系统兼容的实模式（real mode），这样 1980 年代古老的引导固件才能够继续使用。
// BIOS启动的过程：
// 当电脑启动时，主板上特殊的闪存中存储的 BIOS 固件将被加载。BIOS 固件将会加电自检、初始化硬件，然后它将寻找一个可引导的存储介质。如果找到了，那电脑的控制权将被转交给引导程序（bootloader）：一段存储在存储介质的开头的、512字节长度的程序片段。大多数的引导程序长度都大于512字节——所以通常情况下，引导程序都被切分为一段优先启动、长度不超过512字节、存储在介质开头的第一阶段引导程序（first stage bootloader），和一段随后由其加载的、长度可能较长、存储在其它位置的第二阶段引导程序（second stage bootloader）。

// Nightly Rust
// Nightly 版本的编译器允许我们在源码的开头插入特性标签（feature flag），来自由选择并使用大量实验性的功能。举个例子，要使用实验性的内联汇编（asm!宏），我们可以在 main.rs 的顶部添加 #![feature(asm)]。要注意的是，这样的实验性功能不稳定（unstable），意味着未来的 Rust 版本可能会修改或移除这些功能，而不会有预先的警告过渡。因此我们只有在绝对必要的时候，才应该使用这些特性。

// 目标配置清单 (x86_64-rust_os.json)
// 通过 --target 参数，cargo 支持不同的目标系统。这个目标系统可以使用一个目标三元组（target triple）来描述，它描述了 CPU 架构、平台供应者、操作系统和应用程序二进制接口（Application Binary Interface, ABI）。比方说，目标三元组 x86_64-unknown-linux-gnu 描述一个基于 x86_64 架构 CPU 的、没有明确的平台供应者的 linux 系统，它遵循 GNU 风格的 ABI。Rust 支持许多不同的目标三元组，包括安卓系统对应的 arm-linux-androideabi 和 WebAssembly使用的wasm32-unknown-unknown。
// 为了编写我们的目标系统，并且鉴于我们需要做一些特殊的配置（比如没有依赖的底层操作系统），已经支持的目标三元组都不能满足我们的要求。幸运的是，只需使用一个 JSON 文件，Rust 便允许我们定义自己的目标系统；这个文件常被称作目标配置清单（target specification）。
// 一个配置清单中包含多个配置项（field）。大多数的配置项都是 LLVM 需求的，它们将配置为特定平台生成的代码。打个比方，data-layout 配置项定义了不同的整数、浮点数、指针类型的长度；另外，还有一些 Rust 用作条件编译的配置项，如 target-pointer-width。还有一些类型的配置项，定义了这个包该如何被编译，例如，pre-link-args 配置项指定了应该向链接器（linker）传入的参数。
// 需要注意的是，因为我们要在裸机（bare metal）上运行内核，我们已经修改了 llvm-target 的内容，并将 os 配置项的值改为 none。
// "linker..." 在这里，我们不使用平台默认提供的链接器，因为它可能不支持 Linux 目标系统。为了链接我们的内核，我们使用跨平台的 LLD链接器（LLD linker），它是和 Rust 一起打包发布的。
// "panic-strategy": "abort" 这个配置项的意思是，我们的编译目标不支持 panic 时的栈展开（stack unwinding），所以我们选择直接在 panic 时中止（abort on panic）。这和在 Cargo.toml 文件中添加 panic = "abort" 选项的作用是相同的，所以我们可以不在这里的配置清单中填写这一项。
// "disable-redzone": true   我们正在编写一个内核，所以我们迟早要处理中断。要安全地实现这一点，我们必须禁用一个与红区（redzone）有关的栈指针优化：因为此时，这个优化可能会导致栈被破坏
// "features": "-mmx,-sse,+soft-float" features 配置项被用来启用或禁用某个目标 CPU 特征（CPU feature）。通过在它们前面添加-号，我们将 mmx 和 sse 特征禁用；添加前缀+号，我们启用了 soft-float 特征。
//              mmx 和 sse 特征决定了是否支持单指令多数据流（Single Instruction Multiple Data，SIMD）相关指令，这些指令常常能显著地提高程序层面的性能。然而，在内核中使用庞大的 SIMD 寄存器，可能会造成较大的性能影响：因为每次程序中断时，内核不得不储存整个庞大的 SIMD 寄存器以备恢复——这意味着，对每个硬件中断或系统调用，完整的 SIMD 状态必须存到主存中。由于 SIMD 状态可能相当大（512~1600 个字节），而中断可能时常发生，这些额外的存储与恢复操作可能显著地影响效率。为解决这个问题，我们对内核禁用 SIMD（但这不意味着禁用内核之上的应用程序的 SIMD 支持）。
//              禁用 SIMD 产生的一个问题是，x86_64 架构的浮点数指针运算默认依赖于 SIMD 寄存器。我们的解决方法是，启用 soft-float 特征，它将使用基于整数的软件功能，模拟浮点数指针运算。

// 编译内核
// 要编译我们的内核，我们将使用 Linux 系统的编写风格（这可能是 LLVM 的默认风格）。这意味着，我们需要把前一篇文章中编写的入口点重命名为 _start
// 注意的是，无论你开发使用的是哪类操作系统，你都需要将入口点命名为 _start。前一篇文章中编写的 Windows 系统和 macOS 对应的入口点不应该被保留。
// 通过把 JSON 文件名传入 --target 选项，我们现在可以开始编译我们的内核。
// 毫不意外的编译失败了，错误信息告诉我们编译器没有找到 core 这个crate，它包含了Rust语言中的部分基础类型，如 Result、Option、迭代器等等，并且它还会隐式链接到 no_std 特性里面。
// 通常状况下，core crate以预编译库（precompiled library）的形式与 Rust 编译器一同发布——这时，core crate只对支持的宿主系统有效，而对我们自定义的目标系统无效。如果我们想为其它系统编译代码，我们需要为这些系统重新编译整个 core crate。

// 内存相关函数
// 目前来说，Rust编译器假定所有内置函数（built-in functions）在所有系统内都是存在且可用的。事实上这个前提只对了一半， 绝大多数内置函数都可以被 compiler_builtins 提供，而这个crate刚刚已经被我们重编译过了，然而部分内存相关函数是需要操作系统相关的标准C库提供的。 比如，memset（该函数可以为一个内存块内的所有比特进行赋值）、memcpy（将一个内存块里的数据拷贝到另一个内存块）以及memcmp（比较两个内存块的数据）。 好在我们的内核暂时还不需要用到这些函数，但是不要高兴的太早，当我们编写更丰富的功能（比如拷贝数据结构）时就会用到了。
// 现在我们当然无法提供操作系统相关的标准C库，所以我们需要使用其他办法提供这些东西。一个显而易见的途径就是自己实现 memset 这些函数，但不要忘记加入 #[no_mangle] 语句，以避免编译时被自动重命名。 当然，这样做很危险，底层函数中最细微的错误也会将程序导向不可预知的未来。比如，你可能在实现 memcpy 时使用了一个 for 循环，然而 for 循环本身又会调用 IntoIterator::into_iter 这个trait方法，这个方法又会再次调用 memcpy，此时一个无限递归就产生了，所以还是使用经过良好测试的既存实现更加可靠。
// 幸运的是，compiler_builtins 事实上自带了所有相关函数的实现，只是在默认情况下，出于避免和标准C库发生冲突的考量被禁用掉了，此时我们需要将 build-std-features 配置项设置为 ["compiler-builtins-mem"] 来启用这个特性。如同 build-std 配置项一样，该特性可以使用 -Z 参数启用，也可以在 .cargo/config.toml 中使用 unstable 配置集启用。现在我们的配置文件中的相关部分是这样子的：
// in .cargo/config.toml: build-std-features = ["compiler-builtins-mem"]
//                        该参数为 compiler_builtins 启用了 mem 特性，至于具体效果，就是已经在内部通过 #[no_mangle] 向链接器提供了 memcpy 等函数的实现。

// 设置默认编译目标
// 每次调用 cargo build 命令都需要传入 --target 参数很麻烦吧？其实我们可以复写掉默认值，从而省略这个参数，只需要在 .cargo/config.toml 中加入以下 cargo 配置：
// in .cargo/config.toml: target = "x86_64-blog_os.json"

// 向屏幕打印字符 - VGA
// 要做到这一步，最简单的方式是写入 VGA 字符缓冲区（VGA text buffer）：这是一段映射到 VGA 硬件的特殊内存片段，包含着显示在屏幕上的内容。通常情况下，它能够存储 25 行、80 列共 2000 个字符单元（character cell）；每个字符单元能够显示一个 ASCII 字符，也能设置这个字符的前景色（foreground color）和背景色（background color）。输出到屏幕的字符大概长这样：
// 我们将在下篇文章中详细讨论 VGA 字符缓冲区的内存布局；目前我们只需要知道，这段缓冲区的地址是 0xb8000，且每个字符单元包含一个 ASCII 码字节和一个颜色字节。

// 启动内核
// 既然我们已经有了一个能够打印字符的可执行程序，是时候把它运行起来试试看了。首先，我们将编译完毕的内核与引导程序链接，来创建一个引导映像；这之后，我们可以在 QEMU 虚拟机中运行它，或者通过 U 盘在真机上运行。
// `qemu-system-x86_64 -drive format=raw,file=target/x86_64-rust_os/debug/bootimage-rust-os.bin`


// ===================================
// ch3. VGA Text Mode
// https://os.phil-opp.com/zh-CN/vga-text-mode/#vga-zi-fu-huan-chong-qu

// VGA 字符缓冲区
// 要修改 VGA 字符缓冲区，我们可以通过存储器映射输入输出（memory-mapped I/O）的方式，读取或写入地址 0xb8000；这意味着，我们可以像操作普通的内存区域一样操作这个地址。
// 需要注意的是，一些硬件虽然映射到存储器，但可能不会完全支持所有的内存操作：可能会有一些设备支持按 u8 字节读取，但在读取 u64 时返回无效的数据。幸运的是，字符缓冲区都支持标准的读写操作，所以我们不需要用特殊的标准对待它。

// 包装到 Rust 模块
// 我们的模块暂时不需要添加子模块，所以我们将它创建为 src/vga_buffer.rs 文件。
mod vga_buffer;

// 易失操作
// 我们刚才看到，自己想要输出的信息被正确地打印到屏幕上。然而，未来 Rust 编译器更暴力的优化可能让这段代码不按预期工作。
// 产生问题的原因在于，我们只向 Buffer 写入，却不再从它读出数据。此时，编译器不知道我们事实上已经在操作 VGA 缓冲区内存，而不是在操作普通的 RAM——因此也不知道产生的副效应（side effect），即会有几个字符显示在屏幕上。这时，编译器也许会认为这些写入操作都没有必要，甚至会选择忽略这些操作！所以，为了避免这些并不正确的优化，这些写入操作应当被指定为易失操作。这将告诉编译器，这些写入可能会产生副效应，不应该被优化掉。
// 为了在我们的 VGA 缓冲区中使用易失的写入操作，我们使用 volatile 库。这个包（crate）提供一个名为 Volatile 的包装类型（wrapping type）和它的 read、write 方法；这些方法包装了 core::ptr 内的 read_volatile 和 write_volatile 函数，从而保证读操作或写操作不会被编译器优化。
