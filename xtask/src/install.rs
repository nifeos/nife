//! **The install gate**: a stick boots, puts itself on a disk, and the machine boots from that disk
//! with the stick gone (milestone 198 (a package manager, and the trivial install that makes a
//! second customer possible), rung 2a.
//!
//! Milestone 515 (a stick that puts itself on the machine's disk) is the proposal it follows.
//!
//! # Why it is two boots, and then a third
//!
//! **A sequence that only works while the stick is still attached has not installed anything**, so
//! the second boot is the gate, and three properties keep it from succeeding by accident:
//!
//! - **The stick is not attached at all**, not merely deprioritised. `NIFE_UEFI_NO_STICK=1` removes
//!   the vvfat drive from the machine.
//! - **The firmware variable store is deleted before each boot.** The default store persists across
//!   runs in a checkout, so boot 1 would leave a boot option that boot 2 then rode.
//! - **The NVMe image is created empty**, every run, so nothing survives from the last one.
//!
//! The third boot is the stick again, on the machine it installed, with nobody typing anything
//! ([`stick_on_the_installed_machine`]). It is the live stick proposal's G1 and G2 as a measurement:
//! which system ran, and whether the disk changed by a single byte.
//!

//! # EXAMPLES
//!
//! ```console
//! $ cargo xtask install-boot
//! --- the stick installs itself onto an empty NVMe disk ---
//!   install     : installed. nife data at LBA 2048, EFI system at LBA 1046528.
//! --- boot 2 of 3: the same machine with the stick detached ---
//! BdsDxe: loading Boot0001 "UEFI QEMU NVMe Ctrl nife-nvme 1" ...
//! $ ls
//!   made-on-target
//! --- boot 3 of 3: the stick again, with the installed disk attached ---
//! uefi_loader: the boot slots here are on another disk; this file's medium has none, so it is using the image in this file
//! install-boot: boot 3 left the installed disk byte-for-byte unchanged
//! install-boot: PASS
//! ```
//!
//! # BUGS
//!
//! - **It is one disk of one size**, a 1 GiB image. The installer's layout arithmetic has a floor
//!   and a rounding step that nothing here varies, and a disk under about 600 MiB is refused by
//!   code no test exercises.
//! - **It proves nothing about a real firmware.** OVMF is one implementation and a generous one.
//! - **It runs nowhere on its own** (about 42 seconds warm, yet not in `script/test`'s default legs
//!   and not scheduled): `design/roadmap/0712-the-install-gates-run-nowhere.md`.
//! - **Boot 3 proves the stick left the disk alone under OVMF, and only there.** What keeps it so
//!   is the `boot_slot::medium` token (calef, 2026-10-04 UTC): the loader
//!   writes it only when its file is on an NVMe namespace, and a real firmware that built that
//!   device path differently would leave an installed machine without its filesystem rather than
//!   a stick with one.
//! - **Boot 3 needs the disk in the firmware's boot order.** OVMF connects only the devices its boot
//!   options name, so with the stick alone the loader never sees the disk and G1 cannot happen.
//!   `NIFE_NVME_DEVICE_OPTS=,bootindex=1` is what makes it the machine a boot menu would give.
//! - **A failure leaves the NVMe image behind**, on purpose: it is the evidence, and
//!   `hdiutil attach -imagekey diskimage-class=CRawDiskImage` on the partition reads the EFI system
//!   partition it wrote.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::X86_TARGET;
use crate::disk::redoxfs_server_build;
use crate::host::workspace_root;
use crate::uefi::{uefi_kernel, uefi_stage};

/// The disk the install goes onto. **One gibibyte**: comfortably over the installer's own floor
/// (a 512 MiB EFI system partition plus its 64 MiB minimum for data), and a sparse file, so it
/// costs what is written to it and nothing else.
const DISK_BYTES: u64 = 1024 * 1024 * 1024;

