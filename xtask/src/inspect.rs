//! Looking at what was built: `gdb`, `objdump`, and the flat `image` with its header dumped.

use std::process::Command;

use crate::host::{kernel_elf, llvm_tool, run, workspace_root};
use crate::{RISCV_TARGET, RUNNER, TARGET, X86_TARGET, build};

/// Boot the kernel with QEMU frozen and a GDB stub listening.
///
/// `-s` opens the stub on :1234, `-S` holds the CPU before the first instruction.
/// The kernel ELF carries symbols and DWARF, so GDB shows Rust source lines rather
/// than raw addresses (notes/elf.md). Point GDB at the **ELF**, even though QEMU is
/// running the flat image: the image has no symbols, and the addresses match.
///
/// This is the tool that will save you at milestone 4, when the MMU comes on and
/// `println!` stops being an option.
pub(crate) fn gdb() -> bool {
    if !build() {
        return false;
    }

    let elf = kernel_elf();
    eprintln!("QEMU is paused, waiting for a debugger on localhost:1234.");
    eprintln!("In another terminal:");
    eprintln!();
    eprintln!("    gdb {elf}");
    eprintln!("    (gdb) target remote :1234");
    eprintln!("    (gdb) break kernel_main");
    eprintln!("    (gdb) continue");
    eprintln!();
    eprintln!("To watch boot.s set up the stack and zero .bss:");
    eprintln!();
    eprintln!("    (gdb) break _boot");
    eprintln!("    (gdb) layout asm");
    eprintln!("    (gdb) si          # step one instruction");
    eprintln!();

    run(RUNNER, &[&elf, "-s", "-S"])
}

pub(crate) fn objdump() -> bool {
    if !build() {
        return false;
    }
    match llvm_tool("llvm-objdump") {
        Some(tool) => run(
            &tool,
            &[
                "-d",
                "--no-show-raw-insn",
                "-M",
                "no-aliases",
                &kernel_elf(),
            ],
        ),
        None => false,
    }
}

/// Build the flat arm64 Image and show its 64-byte header.
///
/// Useful when the header is wrong, which is a failure mode with no diagnostics at
/// all: QEMU simply falls back to treating the file as an anonymous blob, boots it,
/// and hands you a zero in x0. See notes/boot-protocol.md.
pub(crate) fn image() -> bool {
    if !build() {
        return false;
    }
    let Some(objcopy) = llvm_tool("llvm-objcopy") else {
        return false;
    };

    let elf = kernel_elf();
    let img = format!("{elf}.img");
    // --remove-section: see the matching comment in helpers/qemu-runner-aarch64.sh. The ELF keeps
    // `.eh_frame`/`.eh_frame_hdr` (milestone: CFI in hand-written asm) for a debugger to read;
    // this flat Image, the thing a bootloader actually loads, does not need them along for the
    // ride.
    if !run(
        &objcopy,
        &[
            "-O",
            "binary",
            "--remove-section=.eh_frame",
            "--remove-section=.eh_frame_hdr",
            &elf,
            &img,
        ],
    ) {
        return false;
    }

    match std::fs::read(&img) {
        Ok(bytes) if bytes.len() >= 64 => {
            let magic = u32::from_le_bytes(bytes[56..60].try_into().unwrap());
            let text_offset = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
            let image_size = u64::from_le_bytes(bytes[16..24].try_into().unwrap());

            eprintln!("{img}  ({} bytes)", bytes.len());
            eprintln!();
            eprintln!("  text_offset  {text_offset:#x}");
            eprintln!("  image_size   {image_size:#x}");
            eprintln!(
                "  magic        {magic:#010x}  {}",
                if magic == 0x644d5241 {
                    "ok (\"ARM\\x64\")"
                } else {
                    "WRONG - QEMU will not treat this as a kernel"
                }
            );
            magic == 0x644d5241
        }
        Ok(_) => {
            eprintln!("image is shorter than its own 64-byte header");
            false
        }
        Err(e) => {
            eprintln!("cannot read {img}: {e}");
            false
        }
    }
}

/// The namespace `read_stripped` files a stripped copy under, from the triple in `path`.
///
/// Only a triple names an architecture or a std build; the checkout's own directory name is not
/// evidence of anything, so no arm may match on a bare `nife`.
fn strip_tag(path: &str) -> &'static str {
    if path.contains(RISCV_TARGET) {
        "riscv"
    } else if path.contains(X86_TARGET) {
        "x86"
    } else if path.contains(TARGET) {
        // Provisional name: the aarch64 bare-metal tag, so `host` means real host builds only.
        "aarch64"
    } else if path.contains("riscv64-unknown-nife") {
        "std-riscv"
    } else if path.contains("x86_64-unknown-nife") {
        "std-x86"
    } else if path.contains("aarch64-unknown-nife") {
        "std"
    } else {
        "host"
    }
}

/// **Read the program ELF at `path` for packing, with its debug information removed**
/// (`llvm-objcopy --strip-debug`), and return the stripped bytes.
///
/// The initial ramdisk is reserved RAM and a debug build is almost all `.debug_*`, which nothing
/// reads (`crates/elf` parses program headers only; faults are symbolised offline against
/// `target/`). `--strip-debug`, not `--strip-all`, keeps the symbol table. A missing `llvm-objcopy`
/// is a hard failure, because the measured-boot digest (§26 (the fault endpoint)'s phase B.1) is
/// taken over these bytes, and a build whose bytes depended on installed tools would have a
/// different trust root on each machine. See notes/progenitor-and-loading.md, "The archive carries
/// stripped ELFs".
pub(crate) fn read_stripped(path: &str) -> std::io::Result<Vec<u8>> {
    let objcopy = llvm_tool("llvm-objcopy").ok_or_else(|| {
        std::io::Error::other("llvm-objcopy not found; the llvm-tools rustup component provides it")
    })?;
    let out = workspace_root().join("target/stripped");
    std::fs::create_dir_all(&out)?;
    let stem = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("program");
    // Namespaced by the triple the binary was built for, so two architectures' builds of a program
    // cannot overwrite each other's stripped copy and have the wrong one measured into a trust
    // root. Each arm of `strip_tag` is anchored on a full triple: an x86 Linux CI runner's host
    // paths contain `x86_64-unknown-linux-gnu`, and every checkout lives under a directory called
    // `nife`.
    let tag = strip_tag(path);
    let dst = out.join(format!("{tag}-{stem}"));
    // Fail before running the tool if the input is missing, so the caller's error message names the
    // binary it wanted rather than objcopy's exit status.
    std::fs::metadata(path)?;
    let status = Command::new(&objcopy)
        .arg("--strip-debug")
        .arg(path)
        .arg(&dst)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "{objcopy} --strip-debug {path} failed ({status})"
        )));
    }
    std::fs::read(&dst)
}

#[cfg(test)]
mod strip_tag_tests {
    use super::strip_tag;

    const CHECKOUT: &str = "/home/runner/work/nife/nife";

    #[test]
    fn std_program_is_tagged_std() {
        let p = format!("{CHECKOUT}/std_exerciser/target/aarch64-unknown-nife/release/x");
        assert_eq!(strip_tag(&p), "std");
    }

    #[test]
    fn bare_metal_program_gets_its_own_tag_despite_a_checkout_named_nife() {
        let p = format!("{CHECKOUT}/target/aarch64-unknown-none-softfloat/release/init");
        assert_eq!(strip_tag(&p), "aarch64");
    }

    #[test]
    fn host_build_is_tagged_host() {
        let p = format!("{CHECKOUT}/target/release/xtask");
        assert_eq!(strip_tag(&p), "host");
    }
}
