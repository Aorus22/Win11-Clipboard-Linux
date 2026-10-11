//! Test-only serialization for tests that touch process-global state
//! (env vars, real config files). Rust runs #[test] fns on threads of one
//! process — without this, env flips in one test flake path resolution in another.

pub static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|poison| poison.into_inner())
}