/// Where the boot files are staged for this gate. Its own directory rather than `target/esp`,
/// because that one is what the bench procedure copies to a stick and nothing here should decide
/// when it is rebuilt.
pub(crate) fn install_esp_dir() -> PathBuf {
    workspace_root().join("target/esp-install")
}

/// An empty directory to hand the runner as its ESP argument on the second boot. The runner still
/// requires one and still has to find it; what it does not do is attach it.
pub(crate) fn empty_esp_dir() -> PathBuf {
    workspace_root().join("target/esp-detached")
}

/// The NVMe image this gate installs onto, written empty at the start of every run.
pub(crate) fn install_disk_path() -> PathBuf {
    workspace_root().join("target/nife-install.img")
}

/// This gate's own firmware variable store. See the module header: deleting it is a load-bearing
/// part of the second boot's claim.
pub(crate) fn vars_path() -> PathBuf {
    workspace_root().join("target/ovmf-vars-install.fd")
}

/// **`ls`, typed at the shell's first prompt**: the banner, then the `$ ` after it. The banner comes
/// first so that `$ ` is read only once the shell exists; a send's marker is looked for after the
/// previous one's, and an empty send types nothing.
pub(crate) const LS_AT_THE_PROMPT: [(&str, &str); 2] = [(boot_ladder::PROMPT, ""), ("$ ", "ls\r")];

/// **The gate.** `true` only if both boots did what they were supposed to.
pub(crate) fn install_boot() -> bool {
    if !prepare() {
        return false;
    }
    if install_once().is_none() {
        return false;
    }

    eprintln!();
    eprintln!("--- boot 2 of 3: the same machine with the stick detached ---");
    let Some(second) = boot(
        &empty_esp_dir(),
        true,
        &[
            LS_AT_THE_PROMPT[0],
            LS_AT_THE_PROMPT[1],
            // And then read the file back, which is what makes this an install rather than a boot.
            ("made-on-target", "wc made-on-target\r"),
            // **The disk tree, beside the filesystem on the same disk** (milestone 834 (ioping on nife and Linux)): the
            // NVMe server's two clients, each in its own transfer window, the arrangement xenon
            // runs `ioping` in. `caps` says where it is mounted and `ls` reads its directory.
            ("1 10 57", "caps\r"),
            ("mounted at /disk", "ls /disk/nvme0\r"),
        ],
        // `ls`'s row for the disk tree's one file.
        "  n1",
        300,
    ) else {
        return false;
    };

    let mut ok = true;
    for wanted in [
        // The firmware found the file on the disk, with no boot variable and no boot index.
        "UEFI QEMU NVMe Ctrl",
        // The loader's own first line, which it has printed since milestone 87 (the x86_64
        // bare-metal machine) and which says the firmware started what was written rather than
        // something it found elsewhere.
        "nife uefi_loader: milestone 87",
        // Rung 2b: the installed disk's own chooser picked the slot the install marked good, and
        // the image it started knows which slot it came from.
        "uefi_loader: starting boot slot 0",
        "uefi_loader: started from boot slot 0",
        "nife: handing the system to the userspace progenitor.",
        // A prompt.
        "nife capability shell.",
        // The file `mkfs` wrote before the reboot, listed and then read.
        "made-on-target",
        // `wc`'s answer for `filesystem_fixture::blank::MADE_BODY`: one line, ten words,
        // fifty-seven bytes. Written out rather than derived, because a gate that computed the
        // number from the same constant the program wrote would be checking its own arithmetic.
        "1 10 57",
        "a second tree, mounted at /disk",
        "  n1",
    ] {
        if !second.contains(wanted) {
            eprintln!("install-boot: boot 2's transcript is missing {wanted:?}");
            ok = false;
        }
    }
    // **The offer must not be made to a machine that already carries nife**, or every boot of every
    // installed machine would pause to ask whether to wipe itself.
    if second.contains("EVERYTHING ON THAT DISK WILL BE DESTROYED") {
        eprintln!("install-boot: boot 2 offered to install over an installed machine");
        ok = false;
    }
    if !ok {
        return false;
    }
    if !stick_on_the_installed_machine() {
        return false;
    }
    eprintln!("install-boot: PASS");
    true
}

