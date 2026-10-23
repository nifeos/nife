//! radon's storage bench boot, behind the `storage_bench` feature, riscv64 only (name
//! provisional).
//!
//! [`run`] replaces the hand-over: for the microSD slot and then the eMMC socket it reports the
//! clocks, the controller and the card as found, runs the read-only probe and a timed read (and,
//! only in a `NIFE_STORAGE_BENCH_WRITE=scratch` build, the scratch write test on the microSD slot),
//! repeats them through the IDMAC and the EL0 block server, prints a `storage-bench:` verdict and
//! halts. See notes/bench-boots.md and notes/designware-mobile-storage.md.

//! # BUGS
//! - The booted system has no block device on radon yet; this boot picks its own window.
//! - It re-initializes the card U-Boot booted from, so it halts rather than hand the card back.
//! - The eMMC socket is probed read-only, always.

use designware_mobile_storage::bench::{self, ReadFailure, ReadReport, WriteVerdict};
use designware_mobile_storage::host::{Error, Host, Identity, Registers};
use designware_mobile_storage::idmac::{DirectMemoryAccessRegion, Idmac, Refusal};
use designware_mobile_storage::jh7110::Slot;
use designware_mobile_storage::regs;
use designware_mobile_storage::sd::{self, BLOCK, InitError, IoError, Kind};

use crate::designware_mobile_storage::{Region, Window};
use crate::{arch, println};

/// Whether this build carries the scratch write test: `NIFE_STORAGE_BENCH_WRITE=scratch` at build
/// time, and nothing else, so a card is never written by a build that did not ask in so many words.
const WRITE_TEST: bool = match option_env!("NIFE_STORAGE_BENCH_WRITE") {
    Some(v) => is_scratch(v.as_bytes()),
    None => false,
};

/// `v == b"scratch"`, in a `const` context, where string comparison is not yet allowed.
const fn is_scratch(v: &[u8]) -> bool {
    matches!(v, b"scratch")
}

/// The timed read's length: 8 MiB, long enough that a polled read at default speed takes about a
/// second, short enough that a slow card does not stall the boot.
const TIMED_BLOCKS: u64 = 16_384;
/// Pages in the timed read's buffer: 64 KiB, 128 blocks per `CMD18`.
const READ_PAGES: usize = 16;

/// Run the probe on both slots, print the verdict, and never come back.
pub fn run() -> ! {
    println!();
    println!(
        "storage-bench: milestone 53 storage bench boot, {} build, {} Hz counter. {}",
        if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "release"
        },
        arch::timer::frequency(),
        if WRITE_TEST {
            "WRITE TEST BUILT IN: the microSD card's pre-partition gap only."
        } else {
            "Read-only build: nothing on any card is written."
        },
    );
    let verdict = match crate::memory::jh7110_storage() {
        None => Outcome::NotDescribed,
        Some(slots) => {
            let sd = slots[1].map_or(Outcome::NotDescribed, |s| probe(&s, WRITE_TEST));
            if let Some(s) = slots[0] {
                let _ = probe(&s, false);
            } else {
                println!("storage-bench: slot 0 (eMMC socket): not in the device tree");
            }
            sd
        }
    };
    print_verdict(&verdict);
    #[cfg(feature = "storage_read_matrix")]
    if let Some(slots) = crate::memory::jh7110_storage()
        && let Some(sd) = slots[1]
    {
        read_matrix(&sd);
        println!(
            "storage-matrix: THIS BUILD RESETS THE BOARD when the matrix is done, so a netbooted \
             bench can fetch its next image."
        );
        let refused = crate::reboot::cold_reset("storage-matrix:");
        println!("storage-matrix: every reset route was refused ({refused:?})");
    }
    println!("storage-bench: done, halting.");
    arch::halt(arch::HaltReason::measurement_boot());
}

