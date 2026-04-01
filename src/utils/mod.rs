pub mod fuzzy;
pub mod terminal;
use std::sync::atomic::{AtomicBool, Ordering};

pub static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn is_debug_enabled() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

pub fn enable_debug() {
    DEBUG.store(true, Ordering::Relaxed);
}
