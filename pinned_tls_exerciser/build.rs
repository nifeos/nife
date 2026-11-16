//! The same linker contract as `std_exerciser` and `cryptography_exerciser`: the shared
//! `link.ld` that `user_mode_runtime` delivers, as is.

fn main() {
    println!("cargo::rustc-link-arg=-Tlink.ld");
    println!("cargo::rustc-link-arg=-u_start");
    println!("cargo::rustc-link-arg=--build-id=none");
}
