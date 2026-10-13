//! What `wasm32-unknown-unknown` lacks: a source of randomness. Harper does not need real
//! randomness (it only seeds hash maps), and a plugin has no imports to ask the host for any,
//! so a fixed seed is used.
//!
//! The `--cfg getrandom_backend="custom"` flag for `getrandom` 0.3 is set in `.cargo/config.toml`.

fn fill(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
}

fn getrandom_02(buf: &mut [u8]) -> Result<(), getrandom02::Error> {
    fill(buf);
    Ok(())
}
getrandom02::register_custom_getrandom!(getrandom_02);

/// # Safety
/// `dest` points at `len` writable bytes.
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom03::Error> {
    // SAFETY: guaranteed by the caller.
    fill(unsafe { std::slice::from_raw_parts_mut(dest, len) });
    Ok(())
}