/// **Boot 3: the stick again, on the machine it just installed, and nobody types anything.**
///
/// The live stick proposal (`design/roadmap/0773-a-live-stick.md`, G1 and G2) read two defects
/// out of the code: the stick's loader chain-loads the installed disk's boot slot instead of
/// running its own image, and a stick boot mounts the internal disk, which writes it. This boot is
/// the measurement. It asserts which system ran, and it compares the disk byte for byte before and
/// after, so a write anywhere (the partition table, the filesystem, the EFI system partition) fails
/// it and is named by region.
///
/// It comes after boot 2 on purpose: by then the installed system has booted once and confirmed
/// itself, so anything this boot changes is the stick's doing and not the install settling.
fn stick_on_the_installed_machine() -> bool {
    eprintln!();
    eprintln!("--- boot 3 of 3: the stick again, with the installed disk attached ---");
    let before = workspace_root().join("target/nife-install-before-stick.img");
    let _ = std::fs::remove_file(&before);
    // `fs::copy` clones on APFS, so a 1 GiB sparse image costs nothing to keep.
    if let Err(e) = std::fs::copy(install_disk_path(), &before) {
        eprintln!("install-boot: could not snapshot the installed disk: {e}");
        return false;
    }
    let Some(third) = boot_lingering(
        &install_esp_dir(),
        false,
        // `ls` asks for the filesystem, which is what a person at a live stick does first.
        &LS_AT_THE_PROMPT,
        "nife capability shell.",
        300,
        // A few seconds after the prompt, so a write the mount or `ls` started has landed.
        5,
        // **The disk is a boot option too**, second to the stick. That is what makes OVMF connect
        // the NVMe controller's block devices before it starts the stick: with the stick alone in
        // the boot order it connects only the stick, the loader then sees two `BlockIo` handles
        // and no slots, and G1 cannot happen (measured 2026-10-04). A real firmware's one-time
        // boot menu has to enumerate every disk to list them, so this is the faithful machine.
        &[("NIFE_NVME_DEVICE_OPTS", ",bootindex=1")],
    ) else {
        return false;
    };

    // **The firmware must have started the stick**, or nothing below measures the stick at all.
    // The first run of this boot did not: OVMF preferred the disk, and the gate read the installed
    // system's own boot as the stick's.
    if third
        .lines()
        .any(|l| l.contains("BdsDxe: starting") && l.contains("NVMe"))
    {
        eprintln!(
            "install-boot: boot 3's firmware started the disk, not the stick; nothing was measured"
        );
        return false;
    }
    let mut ok = true;
    // G1: which system ran. A slot is the installed disk's system; this file is the stick's own.
    if third.contains("uefi_loader: starting boot slot") || third.contains("started from boot slot")
    {
        eprintln!(
            "install-boot: boot 3 ran the INSTALLED system: the stick's loader chain-loaded the disk's boot slot (G1)"
        );
        ok = false;
    }
    if !third.contains("uefi_loader: the boot slots here are on another disk") {
        eprintln!("install-boot: boot 3 never said it left the other disk's boot slots alone");
        ok = false;
    }
    if third.contains("EVERYTHING ON THAT DISK WILL BE DESTROYED") {
        eprintln!("install-boot: boot 3 offered to install over an installed machine");
        ok = false;
    }
    // G2, and G1's write: nothing on the disk may change.
    let changed = disk_changes(&before, &install_disk_path());
    match changed {
        None => ok = false,
        Some(regions) if regions.is_empty() => {
            eprintln!("install-boot: boot 3 left the installed disk byte-for-byte unchanged");
        }
        Some(regions) => {
            for r in regions {
                eprintln!("install-boot: boot 3 WROTE the installed disk: {r}");
            }
            ok = false;
        }
    }
    if ok {
        let _ = std::fs::remove_file(&before);
    }
    ok
}

