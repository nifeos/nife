//! Measured boot: the digest of the boot program, handed to the kernel build (milestone 22 phase
//! B.1, DECISIONS §22).
//!
//! The kernel loads exactly one program itself, the boot program, and until now it loaded whatever
//! bytes it was handed. Now the build measures that entry and the kernel image carries the digest, so
//! the check means "this kernel runs exactly this progenitor." The ordering is one-way and the build
//! already had it: userspace -> archive -> manifest -> kernel. See kernel/build.rs (which consumes
//! the manifest) and notes/trusted-init.md.

use std::path::PathBuf;

use crate::host::workspace_root;

/// The archive entries the kernel itself may enter as the boot program. Everything else in the
/// archive is loaded by the progenitor, in userspace, so it is not part of the kernel's trust root.
///
/// **`progenitor` is on every list** (milestone 266): one first process, one name, on all three
/// boards. Before that the entry was called `init` and meant a different binary depending on the
/// architecture, and this table was where that showed.
///
/// **The six ELF child builders are on every list too, and they are not boot programs in the
/// ordinary sense.** They are milestone 19d (run a real workload)'s and 19e's userspace parents,
/// and `spawn_elf_child_builder` enters each directly; `trust::require` refuses any entry the trust
/// root does not name. A kernel that could enter a program it never measured would be the hole
/// measured boot exists to close, so the entry is here rather than the check being relaxed there.
///
/// **It stopped being per-architecture at milestone 295, and took its `arch` parameter with it.**
/// riscv64 and `x86_64` used to add `builder`, milestone 20 (a portable HAL)'s richer initial
/// ramdisk demo, because the RISC-V boot tour entered it directly to prove that userspace composes
/// the system; calef retired that program on 2026-09-14 once milestone 268 (every architecture
/// boots the same way) made the default riscv64 boot hand over to the progenitor, which makes the
/// same claim at a larger scale. So the kernel enters the same seven programs on every board, and
/// a trust root that differed by architecture is one fewer thing for a reader to hold.
///
/// The parameter went rather than being kept for a future divergence, because an argument nothing
/// reads is a claim that something varies when nothing does. `write_measure_manifest` still takes
/// an `arch` and still writes one manifest per architecture; if a board's list ever needs to differ
/// again, the parameter comes back at that point with a reason attached. The absent-name path below
/// it is the one that was already written for lists that differ, and it is kept.
///
/// **The list itself moved to `sealed_pair::BOOT_PROGRAMS`**, for milestone 223 (read a card and say whether its kernel and archive match).
/// It was written out here and
/// again in `uefi_loader/build.rs`, and a third copy was about to be written in the card checker.
/// A reader of a kernel image has to know which names its trust root can hold, so the list belongs
/// where all three of them can reach it; this function stays because the manifest writer below is
/// its one caller here and because this block is where the list's reasons are recorded.
pub(crate) fn boot_programs() -> &'static [&'static str] {
    &sealed_pair::BOOT_PROGRAMS
}

/// Where the measurement manifest for an architecture is written. `kernel/build.rs` derives exactly
/// this path from `CARGO_CFG_TARGET_ARCH`, so the two stay in lockstep without an env var to forget.
fn measure_manifest_path(arch: &str) -> PathBuf {
    workspace_root().join(format!("target/init-measure-{arch}.txt"))
}

/// **The table the progenitor measures its own loads against** (milestone 104), packed as an ordinary archive
/// entry under [`measured_boot::PROGRAM_MEASUREMENTS`].
///
/// One line per program, in the same `name sha256:<hex>` format the kernel's manifest uses, because
/// there is one format and one parser (`measured_boot::manifest_entries`). Every entry in the
/// archive is measured except the table itself, which cannot contain its own digest; that includes
/// the boot programs the kernel already measures, which costs nothing and means a reader does not
/// have to know which side of the boundary a name falls on to find it here.
///
/// **Sorted**, so the table is a function of the archive's contents and not of the order the packer
/// happened to list them in. An unsorted table would rewrite itself, and therefore relink the
/// kernel, whenever somebody reordered the file list for readability.
///
/// **Why this is an archive entry rather than something compiled into the progenitor.** The kernel's own trust
/// root is compiled into the kernel image, which works because the kernel is not in the archive it
/// measures. The progenitor is. Generating a table of its siblings into its own binary would mean building
/// userspace, measuring it, and building userspace again, with a "and nothing else changed in the
/// second build" invariant holding up the whole chain. Packing the table beside the programs and
/// letting the kernel's trust root name it buys the same guarantee with a one-pass build: the
/// kernel vouches for the table exactly as it vouches for the progenitor.
pub(crate) fn measurement_table(files: &[(&str, &[u8])]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .filter(|(name, _)| *name != measured_boot::PROGRAM_MEASUREMENTS)
        .map(|(name, bytes)| {
            let hex = measured_boot::digest_text(&measured_boot::sha256(bytes));
            let hex = std::str::from_utf8(&hex).expect("a digest's text is ascii");
            format!("{name} {hex}\n")
        })
        .collect();
    lines.sort();
    let mut text = String::from(
        "# generated by cargo xtask: every program in this archive, for the progenitor to measure what it \
         loads\n",
    );
    for line in lines {
        text.push_str(&line);
    }
    text
}

/// Hash the boot-program entries out of the archive we just packed and write the manifest.
///
/// It parses the packed image back with `nifefs` rather than hashing the input file, deliberately:
/// what must be measured is the bytes **the kernel will read out of the archive**, not the bytes we
/// meant to put in. If packing ever mangled an entry, this measures the mangling and the boot fails,
/// which is the correct direction to be wrong in.
pub(crate) fn write_measure_manifest(arch: &str, image: &[u8]) -> bool {
    let fs = match nifefs::Fs::parse(image) {
        Ok(fs) => fs,
        Err(e) => {
            eprintln!("measure: the archive we just packed does not parse: {e:?}");
            return false;
        }
    };
    let mut text = format!(
        "# generated by cargo xtask; the boot programs this {arch} kernel image is built against\n"
    );
    // The boot programs the kernel may enter, plus the table it vouches for on the progenitor's behalf
    // (milestone 104). The kernel never reads the table's contents; it hashes the entry and refuses
    // to hand the archive over if it is not the one this kernel image was built against, which is
    // what makes the progenitor's refusals worth as much as its own measurement.
    for name in boot_programs()
        .iter()
        .copied()
        .chain([measured_boot::PROGRAM_MEASUREMENTS])
    {
        let Some(bytes) = fs.read(name) else {
            // Not every archive has to carry every boot program. A name that is absent simply
            // gets no measurement, and the kernel refuses to enter a program it has no measurement
            // for, so nothing is quietly waved through. No list differs today (milestone 295 took
            // `builder` off riscv64's and `x86_64`'s, which was the one that did); this is kept
            // because the lists are allowed to differ and a `None` here must not be a panic.
            continue;
        };
        let digest = measured_boot::sha256(bytes);
        let hex = measured_boot::digest_text(&digest);
        let hex = std::str::from_utf8(&hex).expect("a digest's text is ascii");
        text.push_str(&format!("{name} {hex}\n"));
    }
    let path = measure_manifest_path(arch);
    // Write only on change, so an unchanged userspace does not make build.rs relink the kernel.
    if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
        return true;
    }
    if let Err(e) = std::fs::write(&path, &text) {
        eprintln!("measure: cannot write {}: {e}", path.display());
        return false;
    }
    true
}
