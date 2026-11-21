//! The same linker contract as `std_exerciser`: the shared `link.ld` that `user_mode_runtime` delivers, as is.
//!
//! **This used to relink at 16 MiB**, by substituting the shared script's base, because every program
//! was linked at `0x40_0000` with its stack at `0x50_0000` and a `rustls` provider with its RustCrypto
//! primitives did not fit in the 896 KiB between them. Milestone 206 (a program image has under 896 KiB) drew the address-space map
//! (`nifeos/address_space_layout`, DECISIONS §171 (where a program image starts) option D), which gives a program image 496 MiB at the
//! shared base, so the substitution and the second script it produced are gone.

fn main() {
    println!("cargo::rustc-link-arg=-Tlink.ld");
    println!("cargo::rustc-link-arg=-u_start");
    println!("cargo::rustc-link-arg=--build-id=none");
}
