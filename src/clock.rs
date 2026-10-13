//! A clock that works inside the plugin.
//!
//! `std::time::Instant::now()` panics on `wasm32-unknown-unknown`, and a plugin has no imports
//! to ask the host for the time. There, the clock stands still, so time budgets (such as the
//! compound checker's `max_check_time`) never run out; the work they limit is bounded by its
//! own structure instead.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug)]
pub struct Instant;

#[cfg(target_arch = "wasm32")]
impl Instant {
    pub fn now() -> Self {
        Self
    }

    pub fn elapsed(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}
