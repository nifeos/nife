//! Same linker contract as `components` and `fixtures`: the shared `link.ld` that `user_mode_runtime` delivers
//! (linked at `address_space_layout::IMAGE_BASE`, explicit W^X PHDRS), because a std program is still an ordinary nife ELF
//! the loader maps.
//! `-u _start` belts-and-braces the entry: `_start` lives in std's rlib (the PAL), and forcing
//! it undefined guarantees the archive member is extracted whatever the linker's ENTRY timing.

fn main() {
    println!("cargo::rustc-link-arg=-Tlink.ld");
    println!("cargo::rustc-link-arg=-u_start");
    println!("cargo::rustc-link-arg=--build-id=none");
}
