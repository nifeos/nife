//! **What `cargo xtask` says when it is not given a command it knows**: the one-line list of
//! every subcommand and the longer forms of the ones that take options.
//!
//! Moved out of `main` so the dispatch reads as the table it is. `main.rs` is a parent that §282
//! (when a file is split, the parent becomes an index) wants smaller, and this is one less thing
//! in it.
//!
//! Name: provisional, minted 2026-10-13 (UTC) by lane maintainer/long-function-paydown-xtask.

/// Print the usage to stderr, after naming `other` when it was a command this does not know.
pub(crate) fn print_usage(other: &str) {
    if !other.is_empty() {
        eprintln!("unknown command: {other}\n");
    }
    eprintln!(
        "usage: cargo xtask <build|run|shell|swish-check|boot-check|initial-ramdisk-aarch64|initial-ramdisk-riscv|initial-ramdisk-x86|uefi-image|uefi-boot|uefi-test|package|base-image|base-packages|install-boot|rollback-boot|confirm-boot|update-boot|stick|stick-boot|acpi-boot|screen-boot|manual|apropos|std-src|std-stamp|std-exerciser|std-aborts|foreign-note|basalt-fetch|test|coverage|time-summary|undefined-behavior-check|bench|icount|gdb|objdump|image|board-console|soak-test|board-script|card-check> [--hvf]"
    );
    eprintln!("       cargo xtask swish-check [--arch aarch64|riscv64]");
    eprintln!(
        "       cargo xtask install-boot   (x86_64/OVMF: a stick installs itself onto an NVMe disk, then that disk boots with the stick detached)"
    );
    eprintln!(
        "       cargo xtask rollback-boot  (x86_64/OVMF: a doomed upgrade is tried, and the machine comes back on the previous image by itself)"
    );
    eprintln!(
        "       cargo xtask confirm-boot   (x86_64/OVMF: a good upgrade is tried, confirms itself, and is still chosen with no tries left)"
    );
    eprintln!(
        "       cargo xtask update-boot    (x86_64/OVMF: the composed image is staged into slot 1 by the host, confirms itself, and is still chosen with no tries left)"
    );
    eprintln!(
        "       cargo xtask card-check [<mounted card>|<stick dir>|<boot file>]   (default: target/board)"
    );
    eprintln!("       cargo xtask boot-check [--arch aarch64|riscv64|x86_64] [--inject]");
    eprintln!(
        "       cargo xtask undefined-behavior-check [extra cargo-miri-test args, e.g. -p <crate>]"
    );
    eprintln!(
        "       cargo xtask bench [--riscv | --x86] [--real] [--release] [--smp] [--check] [--save --why <reason>]"
    );
    eprintln!(
        "       cargo xtask test [--arch aarch64|riscv64|x86_64[,...]] [--no-host] [--cpu <qemu-cpu-model>] [--hvf] [--test <substring>] [--image kernel|system]"
    );
    eprintln!(
        "       cargo xtask coverage   (the aarch64 kernel test image, instrumented; target/kernel-coverage)"
    );
    eprintln!("       cargo xtask icount [--arch aarch64|riscv64]");
    eprintln!(
        "       cargo xtask board-console [--port <dev>] [--replay <log>] [--log <file>] [--for <duration>] [--until spl|opensbi|uboot|handoff|banner|machine|selftest|tour|prompt|none] [--quiet-after <duration>] [--stop | --stop-after <n>]"
    );
}