/// **Which regions of `after` differ from `before`**, named by the partition table `before` carries,
/// or `None` when either image cannot be read. Empty means the two are identical.
fn disk_changes(before: &std::path::Path, after: &std::path::Path) -> Option<Vec<String>> {
    use globally_unique_identifier_partition_table as gpt;
    const LBA: u64 = 512;
    let (Ok(mut a), Ok(mut b)) = (std::fs::File::open(before), std::fs::File::open(after)) else {
        eprintln!("install-boot: could not reopen the disk images to compare them");
        return None;
    };
    let total = a.metadata().ok()?.len();
    // The regions, from the table as it was before the boot: LBA 0 to 33, every partition, and the
    // backup table in the last 33 blocks.
    let mut head = vec![0u8; 34 * LBA as usize];
    a.read_exact(&mut head).ok()?;
    let mut regions: Vec<(u64, u64, String)> = vec![(
        0,
        33,
        "LBA 0-33, the protective MBR and the primary partition table".into(),
    )];
    if let Ok(table) = gpt::GloballyUniqueIdentifierPartitionTable::parse(
        &head[LBA as usize..2 * LBA as usize],
        &head[2 * LBA as usize..],
    ) {
        for (i, p) in table.partitions() {
            let name = gpt::guid::types::name(p.type_guid).unwrap_or("unknown type");
            regions.push((
                p.first_lba,
                p.last_lba,
                format!("partition {i} ({name}, LBA {}-{})", p.first_lba, p.last_lba),
            ));
        }
    }
    let last = total / LBA - 1;
    regions.push((last - 32, last, "the backup partition table".into()));

    use std::io::Seek;
    a.seek(std::io::SeekFrom::Start(0)).ok()?;
    let mut hits: Vec<(String, u64, u64)> = Vec::new();
    let mut x = vec![0u8; 1 << 20];
    let mut y = vec![0u8; 1 << 20];
    let mut at = 0u64;
    while at < total {
        let n = ((total - at) as usize).min(x.len());
        a.read_exact(&mut x[..n]).ok()?;
        b.read_exact(&mut y[..n]).ok()?;
        if x[..n] != y[..n] {
            for (k, (p, q)) in x[..n]
                .chunks(LBA as usize)
                .zip(y[..n].chunks(LBA as usize))
                .enumerate()
            {
                if p == q {
                    continue;
                }
                let lba = at / LBA + k as u64;
                let name = regions
                    .iter()
                    .find(|(f, l, _)| (*f..=*l).contains(&lba))
                    .map_or_else(|| "outside every partition".to_string(), |r| r.2.clone());
                match hits.iter_mut().find(|h| h.0 == name) {
                    Some(h) => {
                        h.1 = h.1.min(lba);
                        h.2 += 1;
                    }
                    None => hits.push((name, lba, 1)),
                }
            }
        }
        at += n as u64;
    }
    Some(
        hits.into_iter()
            .map(|(name, first, count)| format!("{count} block(s) in {name}, first at LBA {first}"))
            .collect(),
    )
}

