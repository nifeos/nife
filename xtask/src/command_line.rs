//! **The command line**: which subcommand runs, and the HVF choice the QEMU ones read.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

pub(crate) fn main() -> ExitCode {
    let cmd = std::env::args().nth(1).unwrap_or_default();

    let ok = match cmd.as_str() {
        "build" => build(),
        "run" => {
            maybe_hvf();
            // Build the disk and the initial ramdisk first: the kernel boots with them, and `cargo
            // run` would not rebuild them on its own (the kernel does not depend on them in cargo).
            mkdisk() && user() && cargo(&["run", "-p", "kernel", "--target", TARGET])
        }
        "shell" => {
            // The default kernel, with the devices and fixture a person at the prompt wants.
            maybe_hvf();
            eprintln!("--- booting nife to an interactive shell (type `help`, Ctrl-C to quit) ---");
            // A virtio-rng device (DECISIONS §120's 2026-08-26 amendment: "grant the QEMU-only
            // virtio-rng stopgap"), the same terms `swish_check_leg` already attaches one on: this
            // is the interactive boot itself, not the bench boot sharing its runner, so there is
            // no icount-drift reason to keep it test-leg only, and the whole point of the
            // amendment is that a person booting this way should have one.
            // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads.
            // xtask is single-threaded up to this point in `main`, before `cargo(&[..])` spawns
            // its child, so there is no concurrent reader to race.
            unsafe { std::env::set_var("NIFE_RNG", "1") };
            // **The filesystem the prompt's `>` and `<` need** (milestone 50). The FS server first,
            // because `user()` packs the initial ramdisk and the boot loads it out of there by
            // name, and the RedoxFS image because the runner attaches it only when the file exists.
            // Both are rebuilt per boot, so the prompt always meets a fresh fixture rather than
            // whatever the last session wrote.
            redoxfs_server_build(TARGET)
                && mkredoxfs()
                && mkdisk()
                && user()
                && cargo(&["run", "-p", "kernel", "--target", TARGET])
        }
        // The aarch64 archive, standalone (2026-08-27): every other caller reaches
        // `initial_ramdisk_aarch64` through `user()` as part of a boot (`build`, `run`, `shell`,
        // ...), and `initial_ramdisk_aarch64` itself only packs, it does not build.
        // `initial_ramdisk_riscv` and `initial_ramdisk_x86` both build-then-pack in one call, so
        // this subcommand calls `user()` (build, then pack) rather than `initial_ramdisk_aarch64()`
        // alone, to give aarch64 the same self-contained entry point its two siblings already have.
        "initial-ramdisk-aarch64" => user(),
        "initial-ramdisk-riscv" => initial_ramdisk_riscv(),
        // The third archive (milestone 161). Same programs, built for x86_64.
        "initial-ramdisk-x86" => initial_ramdisk_x86(),
        // The bootable UEFI image (milestone 87): the entry real firmware can start, staged at
        // target/esp for a QEMU/OVMF boot or for a FAT32 stick. See notes/x86-uefi-boot.md.
        // Name: `uefi-image` and `uefi-boot` ratified 2026-08-30 (calef, in session, on milestone
        // 87's lane report). Hyphenated like every other subcommand, and each names what it
        // produces rather than the tool that produces it.
        "uefi-image" => uefi_image_command(),
        // The same image, booted under OVMF and checked. Runs inside `script/test --arch x86_64`;
        // exposed on its own because the bench procedure starts by watching this pass locally.
        "uefi-boot" => uefi_boot_command(),
        // Milestone 198's rung 3a, the producer half: a reviewed recipe in, one package file and
        // its digest out. Name provisional (2026-09-23).
        "package" => package::package(std::env::args().nth(2)),
        "install-boot" => install::install_boot(),
        "rollback-boot" => rollback::rollback_boot(),
        // Rung 2b's other half: a GOOD upgrade is tried, confirmed by the running system, and
        // still chosen on the next boot with no tries left. The exact negative of the line above,
        // and the one that proves the rollback is a policy rather than an accident. Name
        // provisional (2026-09-21): `stick-boot` was already the boot stick's gate.
        "confirm-boot" => confirm::confirm_boot(),
        // Milestone 195: the same firmware, the kernel's test binary instead of its tour.
        "uefi-test" => uefi_test_command(),
        // The stick (DECISIONS §157): every architecture's boot file, sealed, and `stick_maker`
        // built around them; then the same directory booted under all three firmwares. See
        // xtask/src/stick.rs and notes/boot-stick.md. Names provisional (2026-09-19).
        "stick" => stick::stick(),
        "stick-boot" => stick::stick_boot(),
        // Milestone 578 (an ACPI discovery path for aarch64): the aarch64 stick with ACPI and no device tree, as a cloud machine boots
        // it. Name provisional (2026-10-19 UTC).
        "acpi-boot" => stick::acpi_boot(),
        // **The boot tour read back off a `ramfb`**, milestone 243 (a machine with no serial port),
        // on the two architectures whose firmware never lights a screen. `uefi-boot`'s twin: same
        // decoder, same claim, a different way of getting a framebuffer. Name provisional.
        "screen-boot" => {
            let arch = std::env::args().nth(2).unwrap_or_else(|| "aarch64".into());
            screen::screen_boot(&arch)
        }
        // The documentation store (milestone 40): build it, print what it costs, and optionally
        // answer a query against it with the same reader the guest uses.
        "manual" => manual_store(std::env::args().nth(2)),
        // The tree-wide search (milestone 40, script/apropos): the same index and the same reader,
        // pointed at this repository instead of at what the image installs. See `tree_apropos`.
        "apropos" => tree_apropos(std::env::args().nth(2)),
        "std-src" => std_src(),
        // Print the farm's input stamp and exit. Exists so that "the stamp does not depend on where
        // the checkout lives" is a claim anyone can CHECK rather than one they have to believe:
        //   cargo xtask std-stamp                        # in the main checkout
        //   git worktree add /tmp/w HEAD && (cd /tmp/w && cargo xtask std-stamp)
        // The two must print the same value. If they ever diverge, something location-dependent has
        // crept back into `std_inputs_stamp`, and every worktree's farm will rebuild on every run.
        "std-stamp" => {
            println!("{:016x}", std_inputs_stamp());
            true
        }
        "std-exerciser" => std_exerciser(),
        // A manifest note as an object of its own, for a program whose source is somebody else's
        // (milestone 595 (the shell runs a `std` program)). basalt's package build links it into
        // `rg`. Name provisional (2026-10-07). See xtask/src/foreign_note.rs.
        "foreign-note" => foreign_note::foreign_note(),
        // The abort sweep (milestone 64): which std calls kill a nife process instead of refusing
        // it. Runs at the end of `std-exerciser` (and so inside `script/test`); exposed on its own
        // because re-reading the list after a nightly bump should not need a rebuild.
        "std-aborts" => std_aborts(),
        // basalt's published packages, fetched by their pinned digests, and the host's reference
        // `rg` (milestone 810 (`ripgrep` is packaged in basalt and installed with `jig`)). Name
        // provisional. See xtask/src/basalt.rs.
        "basalt-fetch" => basalt::fetch(),
        "swish-check" => swish_check(),
        // The boot ladder's gate (milestone 268, item 6): boot every architecture's default kernel
        // and fail if its self-test verdict is not green. See script/boot-check.
        "boot-check" => boot_check(),
        "test" => test(),
        // Milestone 807 (the kernel suite reports what each test cost): the time record as markdown,
        // for a CI step summary. Name provisional. See xtask/src/time_record.rs.
        "time-summary" => time_record::summary(),
        "undefined-behavior-check" => undefined_behavior_check(),
        "bench" => bench(),
        // The instruction-count instrument (milestone 78): the two timing claims a wall clock
        // cannot make, on both ISAs. See script/icount.
        "icount" => icount(),
        "gdb" => gdb(),
        "objdump" => objdump(),
        "image" => image(),
        // The real board's serial console (milestone 216). Returns its own exit code rather than a
        // bool, so a bench script can tell "reached the banner" from "went quiet" from "ran out".
        "board-console" => return board_console(),
        // The sustained multicore run under QEMU (milestone 219), judged by the same recogniser
        // `board-console` points at a board. Returns its own exit code for the same reason.
        "soak-test" => return soak_test(),
        // The multi-tasking workload sweep under QEMU (milestone 168). Returns its own exit code
        // for `soak-test`'s reason: a rehearsal that cannot say *how* it failed is not a rehearsal.
        "job-mix" => return job_mix_sweep(),
        // Fatal risk 6's bench boot (milestone 261 (the NVMe driver leaves the kernel)): stage the stick image and rehearse its four
        // verdicts under OVMF. Name provisional. See xtask/src/disk_throughput.rs.
        "disk-throughput" => return disk_throughput::disk_throughput(),
        // Milestone 494 (a driver for the network card a PC actually has)'s bench boot: stage the
        // stick image and rehearse its verdict under OVMF against a slirp peer. Name provisional.
        // See xtask/src/network_bench.rs.
        "network-bench" => return network_bench::network_bench(),
        // The card's U-Boot script (milestone 218): what makes the board boot without a person at
        // its prompt. `script/board-image` calls this; it is a separate verb so the script it
        // produces can be rebuilt and read on its own.
        "board-script" => board_script(),
        // **milestone 223 (read a card and say whether its kernel and archive match).**
        // Returns its own exit code rather than a bool, because "this pair
        // would be refused" and "there is nothing here to check" want different reactions from
        // whoever ran it, and a bench script wants to tell them apart.
        "card-check" => return card_check::card_check(std::env::args().nth(2)),
        other => {
            usage::print_usage(other);
            return ExitCode::FAILURE;
        }
    };

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The packed initial ramdisk archive ([`archive::initial_ramdisk_path`]) is what
/// `helpers/qemu-runner-aarch64.sh` passes to QEMU as `-initrd` (milestone 19f (run a real
/// workload)); the raw user ELFs ([`host::bin_elf`]) are only the input `initial_ramdisk_aarch64`
/// packs.
///
/// **Deliberately the same road Linux's initramfs travels**, now literally an archive like theirs.
/// QEMU loads the file into RAM and writes its address into `/chosen/linux,initrd-start` in the
/// device tree; the kernel finds it there (`memory::initial_ramdisk_region`, built at milestone 3
/// (hand out physical memory) for exactly this). Nothing about the contents is known to the kernel
/// at build time, which is the entire point of milestone 7c (user mode: EL0, capabilities, the ELF
/// loader, and IPC).
///
/// If `--hvf` was passed, boot under Apple's Hypervisor.framework instead of TCG.
pub(crate) fn maybe_hvf() {
    if std::env::args().any(|a| a == "--hvf") {
        // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
        // threads. xtask is single-threaded here: this runs on the main thread before the child
        // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
        // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
        unsafe { std::env::set_var("NIFE_ACCEL", "hvf") };
        eprintln!("--- on the real Apple Silicon core via Hypervisor.framework ---");
    }
}
