//! The FS-server EL0 bins are ordinary nife ELFs, so they link against the SAME linker script
//! `components`, `fixtures` and `std_exerciser` use, `crates/user_mode_runtime/link.ld` (linked at
//! `address_space_map::IMAGE_BASE`, explicit W^X PHDRS). The link args are scoped to the EL0 bins
//! (`rustc-link-arg-bin`), never the lib or the host test binary, which link the host way.
//!
//! Every EL0 bin this package builds is listed below: the loader refuses an image outside the
//! address-space map's image band (milestone 206 (a program image has under 896 KiB)), and a bin
//! left off links at lld's default address.

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=../crates/user_mode_runtime/link.ld");
    for bin in ["redoxfs_server", "mkfs"] {
        println!("cargo::rustc-link-arg-bin={bin}=-T{dir}/../crates/user_mode_runtime/link.ld");
        // `_start` lives in each bin, but forcing it undefined is the same belt-and-braces
        // std_exerciser uses so the entry survives whatever the linker's ENTRY timing.
        println!("cargo::rustc-link-arg-bin={bin}=-u_start");
        // Keep the ELF an ELF: the kernel's loader wants program headers, not a build-id note.
        println!("cargo::rustc-link-arg-bin={bin}=--build-id=none");
    }
}