/// **Build everything and write an empty disk**, which is what any gate starting from a bare
/// machine needs. Shared with `cargo xtask rollback-boot`, which begins with the same install.
pub(crate) fn prepare() -> bool {
    // **An installed image is always a release build** (calef, 2026-10-03 UTC, the install boot
    // slots: "keep 64 MiB with both fixes"). Every gate that begins here stages the file the
    // installer writes into a slot, so the profile is set here and not left to a flag: a debug
    // image is 15 MB where release is a few, and the loader refuses to offer a debug image for
    // install at all (`uefi_loader::image::carries_debug_info`), so this gate would find no
    // installer to type at. The debug default is for fast rebuilds in the test paths, none of
    // which install anything.
    crate::RELEASE.store(true, std::sync::atomic::Ordering::Relaxed);
    // `mkfs` and the FS server, which the archive packs only if something built them for this
    // target. Without `mkfs` the install partitions the disk and leaves the data partition empty,
    // and the second boot then has nothing to read back.
    if !redoxfs_server_build(X86_TARGET) {
        eprintln!("install-boot: could not build the FS server and mkfs for {X86_TARGET}");
        return false;
    }
    // `uefi_kernel` packs the archive first and then builds the kernel, which is the order the
    // measured-boot seal requires; see its own comment.
    let Some(kernel) = uefi_kernel(None) else {
        return false;
    };
    if !uefi_stage(
        &kernel,
        &install_esp_dir(),
        "the loader, the kernel and the archive",
        false,
    ) {
        return false;
    }
    if std::fs::create_dir_all(empty_esp_dir()).is_err() {
        eprintln!(
            "install-boot: could not create {}",
            empty_esp_dir().display()
        );
        return false;
    }
    write_empty_disk()
}

/// **Boot the stick once and let it install itself.** The transcript, or `None` if anything in the
/// install did not happen.
pub(crate) fn install_once() -> Option<String> {
    eprintln!();
    eprintln!("--- the stick installs itself onto an empty NVMe disk ---");
    let first = boot(
        &install_esp_dir(),
        false,
        // The one thing a person does, typed when the question is on the wire. The marker is the
        // prompt itself, which ends without a newline, so the reader below cannot be line-based.
        &[("install     : > ", "INSTALL\r")],
        // The whole line, not a prefix of it: the loop below stops the machine the instant this
        // appears, so a prefix leaves the rest of the line unread and the assertion for the full
        // sentence then fails on a boot that did everything right.
        "install     : DONE. Remove the installation medium and reboot.",
        420,
    )?;

    // The loader's refusal to offer a debug image, which would mean this gate staged one.
    if first.contains("a debug image is not offered for install") {
        eprintln!("install: the stick carried a DEBUG kernel; an install image must be release");
        return None;
    }
    for wanted in [
        "install     : this system was booted from a file and can install itself.",
        "install     :   EVERYTHING ON THAT DISK WILL BE DESTROYED.",
        "install     : installed. nife data at LBA 2048",
        "install     : filesystem created.",
        "install     : DONE. Remove the installation medium and reboot.",
    ] {
        if !first.contains(wanted) {
            eprintln!("install: the install transcript is missing {wanted:?}");
            return None;
        }
    }
    Some(first)
}

/// Write `DISK_BYTES` of nothing, replacing whatever the last run left.
pub(crate) fn write_empty_disk() -> bool {
    match std::fs::File::create(install_disk_path()).and_then(|f| f.set_len(DISK_BYTES)) {
        Ok(()) => true,
        Err(e) => {
            eprintln!(
                "install-boot: could not write {}: {e}",
                install_disk_path().display()
            );
            false
        }
    }
}

/// **Boot the machine once**, typing `sends` when each one's marker appears on the wire, and stop
/// as soon as `until` has been printed.
///
/// Reads bytes rather than lines, which is not a detail: the installer's question ends in `> ` with
/// no newline, so a line-based reader would not see it until after the answer was due.
pub(crate) fn boot(
    esp: &PathBuf,
    detached: bool,
    sends: &[(&str, &str)],
    until: &str,
    timeout_secs: u64,
) -> Option<String> {
    boot_lingering(esp, detached, sends, until, timeout_secs, 0, &[])
}

