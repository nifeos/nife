//! Picks the link script for the target being built.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let target = std::env::var("TARGET").unwrap();
    let script = if target.starts_with("aarch64") {
        "link-aarch64.ld"
    } else if target.starts_with("riscv64") {
        "link-riscv64.ld"
    } else {
        panic!("host_reordering_litmus builds for aarch64 and riscv64 guests only, not {target}");
    };
    println!("cargo:rustc-link-arg=-T{dir}/{script}");
    println!("cargo:rerun-if-changed={dir}/{script}");
    println!("cargo:rerun-if-env-changed=LITMUS_SCALE");
}