/// How the microSD slot's probe ended: decided where each step ends, so the verdict line cannot
/// claim more than the step that produced it.
#[derive(Debug)]
enum Outcome {
    /// No JH7110 SD/MMC controller in the tree, or no microSD one.
    NotDescribed,
    /// No SYS clock window was recorded, so nothing was touched.
    NoClockWindow,
    /// A clock did not read back enabled, or the reset did not release.
    ClocksDown,
    /// The controller refused (its FIFO width).
    Controller(Error),
    /// The kernel had no pages for a buffer.
    NoMemory,
    /// Identification stopped: no card, or a card that did not finish.
    NoCard(InitError),
    /// A read failed: the block and the error.
    ReadFailed(u64, IoError),
    /// Read-only, and every polled read succeeded: the timed rate in KiB/s, and the IDMAC phase.
    ReadOk(u64, Dma),
    /// Read, then the scratch write test: the rate, the test's verdict, and the IDMAC phase.
    Written(u64, WriteVerdict, Dma),
    /// The kernel's own probe passed and the EL0 block server's did not.
    El0(El0),
}

/// How the IDMAC phase ended.
#[derive(Debug)]
enum Dma {
    /// No pages for the region or the buffers: nothing ran.
    NoMemory,
    /// The region was refused before anything was written.
    Refused(Refusal),
    /// A read failed, polled (the reference) or through the IDMAC.
    Failed(ReadFailure),
    /// The timed read's rate in KiB/s, whether the first 64 KiB matched the polled read, and in a
    /// write build the scratch write test through the IDMAC.
    Read(u64, bool, Option<WriteVerdict>),
}

impl Dma {
    /// Whether the IDMAC did everything this build asked of it.
    fn passed(&self) -> bool {
        matches!(
            self,
            Dma::Read(
                _,
                true,
                None | Some(WriteVerdict::Verified(..) | WriteVerdict::Refused)
            )
        )
    }
}

impl core::fmt::Display for Dma {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Dma::Read(kib, true, None) => write!(f, "IDMAC READ-OK {kib} KiB/s"),
            Dma::Read(kib, true, Some(WriteVerdict::Verified(..))) => {
                write!(f, "IDMAC READ-OK {kib} KiB/s, IDMAC WRITE-VERIFIED")
            }
            Dma::Read(kib, true, Some(WriteVerdict::Refused)) => {
                write!(f, "IDMAC READ-OK {kib} KiB/s")
            }
            Dma::Read(kib, false, _) => {
                write!(
                    f,
                    "IDMAC FAILED: {kib} KiB/s but the bytes differ from the polled read"
                )
            }
            Dma::Read(_, true, Some(v)) => write!(f, "IDMAC FAILED: write test {v:?}"),
            Dma::NoMemory => write!(f, "IDMAC FAILED: no pages for its region or buffers"),
            Dma::Refused(e) => write!(f, "IDMAC FAILED: region refused, {e:?}"),
            Dma::Failed(e) => write!(f, "IDMAC FAILED: {e:?}"),
        }
    }
}

fn print_verdict(o: &Outcome) {
    match o {
        Outcome::ReadOk(kib, dma) => println!(
            "storage-bench: verdict {} {kib} KiB/s, {dma}, EL0 block server served",
            if dma.passed() {
                "READ-OK"
            } else {
                "FAILED: polled READ-OK"
            },
        ),
        Outcome::El0(e) => println!("storage-bench: verdict FAILED: EL0 block server {e:?}"),
        Outcome::Written(kib, WriteVerdict::Verified(first, count, blank), dma) => println!(
            "storage-bench: verdict {} {kib} KiB/s, WRITE-VERIFIED sectors {first}..{} (were {}), restored, {dma}, EL0 block server served",
            if dma.passed() {
                "READ-OK"
            } else {
                "FAILED: polled READ-OK"
            },
            first + count,
            if *blank { "zero" } else { "not zero" },
        ),
        Outcome::Written(kib, WriteVerdict::Refused, dma) => println!(
            "storage-bench: verdict READ-OK {kib} KiB/s, WRITE-REFUSED: no gap before the first partition, {dma}"
        ),
        Outcome::Written(_, v, _) => println!("storage-bench: verdict FAILED: write test {v:?}"),
        Outcome::NotDescribed => {
            println!("storage-bench: verdict SKIPPED: the tree describes no microSD controller");
        }
        Outcome::NoCard(e) => println!("storage-bench: verdict NO-CARD: {e:?}"),
        Outcome::Controller(e) => println!("storage-bench: verdict FAILED: controller {e:?}"),
        Outcome::ReadFailed(lba, e) => {
            println!("storage-bench: verdict FAILED: read of block {lba}: {e:?}");
        }
        other => println!("storage-bench: verdict FAILED: {other:?}"),
    }
}

