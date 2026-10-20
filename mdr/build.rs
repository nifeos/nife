//! Same linker contract as `components` and `fixtures`: the shared `crates/user_mode_runtime/link.ld`
//! (linked at `address_space_map::IMAGE_BASE`, explicit W^X PHDRS), because a std program is still an ordinary nife ELF
//! the loader maps.
//! `-u _start` belts-and-braces the entry: `_start` lives in std's rlib (the PAL), and forcing
//! it undefined guarantees the archive member is extracted whatever the linker's ENTRY timing.

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=../crates/user_mode_runtime/link.ld");
    println!("cargo::rustc-link-arg=-T{dir}/../crates/user_mode_runtime/link.ld");
    println!("cargo::rustc-link-arg=-u_start");
    println!("cargo::rustc-link-arg=--build-id=none");
}
