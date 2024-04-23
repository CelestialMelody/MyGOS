use core::fmt::{self, Write};

use spin::Mutex;

use crate::sbi::console_putchar;

struct Stdout;

#[cfg(feature = "multi-harts")]
// lazy_static! {
//     static ref CONSOLE_PRINT_LOCK: Mutex<()> = Mutex::new(());
// }
static CONSOLE_PRINT_LOCK: spin::Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            console_putchar(c as i32);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments<'_>) {
    #[cfg(feature = "multi-harts")]
    let lck = CONSOLE_PRINT_LOCK.lock();
    Stdout.write_fmt(args).unwrap();
    #[cfg(feature = "multi-harts")]
    drop(lck);
}

#[macro_export]
macro_rules! print {
    ($fmt:literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::console::print("\n");
    };
    ($fmt:literal $(, $($arg:tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    };
    ($($arg:tt)+) => {
        $crate::console::print(format_args!($($arg)+));
    };
}