/// Print one line about slot `i`, prefixed the same way every time.
macro_rules! say {
    ($slot:expr, $($arg:tt)*) => {
        println!(
            "storage-bench: slot {} ({}): {}",
            $slot.index,
            $slot.role(),
            format_args!($($arg)*)
        )
    };
}

/// Bring the slot's card clock to the tree's rate, and return the rate it really runs at.
///
/// Without the SYS syscon (mapped only where the tree names the PMIC's bus) the rate cannot be
/// computed, so the tree's is returned and printed as unchecked.
fn card_clock(slot: &Slot, crg_phys: u64) -> u32 {
    if crate::memory::jh7110_pmic_bus().is_none() {
        say!(
            slot,
            "card clock: no SYS syscon mapped, so its rate is the tree's {} Hz, unchecked",
            slot.ciu_hz
        );
        return slot.ciu_hz;
    }
    // SAFETY: `mmu::init` mapped the SYS CRG (step 6c) and, because the tree names the PMIC's
    // bus, the SYS syscon (step 6c-prime); the slot index is 0 or 1 by `jh7110_storage`.
    let c = unsafe {
        crate::drivers::jh7110_clock_and_reset::set_card_clock(
            crate::arch::mmu::phys_to_virt(crg_phys) as usize,
            crate::arch::mmu::phys_to_virt(jh7110_clock_and_reset::SYS_SYSCON_BASE) as usize,
            usize::from(slot.index),
            slot.ciu_hz,
        )
    };
    say!(
        slot,
        "card clock: {:#010x} -> {:#010x}, {} Hz from a {} Hz parent (the tree asks {} Hz)",
        c.before,
        c.after,
        c.hz,
        c.parent_hz,
        slot.ciu_hz
    );
    c.hz
}

fn probe(slot: &Slot, write: bool) -> Outcome {
    say!(
        slot,
        "{:#x}+{:#x}, compatible {}, status {}, bus-width {}, fifo-depth {:?}, ciu {} Hz, interrupt {:?}",
        slot.base,
        slot.size,
        core::str::from_utf8(slot.compatible).unwrap_or("?"),
        if slot.status_okay { "okay" } else { "disabled" },
        slot.bus_width,
        slot.fifo_depth,
        slot.ciu_hz,
        slot.interrupt,
    );

    let Some(sys) = crate::memory::jh7110_sys_window() else {
        say!(
            slot,
            "no SYS clock window recorded; not touching the controller"
        );
        return Outcome::NoClockWindow;
    };
    let plan = jh7110_clock_and_reset::SDIO_BRING_UP[usize::from(slot.index)];
    // SAFETY: `memory::jh7110_sys_window` answers only on a JH7110, and `mmu::init` step 6c mapped
    // exactly this window device-typed. The plan and the domain are the same crate's, so every
    // identifier is in range; every step is idempotent on a controller already up.
    let report = unsafe {
        crate::drivers::jh7110_clock_and_reset::bring_up(
            crate::arch::mmu::phys_to_virt(sys.base) as usize,
            &jh7110_clock_and_reset::SYS,
            plan,
        )
    };
    say!(
        slot,
        "clocks before {:#010x} {:#010x} after {:#010x} {:#010x} (dividers {} {}), reset assert {:#x}->{:#x} status {:#x}, released {}, already up {}",
        report.clock_before[0],
        report.clock_before[1],
        report.clock_after[0],
        report.clock_after[1],
        jh7110_clock_and_reset::clock_divider(report.clock_after[0]),
        jh7110_clock_and_reset::clock_divider(report.clock_after[1]),
        report.reset_assert_before,
        report.reset_assert_after,
        report.reset_status_after,
        report.released,
        report.was_already_up(),
    );
    if !report.has_clocks_running() || !report.released {
        return Outcome::ClocksDown;
    }

    let ciu_hz = card_clock(slot, sys.base);

    // SAFETY: step 10 of `mmu::init` mapped this slot's first page from the same
    // `memory::jh7110_storage` answer, and this boot thread is the only thing that drives it.
    let window = unsafe { Window::new(slot.base) };
    let (mut host, id) = match Host::new(window, ciu_hz, slot.fifo_depth) {
        Ok(h) => h,
        Err(e) => {
            say!(slot, "controller refused: {e:?}");
            return Outcome::Controller(e);
        }
    };
    print_identity(slot, &id, host.fifo_depth());

    let Some(buf) = pages(READ_PAGES) else {
        return Outcome::NoMemory;
    };
    let r = match bench::read_only(&mut host, slot.bus_width, buf, TIMED_BLOCKS) {
        Err(ReadFailure::Init(e)) => {
            say!(slot, "identification stopped: {e:?}");
            return Outcome::NoCard(e);
        }
        Err(ReadFailure::Read(lba, e)) => {
            say!(slot, "read of block {lba} failed: {e:?}");
            // TCBCNT counts bytes card to FIFO and TBBCNT FIFO to CPU: the CPU ahead of the card
            // is an underrun, the card ahead with a full FIFO is an overrun.
            let r = host.registers();
            let (status, tcb, tbb) = (
                r.read(regs::STATUS),
                r.read(regs::TCBCNT),
                r.read(regs::TBBCNT),
            );
            say!(
                slot,
                "at the failure: STATUS {status:#x} (FIFO holds {} words) TCBCNT {tcb} TBBCNT {tbb}",
                (status >> 17) & 0x1fff
            );
            return Outcome::ReadFailed(lba, e);
        }
        Ok(r) => r,
    };
    print_read(slot, &r);
    let outcome = write_and_idmac(slot, host, &id, &r, write);
    // The kernel is done with this controller: `through_idmac` took `host` and stopped the IDMAC,
    // and from here the EL0 server drives it.
    if slot.index == 1 {
        let el0 = through_el0(slot, &r, write);
        say!(slot, "EL0 block server: {el0:?}");
        if el0 != El0::Served && !matches!(outcome, Outcome::Written(_, WriteVerdict::Refused, _)) {
            return Outcome::El0(el0);
        }
    }
    outcome
}

