//! Link a C program into `c_program` (milestone 835 (a C library, stage 1: files, clock and
//! memory)).
//!
//! `NIFE_C_ARCHIVE` names the static archive of the C program's objects, built by
//! `helpers/build-c-program.sh` with the flags `c_library/README.md` lists. It is linked into the
//! `c_program` binary only; the library itself needs nothing linked. The link script and the two
//! arguments after it are the ones every in-tree `std` program takes (`std_exerciser/build.rs`).
use std::process::Command;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    compile_seeded_c(&dir);
    println!("cargo::rerun-if-env-changed=NIFE_C_ARCHIVE");
    println!("cargo::rerun-if-env-changed=NIFE_C_NOTE");
    println!("cargo::rerun-if-changed=../crates/user_mode_runtime/link.ld");
    println!("cargo::rustc-link-arg-bins=-T{dir}/../crates/user_mode_runtime/link.ld");
    println!("cargo::rustc-link-arg-bins=-u_start");
    println!("cargo::rustc-link-arg-bins=--build-id=none");
    if let Ok(archive) = std::env::var("NIFE_C_ARCHIVE") {
        println!("cargo::rerun-if-changed={archive}");
        println!("cargo::rustc-link-arg-bins={archive}");
    }
    // The program's manifest note (milestone 597 (a program carries its manifest in an ELF note)),
    // the object `cargo xtask foreign-note` writes, as `helpers/build-ripgrep.sh` links one into
    // `rg`.
    if let Ok(note) = std::env::var("NIFE_C_NOTE") {
        println!("cargo::rerun-if-changed={note}");
        println!("cargo::rustc-link-arg-bins={note}");
    }
}

/// relibc's one C file (`../vendor/relibc/c/stdlib.c`: `long double` conversions), compiled with
/// the flags every C program for nife takes, into a static library linked with this crate.
fn compile_seeded_c(dir: &str) {
    println!("cargo::rerun-if-changed=../vendor/relibc/c/stdlib.c");
    println!("cargo::rerun-if-changed=../helpers/c-library-cflags.sh");
    println!("cargo::rerun-if-env-changed=NIFE_CC");
    let target = std::env::var("TARGET").unwrap();
    let out = std::env::var("OUT_DIR").unwrap();
    // x86_64: clang refuses `long double` with the x87 off, so the file cannot be compiled there
    // and no C program there can pass the type; `src/long_double.rs` supplies the one symbol
    // relibc's `printf` links against.
    if target.starts_with("x86_64") {
        return;
    }
    let flags = Command::new(format!("{dir}/../helpers/c-library-cflags.sh"))
        .arg(&target)
        .output()
        .expect("c_library: cannot run helpers/c-library-cflags.sh");
    assert!(
        flags.status.success(),
        "c_library: no C flags for target {target}"
    );
    let flags = String::from_utf8(flags.stdout).unwrap();
    let cc = clang();
    let obj = format!("{out}/stdlib.o");
    let ok = Command::new(&cc)
        .args(flags.split_whitespace())
        .args([
            "-O2",
            "-c",
            &format!("{dir}/../vendor/relibc/c/stdlib.c"),
            "-o",
            &obj,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(
        ok,
        "c_library: {cc} could not compile ../vendor/relibc/c/stdlib.c"
    );
    let ar = llvm_tool("llvm-ar");
    let lib = format!("{out}/libc_library_seeded.a");
    let _ = std::fs::remove_file(&lib);
    let ok = Command::new(&ar)
        .args(["rcs", &lib, &obj])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(ok, "c_library: {ar} could not archive stdlib.o");
    println!("cargo::rustc-link-search=native={out}");
    println!("cargo::rustc-link-lib=static=c_library_seeded");
}

/// A clang with all three backends, found as fixtures/build.rs and helpers/build-speedtest1.sh
/// find one.
fn clang() -> String {
    if let Ok(cc) = std::env::var("NIFE_CC") {
        return cc;
    }
    for c in [
        "/opt/homebrew/opt/llvm/bin/clang",
        "/usr/local/opt/llvm/bin/clang",
        "clang",
    ] {
        let has_riscv = Command::new(c)
            .arg("-print-targets")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("riscv64"))
            .unwrap_or(false);
        if has_riscv {
            return c.to_string();
        }
    }
    panic!("c_library: no clang with the AArch64, RISC-V and X86 backends; set NIFE_CC");
}

/// An LLVM tool from the toolchain's `llvm-tools` component (the farm is a clone of it).
fn llvm_tool(name: &str) -> String {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let sysroot = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    let sysroot = String::from_utf8(sysroot.stdout).unwrap();
    let host = std::env::var("HOST").unwrap();
    format!("{}/lib/rustlib/{host}/bin/{name}", sysroot.trim())
}
