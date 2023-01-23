// 通常来说，编译器会对每个未使用的变量发出警告（warning）；使用 #[allow(dead_code)]，我们可以对 Color 枚举类型禁用这个警告
#[allow(dead_code)]
// 我们还生成（derive）了 Copy、Clone、Debug、PartialEq 和 Eq 这几个 trait：这让我们的类型遵循复制语义（copy semantics），也让它可以被比较、被调试和打印。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]

// 首先，我们使用 Rust 的枚举（enum）表示特定的颜色：
// 我们使用类似于 C 语言的枚举（C-like enum），为每个颜色明确指定一个数字。在这里，每个用 repr(u8) 注记标注的枚举类型，都会以一个 u8 的形式存储——事实上 4 个二进制位就足够了，但 Rust 语言并不提供 u4 类型。
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

// 为了描述包含前景色和背景色的、完整的颜色代码（color code），我们基于 u8 创建一个新类型：
// 这里，ColorCode 类型包装了一个完整的颜色代码字节，它包含前景色和背景色信息。和 Color 类型类似，我们为它生成 Copy 和 Debug 等一系列 trait。为了确保 ColorCode 和 u8 有完全相同的内存布局，我们添加 repr(transparent) 标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

// 字符缓冲区
// 现在，我们可以添加更多的结构体，来描述屏幕上的字符和整个字符缓冲区：
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 在内存布局层面，Rust 并不保证按顺序布局成员变量。因此，我们需要使用 #[repr(C)] 标记结构体；这将按 C 语言约定的顺序布局它的成员变量，让我们能正确地映射内存片段。对 Buffer 类型，我们再次使用 repr(transparent)，来确保类型和它的单个成员有相同的内存布局。
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

// 现在，我们使用它来完成 VGA 缓冲区的 volatile 写入操作。我们将 Buffer 类型的定义修改为下列代码：
use volatile::Volatile;

struct Buffer {
    // 在这里，我们不使用 ScreenChar ，而选择使用 Volatile<ScreenChar> ——在这里，Volatile 类型是一个泛型（generic），可以包装几乎所有的类型——这确保了我们不会通过普通的写入操作，意外地向它写入数据；我们转而使用提供的 write 方法。
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

// 为了输出字符到屏幕，我们来创建一个 Writer 类型：
// 我们将让这个 Writer 类型将字符写入屏幕的最后一行，并在一行写满或接收到换行符 \n 的时候，将所有的字符向上位移一行。column_position 变量将跟踪光标在最后一行的位置。当前字符的前景和背景色将由 color_code 变量指定；另外，我们存入一个 VGA 字符缓冲区的可变借用到buffer变量中。需要注意的是，这里我们对借用使用显式生命周期（explicit lifetime），告诉编译器这个借用在何时有效：我们使用** 'static 生命周期 **（’static lifetime），意味着这个借用应该在整个程序的运行期间有效；这对一个全局有效的 VGA 字符缓冲区来说，是非常合理的。
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

// 打印字符
// 现在我们可以使用 Writer 类型来更改缓冲区内的字符了。首先，为了写入一个 ASCII 码字节，我们创建这样的函数：
impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;

                // Volatile
                // 正如代码所示，我们不再使用普通的 = 赋值，而使用了 write 方法：这能确保编译器不再优化这个写入操作。
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code: color_code,
                });

                self.column_position += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // 可以是能打印的 ASCII 码字节，也可以是换行符
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // 不包含在上述范围之内的字节
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn new_line(&mut self) {}
}

// 格式化宏
// 支持 Rust 提供的格式化宏（formatting macros）也是一个很好的思路。通过这种途径，我们可以轻松地打印不同类型的变量，如整数或浮点数。为了支持它们，我们需要实现 core::fmt::Write trait；要实现它，唯一需要提供的方法是 write_str，它和我们先前编写的 write_string 方法差别不大，只是返回值类型变成了 fmt::Result：
use core::fmt;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        // 这里，Ok(()) 属于 Result 枚举类型中的 Ok，包含一个值为 () 的变量。
        Ok(())
    }
}
//现在我们就可以使用 Rust 内置的格式化宏 write! 和 writeln! 了：


// VGA 字符缓冲区只支持 ASCII 码字节和代码页 437（Code page 437）定义的字节。Rust 语言的字符串默认编码为 UTF-8，也因此可能包含一些 VGA 字符缓冲区不支持的字节：我们使用 match 语句，来区别可打印的 ASCII 码或换行字节，和其它不可打印的字节。对每个不可打印的字节，我们打印一个 ■ 符号；这个符号在 VGA 硬件中被编码为十六进制的 0xfe。
// 我们可以亲自试一试已经编写的代码。为了这样做，我们可以临时编写一个函数：
pub fn print_something() {
    use core::fmt::Write;
    
    // 这个函数首先创建一个指向 0xb8000 地址VGA缓冲区的 Writer。实现这一点，我们需要编写的代码可能看起来有点奇怪：首先，我们把整数 0xb8000 强制转换为一个可变的裸指针（raw pointer）；之后，通过运算符*，我们将这个裸指针解引用；最后，我们再通过 &mut，再次获得它的可变借用。这些转换需要 unsafe 语句块（unsafe block），因为编译器并不能保证这个裸指针是有效的。
    // 然后它将字节 b'H' 写入缓冲区内. 前缀 b 创建了一个字节常量（byte literal），表示单个 ASCII 码字符；通过尝试写入 "ello " 和 "Wörld!"，我们可以测试 write_string 方法和其后对无法打印字符的处理逻辑
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };

    writer.write_byte(b'H');
    writer.write_string("ello ");
    // 现在我们就可以使用 Rust 内置的格式化宏 write! 和 writeln! 了：
    // writer.write_string("Wörld!");
    write!(writer, "The numbers are {} and {}", 42, 1.0/3.0).unwrap();
}

