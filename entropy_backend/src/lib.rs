//! The `getrandom` backend for nife. `getrandom` picks its source by `target_os` and ends in
//! `compile_error!` with no `nife` arm, so most of crates.io failed to build here
//! (notes/crates-io-on-nife.md). This is upstream's documented `custom` backend: one
//! `extern "Rust"` function per `getrandom` shape, each a `SystemRng` draw.
//!
//! ```text
//! uuid -> getrandom -> __getrandom_v03_custom (here) -> std::random::SystemRng
//!      -> the entropy service (one endpoint, naming no device) -> virtio-rng
//! ```

//! # How to use it
//!
//! Depend on `entropy_backend`, set `rustflags = ["--cfg", "getrandom_backend=\"custom\""]` in the
//! workspace's `.cargo/config.toml` (once per workspace: which source exists is a property of the
//! target), and write `use entropy_backend as _;` in the binary. An rlib nothing references is not
//! linked, so without that line the build stops at `undefined symbol: __getrandom_v03_custom`. The
//! crate is its own workspace at the tree root, like `std_exerciser`, so it builds only for a
//! `*-unknown-nife` target with build-std and its `--cfg` never reaches the workspace crates.
//! notes/cryptography-provider/the-getrandom-gap.md has why it is a crate and not a patch.

//! # BUGS
//!
//! - `getrandom` 0.2 needs its own hook: it returns a bare `u32` and needs 0.2's `custom` feature,
//!   which only a manifest can enable, so this crate depends on 0.2 too.
//! - 0.3 and 0.4 both declare `__getrandom_v03_custom`, so one definition serves both; their
//!   `Error` types are layout-identical, which neither crate promises.
//! - A draw panics when the process holds no entropy capability, as `SystemRng` does; there is no
//!   degraded mode, so a program using `rand` must be granted entropy.
//! - Nothing in the tree's gate builds this crate; the probe harness in notes/crates-io-on-nife.md
//!   and its consumers exercise it.

#![feature(random)]

use core::slice;
use std::random::{Rng, SystemRng};

/// `getrandom`'s custom-backend hook, filling `len` bytes at `dest` from [`SystemRng`].
///
/// # Safety
///
/// `getrandom`'s contract: `dest` is valid for writes of `len` bytes, and on `Ok(())` every one of
/// them must be initialized. `fill_bytes` writes the whole slice or panics, so there is no partial
/// path that could return `Ok` over uninitialized memory.
///
/// [`SystemRng`]: std::random::SystemRng
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom::Error> {
    // SAFETY: the caller's contract, restated above. `getrandom` never passes a null pointer with a
    // non-zero length, and a zero length yields an empty slice, which `from_raw_parts_mut` allows
    // for any aligned non-null pointer; `getrandom` passes the address of a real buffer either way.
    let buf = unsafe { slice::from_raw_parts_mut(dest, len) };
    SystemRng.fill_bytes(buf);
    Ok(())
}

/// `getrandom` **0.2**'s custom-backend hook: a different symbol and signature, the same body.
///
/// What `getrandom::register_custom_getrandom!` expands to, written out so it needs none of 0.2's
/// re-exports in scope. Upstream's signature, `fn(*mut u8, usize) -> u32` with the Rust ABI,
/// returns zero for success; `fill_bytes` writes the whole slice or panics, so it never fails.
///
/// # Safety
///
/// `dest` is valid for writes of `len` bytes, and 0.2 has already zero-filled it
/// (`uninit_slice_fill_zero` in its `custom.rs`), so nothing here observes uninitialized memory.
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_custom(dest: *mut u8, len: usize) -> u32 {
    // SAFETY: the caller's contract, restated above.
    let buf = unsafe { slice::from_raw_parts_mut(dest, len) };
    SystemRng.fill_bytes(buf);
    0
}
