//! Links the payload at the address U-Boot's `booti` places it, and carries the crate commit in.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let target = std::env::var("TARGET").unwrap();
    assert!(
        target.starts_with("riscv64"),
        "jh7110_entropy_board_check builds for riscv64 only, not {target}"
    );
    println!("cargo:rustc-link-arg=-T{dir}/link-riscv64.ld");
    println!("cargo:rerun-if-changed={dir}/link-riscv64.ld");
    println!("cargo:rerun-if-env-changed=BOARD_CHECK_CRATE_COMMIT");
}