/// **The probe's last kernel steps**: in a write build the scratch write test, polled, then the
/// IDMAC phase, which takes the host. The outcome carries both.
fn write_and_idmac(
    slot: &Slot,
    mut host: Host<Window>,
    id: &Identity,
    r: &ReadReport,
    write: bool,
) -> Outcome {
    let verdict = if write {
        let (Some(a), Some(b)) = (pages(1), pages(1)) else {
            return Outcome::NoMemory;
        };
        let v = bench::scratch_write(&mut host, &r.card, r.mbr.as_ref(), a, b);
        say!(slot, "scratch write test: {v:?}");
        Some(v)
    } else {
        None
    };
    let dma = through_idmac(slot, host, id, r, write);
    say!(slot, "{dma}");
    let kib = kib_per_s(r.timed_blocks, r.timed_us);
    match verdict {
        Some(v) => Outcome::Written(kib, v, dma),
        None => Outcome::ReadOk(kib, dma),
    }
}

/// How the EL0 phase ended.
#[derive(Debug, PartialEq, Eq)]
enum El0 {
    /// Every check passed: the server came up, served reads of the right bytes, refused what it
    /// must refuse, and (in a write build) wrote, read back and restored its one block.
    Served,
    /// The archive has no `designware_mobile_storage` program.
    NoProgram,
    /// The partition table gives no window to serve.
    NoWindow,
    /// The service refused to start.
    NotStarted,
    /// The server's readiness message said a step failed: its words.
    NotReady(u64, u64, u64),
    /// A request came back wrong: which check, and the server's answer.
    Wrong(&'static str, i64),
}

/// The same card through the EL0 block server, called the way the FS server calls one.
///
/// A read-only build serves the first partition and checks block 0, `EROFS` and `EINVAL`; a
/// `write` build serves the scratch range and round-trips one block. See notes/bench-boots.md.
fn through_el0(slot: &Slot, r: &ReadReport, write: bool) -> El0 {
    use filesystem_contract::blk;

    use crate::user::designware_mobile_storage_service as service;

    say!(
        slot,
        "EL0 block server: starting it (PROVEN_ON_SILICON is {}; this boot is the proof the booted system waits for)",
        service::PROVEN_ON_SILICON
    );
    let Some(image) = crate::trust::require_program("designware_mobile_storage") else {
        return El0::NoProgram;
    };
    let Some(mbr) = r.mbr else {
        return El0::NoWindow;
    };
    let Some(window) = el0_window(&mbr, write) else {
        return El0::NoWindow;
    };
    let Ok(w) = service::start(image, slot, window) else {
        return El0::NotStarted;
    };
    let ready = crate::sched::ipc_receive(w.ready);
    if ready[0] != filesystem_contract::READY {
        return El0::NotReady(ready[0], ready[1], ready[2]);
    }
    let call = |op, block| crate::sched::ipc_call(w.request, [blk::req(op, 1), block])[0] as i64;
    // SAFETY: the service allocated these pages for the server and its one client, which this boot
    // thread now is; the direct map covers them, and the server touches them only inside a call.
    let shared = unsafe {
        core::slice::from_raw_parts_mut(
            crate::arch::mmu::phys_to_virt(w.transfer_phys) as *mut u8,
            blk::BLOCK_SIZE,
        )
    };
    if call(blk::SIZE, 0) != (window.sectors * BLOCK as u64) as i64 {
        return El0::Wrong("SIZE", call(blk::SIZE, 0));
    }
    let past = window.sectors / 8;
    let got = call(blk::READ, past);
    if got != filesystem_contract::reply_err(22) {
        return El0::Wrong("READ past the window is EINVAL", got);
    }
    let got = call(blk::READ, 0);
    if got != 1 {
        return El0::Wrong("READ block 0", got);
    }
    if !write {
        return read_only_checks(&call, shared, r);
    }
    write_cycle_checks(&call, shared, window)
}

/// The window the EL0 server serves: the scratch range in a write build, else the first used
/// partition, rounded down to whole blocks. `None` when the card has neither.
fn el0_window(
    mbr: &designware_mobile_storage::partition::Mbr,
    write: bool,
) -> Option<designware_mobile_storage::serve::Window> {
    use designware_mobile_storage::serve::Window;

    let window = if write {
        match mbr.scratch(bench::SCRATCH_SECTORS) {
            Some((first, 8)) => Window {
                first: u64::from(first),
                sectors: 8,
                writable: true,
            },
            _ => return None,
        }
    } else {
        let p = mbr.entries.iter().find(|e| e.is_used() && e.sectors >= 8)?;
        Window {
            first: u64::from(p.start),
            sectors: u64::from(p.sectors) & !7,
            writable: false,
        }
    };
    Some(window)
}

/// A read-only build's checks: block 0 is the boot sector the kernel read, and a WRITE is refused.
fn read_only_checks(call: &impl Fn(u64, u64) -> i64, shared: &[u8], r: &ReadReport) -> El0 {
    use filesystem_contract::blk;

    if shared[510..512] != [0x55, 0xaa] || shared[82..90] != r.fs_type {
        return El0::Wrong("block 0 is the boot sector the kernel read", 0);
    }
    let got = call(blk::WRITE, 0);
    if got != filesystem_contract::reply_err(30) {
        return El0::Wrong("WRITE to a read-only window is EROFS", got);
    }
    El0::Served
}

/// A write build's checks: overwrite the window's one block, read it back, restore the original
/// and read that back, then flush.
fn write_cycle_checks(
    call: &impl Fn(u64, u64) -> i64,
    shared: &mut [u8],
    window: designware_mobile_storage::serve::Window,
) -> El0 {
    use filesystem_contract::blk;

    let mut original = [0u8; blk::BLOCK_SIZE];
    original.copy_from_slice(shared);
    for (i, b) in shared.iter_mut().enumerate() {
        *b = bench::pattern(window.first + (i / BLOCK) as u64, i % BLOCK) ^ 0xff;
    }
    let got = call(blk::WRITE, 0);
    if got != 1 {
        return El0::Wrong("WRITE block 0", got);
    }
    shared.fill(0);
    let got = call(blk::READ, 0);
    let intact = shared
        .iter()
        .enumerate()
        .all(|(i, &b)| b == bench::pattern(window.first + (i / BLOCK) as u64, i % BLOCK) ^ 0xff);
    shared.copy_from_slice(&original);
    let restored = call(blk::WRITE, 0);
    if got != 1 || !intact {
        return El0::Wrong("the written block read back", got);
    }
    if restored != 1 {
        return El0::Wrong("WRITE the original back", restored);
    }
    shared.fill(0);
    let got = call(blk::READ, 0);
    if got != 1 || shared[..] != original[..] {
        return El0::Wrong("the original read back", got);
    }
    let flushed = call(blk::FLUSH, 0);
    if flushed != 1 {
        return El0::Wrong("FLUSH counts", flushed);
    }
    El0::Served
}

fn print_identity(slot: &Slot, id: &Identity, depth: u32) {
    say!(
        slot,
        "VERID {:#010x} HCON {:#010x} USRID {:#x} (FIFO at {:#x}, {:?} bytes wide, {} words deep, 64-bit DMA {}), CDETECT {:#x}",
        id.verid,
        id.hcon,
        id.usrid,
        regs::fifo_offset(id.verid),
        regs::fifo_width(id.hcon),
        depth,
        regs::dma_64bit(id.hcon),
        id.cdetect,
    );
    say!(
        slot,
        "as the firmware left it: CTRL {:#x} PWREN {:#x} CLKDIV {:#x} CLKENA {:#x} CTYPE {:#x}",
        id.firmware[0],
        id.firmware[1],
        id.firmware[2],
        id.firmware[3],
        id.firmware[4],
    );
}

fn print_read(slot: &Slot, r: &ReadReport) {
    let c = &r.card;
    let name = c
        .cid
        .name
        .map(|b| if b.is_ascii_graphic() { b } else { b'.' });
    say!(
        slot,
        "card {:?}, manufacturer {:#04x} \"{}\" serial {:#010x}, {} blocks ({} MiB), {}-addressed, {}-bit at {} Hz, OCR {:#010x}",
        c.kind,
        c.cid.manufacturer,
        core::str::from_utf8(&name).unwrap_or("?"),
        c.cid.serial,
        c.blocks,
        c.blocks * BLOCK as u64 / (1 << 20),
        if c.block_addressed { "block" } else { "byte" },
        c.bus_width,
        c.clock_hz,
        c.ocr,
    );
    if c.kind == Kind::Mmc {
        say!(
            slot,
            "an eMMC answered; this boot reads it and never writes it"
        );
    }
    match r.mbr {
        None => say!(slot, "sector 0 carries no MBR signature"),
        Some(m) => {
            for (i, e) in m.entries.iter().enumerate().filter(|(_, e)| e.is_used()) {
                say!(
                    slot,
                    "partition {}: type {:#04x}, sectors {}..{}",
                    i + 1,
                    e.kind,
                    e.start,
                    u64::from(e.start) + u64::from(e.sectors),
                );
            }
            say!(
                slot,
                "first partition's boot sector: signature {}, type field \"{}\"",
                if r.partition_boot_signature {
                    "55aa"
                } else {
                    "absent"
                },
                core::str::from_utf8(&r.fs_type).unwrap_or("?"),
            );
            say!(
                slot,
                "scratch range a write test would use: {:?}",
                m.scratch(bench::SCRATCH_SECTORS)
            );
        }
    }
    say!(
        slot,
        "timed read: {} blocks in {} us, {} KiB/s ({} blocks per command, polled FIFO)",
        r.timed_blocks,
        r.timed_us,
        kib_per_s(r.timed_blocks, r.timed_us),
        READ_PAGES * 4096 / BLOCK,
    );
}

fn kib_per_s(blocks: u64, us: u64) -> u64 {
    (blocks * BLOCK as u64 * 1_000_000)
        .checked_div(us)
        .unwrap_or(0)
        / 1024
}

/// The same card through the IDMAC: the polled host reads the partition's first 64 KiB as the
/// reference, then `bench::idmac_read` reads those blocks and the same 8 MiB, and a write build
/// runs the scratch write test. One descriptor page and `READ_PAGES` data pages, as the polled
/// read.
fn through_idmac(
    slot: &Slot,
    mut host: Host<Window>,
    id: &Identity,
    r: &ReadReport,
    write: bool,
) -> Dma {
    let first = r.timed_from;
    let (Some(reference), Some(buf)) = (pages(READ_PAGES), pages(READ_PAGES)) else {
        return Dma::NoMemory;
    };
    if let Err(e) = sd::read_blocks(&mut host, &r.card, first, reference) {
        return Dma::Failed(ReadFailure::Read(first, e));
    }
    let Some(region) = Region::allocate(1 + READ_PAGES) else {
        return Dma::NoMemory;
    };
    say!(
        slot,
        "IDMAC region at {:#x}, {} bytes",
        region.bus_address(),
        region.bytes(),
    );
    let idmac = match Idmac::new(region, id) {
        Ok(i) => i,
        Err(e) => return Dma::Refused(e),
    };
    let mut host = host.with_idmac(idmac);
    let report = match bench::idmac_read(&mut host, r, reference, buf, TIMED_BLOCKS) {
        Ok(report) => report,
        Err(e) => {
            let w = host.registers();
            say!(
                slot,
                "IDMAC read failed: {e:?}; IDSTS {:#x} DSCADDR {:#x} BUFADDR {:#x} STATUS {:#x} TCBCNT {}",
                w.read(regs::IDSTS),
                w.read(regs::DSCADDR),
                w.read(regs::BUFADDR),
                w.read(regs::STATUS),
                w.read(regs::TCBCNT),
            );
            return Dma::Failed(e);
        }
    };
    let kib = kib_per_s(report.timed_blocks, report.timed_us);
    say!(
        slot,
        "timed read: {} blocks in {} us, {kib} KiB/s ({} blocks per command, IDMAC); the first {} KiB {} the polled read",
        report.timed_blocks,
        report.timed_us,
        READ_PAGES * 4096 / BLOCK,
        READ_PAGES * 4,
        if report.matches {
            "match"
        } else {
            "DIFFER FROM"
        },
    );
    let verdict = if write {
        let (Some(a), Some(b)) = (pages(1), pages(1)) else {
            return Dma::NoMemory;
        };
        let v = bench::scratch_write(&mut host, &r.card, r.mbr.as_ref(), a, b);
        say!(slot, "scratch write test through the IDMAC: {v:?}");
        Some(v)
    } else {
        None
    };
    Dma::Read(kib, report.matches, verdict)
}

/// `count` zeroed, contiguous pages as a byte slice, leaked: this boot halts.
fn pages(count: usize) -> Option<&'static mut [u8]> {
    let frame = crate::memory::alloc_contiguous_zeroed(count)?;
    let va = crate::arch::mmu::phys_to_virt(frame.addr()) as *mut u8;
    // SAFETY: the allocator handed out `count` contiguous pages that nothing else owns, and the
    // direct map covers all of RAM; nothing frees them, because the boot ends in a halt.
    Some(unsafe { core::slice::from_raw_parts_mut(va, count * 4096) })
}

