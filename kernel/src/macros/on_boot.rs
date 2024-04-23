use core::sync::atomic::AtomicBool;

// lazy_static! {
//     pub static ref BOOTED: AtomicBool = core::sync::atomic::AtomicBool::new(false);
// }

use spin::lazy::Lazy;
pub static BOOTED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

macro_rules! synchronize_hart {
    () => {{
        while !$crate::macros::on_boot::BOOTED.load(core::sync::atomic::Ordering::Acquire) {}
    }};
}

macro_rules! toggle_booted {
    () => {{
        $crate::macros::on_boot::BOOTED.store(true, core::sync::atomic::Ordering::Release);
    }};
}