/// [`boot`], but keep the machine running `linger_secs` after `until` appears, for a gate that
/// measures what the machine wrote rather than what it printed.
fn boot_lingering(
    esp: &PathBuf,
    detached: bool,
    sends: &[(&str, &str)],
    until: &str,
    timeout_secs: u64,
    linger_secs: u64,
    env: &[(&str, &str)],
) -> Option<String> {
    // The firmware's variables, fresh. See the module header for why this is the load-bearing line
    // of the second boot.
    let _ = std::fs::remove_file(vars_path());

    let mut command = Command::new("helpers/qemu-uefi-x86_64.sh");
    command
        .arg(esp)
        .current_dir(workspace_root())
        .env("NIFE_NVME", install_disk_path())
        .env("NIFE_OVMF_VARS_OUT", vars_path())
        .env("NIFE_UEFI_TIMEOUT", timeout_secs.to_string())
        // The machine is a disk and a firmware and nothing else, whether or not a suite leg set
        // these in this process first. One boot with two machines is one boot nobody can compare.
        .env_remove("NIFE_DISK")
        .env_remove("NIFE_UEFI_REDOXFS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.envs(env.iter().copied());
    if detached {
        command.env("NIFE_UEFI_NO_STICK", "1");
    } else {
        // The person at the machine picked the stick. Harmless on an empty disk, where the stick
        // is the only thing to boot, and load-bearing on an installed one: see the runner.
        command.env("NIFE_UEFI_STICK_FIRST", "1");
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("install-boot: failed to run helpers/qemu-uefi-x86_64.sh: {e}");
            return None;
        }
    };

    let mut serial = child.stdin.take().expect("piped stdin");
    let mut output = child.stdout.take().expect("piped stdout");
    // stderr is the runner's own complaints, drained on a thread so a full pipe cannot wedge it.
    let errors = {
        let mut from = child.stderr.take().expect("piped stderr");
        std::thread::spawn(move || {
            let mut mine = String::new();
            let _ = from.read_to_string(&mut mine);
            mine
        })
    };

    let mut transcript = String::new();
    let mut since_send = String::new();
    let mut pending = sends.iter();
    let mut next = pending.next();
    let mut buf = [0u8; 4096];
    loop {
        let n = match output.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
        print!("{chunk}");
        let _ = std::io::stdout().flush();
        transcript.push_str(&chunk);
        since_send.push_str(&chunk);

        let mut stuck = false;
        while let Some((marker, text)) = next
            && let Some(at) = since_send.find(marker)
        {
            // The machine has just printed a prompt; the byte after it is the one being waited for.
            // A short settle keeps the answer out of the same read the firmware is still draining.
            std::thread::sleep(std::time::Duration::from_millis(500));
            if serial.write_all(text.as_bytes()).is_err() || serial.flush().is_err() {
                eprintln!("install-boot: could not type {text:?} at the machine");
                stuck = true;
                break;
            }
            // Cut through the marker, so two sends that share a marker substring cannot both fire
            // on one appearance of it. What follows it is kept, because the next marker can arrive
            // in the same read: the shell's banner and the `$ ` after it usually do.
            since_send.drain(..at + marker.len());
            next = pending.next();
        }
        if stuck {
            break;
        }

        if next.is_none() && transcript.contains(until) {
            break;
        }
    }
    if linger_secs > 0 && transcript.contains(until) {
        // Drained on a thread so the guest's console cannot block on a full pipe meanwhile.
        let drain = std::thread::spawn(move || {
            let mut rest = Vec::new();
            let _ = output.read_to_end(&mut rest);
            String::from_utf8_lossy(&rest).into_owned()
        });
        std::thread::sleep(std::time::Duration::from_secs(linger_secs));
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status();
        let rest = drain.join().unwrap_or_default();
        print!("{rest}");
        transcript.push_str(&rest);
    }

    // The kernel never exits, so the machine is stopped rather than waited out: SIGTERM to the
    // wrapper, which is what `qemu-bounded.sh` forwards to QEMU, with its own killer as the
    // backstop.
    let _ = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status();
    let _ = child.wait();
    transcript.push_str(&errors.join().unwrap_or_default());

    if !transcript.contains(until) {
        eprintln!("install-boot: the boot never printed {until:?}");
        return None;
    }
    Some(transcript)
}