/// The multi-block read diagnosis: the same `CMD18` at each card clock and length
/// (`bench::read_matrix`), with the tree's FIFO depth and again trusting `STATUS` up to 128 words.
/// Each row says how full the FIFO got, to tell whether a full FIFO is what ends in a data CRC
/// error. Read-only, after the probe, whose clock bring-up it relies on.
#[cfg(feature = "storage_read_matrix")]
fn read_matrix(slot: &Slot) {
    use designware_mobile_storage::bench::Watch;
    let Some(buf) = pages(READ_PAGES) else {
        println!("storage-matrix: no pages for a buffer");
        return;
    };
    let Some(sys) = crate::memory::jh7110_sys_window() else {
        return;
    };
    let ciu_hz = card_clock(slot, sys.base);
    let first = 2048;
    for depth in [slot.fifo_depth, Some(128)] {
        // SAFETY: as `probe`'s window: the same mapped page, and this boot thread alone drives it.
        let window = unsafe { Window::new(slot.base) };
        let Ok((mut host, _)) = Host::new(Watch::new(window), ciu_hz, depth) else {
            println!("storage-matrix: controller refused");
            return;
        };
        let used = host.fifo_depth();
        let done = bench::read_matrix(&mut host, slot.bus_width, first, buf, |r| {
            println!(
                "storage-matrix: depth {used} card {} Hz (asked {}) blocks {}: {:?} in {} us, \
                 FIFO peak {} words, full on {} STATUS reads; after: STATUS {:#x} RINTSTS {:#x} \
                 TCBCNT {} TBBCNT {}",
                r.card_hz,
                r.asked_hz,
                r.blocks,
                r.result,
                r.us,
                r.max_fill,
                r.full_reads,
                r.after[0],
                r.after[1],
                r.after[2],
                r.after[3],
            );
        });
        if let Err(e) = done {
            println!("storage-matrix: depth {used}: identification stopped: {e:?}");
        }
    }
}
