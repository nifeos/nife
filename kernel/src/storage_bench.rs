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


use designware_mobile_storage::host_controller::{
    Error, HostController, Identity, RegisterWindowAndClock,
};
use designware_mobile_storage::internal_direct_memory_access::{
    DirectMemoryAccessRegion, InternalDirectMemoryAccess, Refusal,
};
use designware_mobile_storage::jh7110::Slot;
use designware_mobile_storage::registers;
use designware_mobile_storage::sd::{
    self, BLOCK_BYTES, BlockTransferError, CardKind, InitializationError,
};

use self::bench::{ReadOnlyProbeFailure, ReadOnlyProbeReport, ScratchWriteVerdict};
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
    NoCard(InitializationError),
    /// A read failed: the block and the error.
    ReadFailed(u64, BlockTransferError),
    /// Read-only, and every polled read succeeded: the timed rate in KiB/s, and the IDMAC phase.
    ReadOk(u64, Dma),
    /// Read, then the scratch write test: the rate, the test's verdict, and the IDMAC phase.
    Written(u64, ScratchWriteVerdict, Dma),
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
    Failed(ReadOnlyProbeFailure),
    /// The timed read's rate in KiB/s, whether the first 64 KiB matched the polled read, and in a
    /// write build the scratch write test through the IDMAC.
    Read(u64, bool, Option<ScratchWriteVerdict>),
}

impl Dma {
    /// Whether the IDMAC did everything this build asked of it.
    fn passed(&self) -> bool {
        matches!(
            self,
            Dma::Read(
                _,
                true,
                None | Some(ScratchWriteVerdict::Verified(..) | ScratchWriteVerdict::Refused)
            )
        )
    }
}

impl core::fmt::Display for Dma {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Dma::Read(kib, true, None) => write!(f, "IDMAC READ-OK {kib} KiB/s"),
            Dma::Read(kib, true, Some(ScratchWriteVerdict::Verified(..))) => {
                write!(f, "IDMAC READ-OK {kib} KiB/s, IDMAC WRITE-VERIFIED")
            }
            Dma::Read(kib, true, Some(ScratchWriteVerdict::Refused)) => {
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
        Outcome::Written(kib, ScratchWriteVerdict::Verified(first, count, blank), dma) => println!(
            "storage-bench: verdict {} {kib} KiB/s, WRITE-VERIFIED sectors {first}..{} (were {}), restored, {dma}, EL0 block server served",
            if dma.passed() {
                "READ-OK"
            } else {
                "FAILED: polled READ-OK"
            },
            first + count,
            if *blank { "zero" } else { "not zero" },
        ),
        Outcome::Written(kib, ScratchWriteVerdict::Refused, dma) => println!(
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
            $slot.board_role(),
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
            slot.card_interface_unit_hertz
        );
        return slot.card_interface_unit_hertz;
    }
    // SAFETY: `mmu::initialize` mapped the SYS CRG (step 6c) and, because the tree names the PMIC's
    // bus, the SYS syscon (step 6c-prime); the slot index is 0 or 1 by `jh7110_storage`.
    let c = unsafe {
        crate::drivers::jh7110_clock_and_reset::set_card_clock(
            crate::arch::mmu::phys_to_virt(crg_phys) as usize,
            crate::arch::mmu::phys_to_virt(
                jh7110_clock_and_reset::SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS,
            ) as usize,
            usize::from(slot.index),
            slot.card_interface_unit_hertz,
        )
    };
    say!(
        slot,
        "card clock: {:#010x} -> {:#010x}, {} Hz from a {} Hz parent (the tree asks {} Hz)",
        c.before,
        c.after,
        c.hz,
        c.parent_hz,
        slot.card_interface_unit_hertz
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
        slot.queue_depth,
        slot.card_interface_unit_hertz,
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
    // SAFETY: `memory::jh7110_sys_window` answers only on a JH7110, and `mmu::initialize` step 6c
    // mapped exactly this window device-typed. The plan and the domain are the same crate's, so
    // every identifier is in range; every step is idempotent on a controller already up.
    let report = unsafe {
        crate::drivers::jh7110_clock_and_reset::bring_up(
            crate::arch::mmu::phys_to_virt(sys.base) as usize,
            &jh7110_clock_and_reset::SYSTEM,
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

    // SAFETY: step 10 of `mmu::initialize` mapped this slot's first page from the same
    // `memory::jh7110_storage` answer, and this boot thread is the only thing that drives it.
    let window = unsafe { Window::new(slot.base) };
    let (mut host, id) = match HostController::new(window, ciu_hz, slot.queue_depth) {
        Ok(h) => h,
        Err(e) => {
            say!(slot, "controller refused: {e:?}");
            return Outcome::Controller(e);
        }
    };
    print_identity(slot, &id, host.queue_depth());

    let Some(buf) = pages(READ_PAGES) else {
        return Outcome::NoMemory;
    };
    let r = match bench::run_read_only_probe(&mut host, slot.bus_width, buf, TIMED_BLOCKS) {
        Err(ReadOnlyProbeFailure::Initialization(e)) => {
            say!(slot, "identification stopped: {e:?}");
            return Outcome::NoCard(e);
        }
        Err(ReadOnlyProbeFailure::Read(lba, e)) => {
            say!(slot, "read of block {lba} failed: {e:?}");
            // The card byte count counts bytes card to FIFO and the host byte count FIFO to CPU:
            // the CPU ahead of the card is an underrun, the card ahead with a full FIFO is an
            // overrun.
            let r = host.registers_mut();
            let (status, tcb, tbb) = (
                r.read(registers::STATUS_REGISTER),
                r.read(registers::TRANSFERRED_CARD_BYTE_COUNT),
                r.read(registers::TRANSFERRED_HOST_BYTE_COUNT),
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
        if el0 != El0::Served
            && !matches!(
                outcome,
                Outcome::Written(_, ScratchWriteVerdict::Refused, _)
            )
        {
            return Outcome::El0(el0);
        }
    }
    outcome
}

/// **The probe's last kernel steps**: in a write build the scratch write test, polled, then the
/// IDMAC phase, which takes the host. The outcome carries both.
fn write_and_idmac(
    slot: &Slot,
    mut host: HostController<Window>,
    id: &Identity,
    r: &ReadOnlyProbeReport,
    write: bool,
) -> Outcome {
    let verdict = if write {
        let (Some(a), Some(b)) = (pages(1), pages(1)) else {
            return Outcome::NoMemory;
        };
        let v =
            bench::run_scratch_write_test(&mut host, &r.card, r.master_boot_record.as_ref(), a, b);
        say!(slot, "scratch write test: {v:?}");
        Some(v)
    } else {
        None
    };
    let dma = through_idmac(slot, host, id, r, write);
    say!(slot, "{dma}");
    let kib = kib_per_second(r.timed_blocks, r.timed_microseconds);
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
fn through_el0(slot: &Slot, r: &ReadOnlyProbeReport, write: bool) -> El0 {
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
    let Some(mbr) = r.master_boot_record else {
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
    if call(blk::SIZE, 0) != (window.sectors * BLOCK_BYTES as u64) as i64 {
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
    mbr: &designware_mobile_storage::partition::MasterBootRecord,
    write: bool,
) -> Option<driver_process_contract::designware_mobile_storage::Window> {
    use driver_process_contract::designware_mobile_storage::Window;

    let window = if write {
        match mbr.scratch_sector_range(bench::MAXIMUM_SCRATCH_SECTORS) {
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
fn read_only_checks(
    call: &impl Fn(u64, u64) -> i64,
    shared: &[u8],
    r: &ReadOnlyProbeReport,
) -> El0 {
    use filesystem_contract::blk;

    if shared[510..512] != [0x55, 0xaa] || shared[82..90] != r.file_system_type {
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
    window: driver_process_contract::designware_mobile_storage::Window,
) -> El0 {
    use filesystem_contract::blk;

    let mut original = [0u8; blk::BLOCK_SIZE];
    original.copy_from_slice(shared);
    for (i, b) in shared.iter_mut().enumerate() {
        *b = bench::scratch_pattern_byte(window.first + (i / BLOCK_BYTES) as u64, i % BLOCK_BYTES)
            ^ 0xff;
    }
    let got = call(blk::WRITE, 0);
    if got != 1 {
        return El0::Wrong("WRITE block 0", got);
    }
    shared.fill(0);
    let got = call(blk::READ, 0);
    let intact = shared.iter().enumerate().all(|(i, &b)| {
        b == bench::scratch_pattern_byte(window.first + (i / BLOCK_BYTES) as u64, i % BLOCK_BYTES)
            ^ 0xff
    });
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
        id.version_identification,
        id.hardware_configuration,
        id.user_identification,
        registers::queue_offset(id.version_identification),
        registers::queue_width(id.hardware_configuration),
        depth,
        registers::uses_64_bit_direct_memory_access(id.hardware_configuration),
        id.card_detect,
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

fn print_read(slot: &Slot, r: &ReadOnlyProbeReport) {
    let c = &r.card;
    let name = c
        .card_identification
        .name
        .map(|b| if b.is_ascii_graphic() { b } else { b'.' });
    say!(
        slot,
        "card {:?}, manufacturer {:#04x} \"{}\" serial {:#010x}, {} blocks ({} MiB), {}-addressed, {}-bit at {} Hz, OCR {:#010x}",
        c.kind,
        c.card_identification.manufacturer,
        core::str::from_utf8(&name).unwrap_or("?"),
        c.card_identification.serial,
        c.blocks,
        c.blocks * BLOCK_BYTES as u64 / (1 << 20),
        if c.block_addressed { "block" } else { "byte" },
        c.bus_width,
        c.clock_hertz,
        c.operation_conditions,
    );
    if c.kind == CardKind::MultimediaCard {
        say!(
            slot,
            "an eMMC answered; this boot reads it and never writes it"
        );
    }
    match r.master_boot_record {
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
                core::str::from_utf8(&r.file_system_type).unwrap_or("?"),
            );
            say!(
                slot,
                "scratch range a write test would use: {:?}",
                m.scratch_sector_range(bench::MAXIMUM_SCRATCH_SECTORS)
            );
        }
    }
    say!(
        slot,
        "timed read: {} blocks in {} us, {} KiB/s ({} blocks per command, polled FIFO)",
        r.timed_blocks,
        r.timed_microseconds,
        kib_per_second(r.timed_blocks, r.timed_microseconds),
        READ_PAGES * 4096 / BLOCK_BYTES,
    );
}

fn kib_per_second(blocks: u64, microseconds: u64) -> u64 {
    (blocks * BLOCK_BYTES as u64 * 1_000_000)
        .checked_div(microseconds)
        .unwrap_or(0)
        / 1024
}

/// The same card through the IDMAC: the polled host reads the partition's first 64 KiB as the
/// reference, then `bench::internal_direct_memory_access_read` reads those blocks and the same 8
/// MiB, and a write build runs the scratch write test. One descriptor page and `READ_PAGES` data
/// pages, as the polled read.
fn through_idmac(
    slot: &Slot,
    mut host: HostController<Window>,
    id: &Identity,
    r: &ReadOnlyProbeReport,
    write: bool,
) -> Dma {
    let first = r.timed_from;
    let (Some(reference), Some(buf)) = (pages(READ_PAGES), pages(READ_PAGES)) else {
        return Dma::NoMemory;
    };
    if let Err(e) = sd::read_blocks(&mut host, &r.card, first, reference) {
        return Dma::Failed(ReadOnlyProbeFailure::Read(first, e));
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
    let idmac = match InternalDirectMemoryAccess::new(region, id) {
        Ok(i) => i,
        Err(e) => return Dma::Refused(e),
    };
    let mut host = host.with_internal_direct_memory_access(idmac);
    let report = match bench::internal_direct_memory_access_read(
        &mut host,
        r,
        reference,
        buf,
        TIMED_BLOCKS,
    ) {
        Ok(report) => report,
        Err(e) => {
            let w = host.registers_mut();
            say!(
                slot,
                "IDMAC read failed: {e:?}; IDSTS {:#x} DSCADDR {:#x} BUFADDR {:#x} STATUS {:#x} TCBCNT {}",
                w.read(registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER),
                w.read(registers::CURRENT_DESCRIPTOR_ADDRESS_REGISTER),
                w.read(registers::CURRENT_BUFFER_ADDRESS_REGISTER),
                w.read(registers::STATUS_REGISTER),
                w.read(registers::TRANSFERRED_CARD_BYTE_COUNT),
            );
            return Dma::Failed(e);
        }
    };
    let kib = kib_per_second(report.timed_blocks, report.timed_microseconds);
    say!(
        slot,
        "timed read: {} blocks in {} us, {kib} KiB/s ({} blocks per command, IDMAC); the first {} KiB {} the polled read",
        report.timed_blocks,
        report.timed_microseconds,
        READ_PAGES * 4096 / BLOCK_BYTES,
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
        let v =
            bench::run_scratch_write_test(&mut host, &r.card, r.master_boot_record.as_ref(), a, b);
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
    use self::bench::QueueWatch;
    let Some(buf) = pages(READ_PAGES) else {
        println!("storage-matrix: no pages for a buffer");
        return;
    };
    let Some(sys) = crate::memory::jh7110_sys_window() else {
        return;
    };
    let ciu_hz = card_clock(slot, sys.base);
    let first = 2048;
    for depth in [slot.queue_depth, Some(128)] {
        // SAFETY: as `probe`'s window: the same mapped page, and this boot thread alone drives it.
        let window = unsafe { Window::new(slot.base) };
        let Ok((mut host, _)) = HostController::new(QueueWatch::new(window), ciu_hz, depth) else {
            println!("storage-matrix: controller refused");
            return;
        };
        let used = host.queue_depth();
        let done = bench::read_matrix(&mut host, slot.bus_width, first, buf, |r| {
            println!(
                "storage-matrix: depth {used} card {} Hz (asked {}) blocks {}: {:?} in {} us, \
                 FIFO peak {} words, full on {} STATUS reads; after: STATUS {:#x} RINTSTS {:#x} \
                 TCBCNT {} TBBCNT {}",
                r.card_hertz,
                r.asked_hertz,
                r.blocks,
                r.result,
                r.microseconds,
                r.maximum_fill,
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

mod bench {
    //! The bench step's two halves, as logic: the read-only probe and the scratch write test that a
    //! kernel's storage bench runs on a VisionFive 2, here so their order of operations, which is the
    //! safety argument, is tested on a host rather than first tried on the card the VisionFive 2 boots from.
    //! [`run_read_only_probe`] identifies the card, reads sector 0, parses the MBR, reads the first partition's
    //! boot sector and times a sequential read from its start; it sends no command that writes.
    //! [`run_scratch_write_test`] writes only inside `MasterBootRecord::scratch_sector_range`'s range, after reading what
    //! is there: a pattern, read back, then the original, read back too. A card with no gap is refused,
    //! not written.
    //!
    //! Name: ratified 2026-11-03 (UTC), #3135. `bench` is the word for the physical test bench: the
    //! step run on the real board, not on a host.

    use designware_mobile_storage::host_controller::{HostController, RegisterWindowAndClock};
    use designware_mobile_storage::internal_direct_memory_access::DirectMemoryAccessRegion;
    use designware_mobile_storage::partition::MasterBootRecord;
    #[cfg(feature = "storage_read_matrix")]
    use designware_mobile_storage::registers::{
        STATUS_REGISTER, TRANSFERRED_CARD_BYTE_COUNT, TRANSFERRED_HOST_BYTE_COUNT,
    };
    use designware_mobile_storage::sd::{
        self, BLOCK_BYTES, BlockTransferError, Card, InitializationError,
    };

    #[cfg(feature = "storage_read_matrix")]
    /// **Raw Interrupt Status Register** (OpenBSD's `SDMMC_RINTSTS`): write one to clear. The matrix
    /// reads it straight after each read, for its transcript.
    const RAW_INTERRUPT_STATUS_REGISTER: u32 = 0x044;

    #[cfg(feature = "storage_read_matrix")]
    /// **FIFO full** (`STATUS_FIFO_FULL`, the databook's `fifo_full`, which `dwmmc-host` keeps): the
    /// FIFO is full.
    const STATUS_QUEUE_FULL: u32 = 1 << 3;

    #[cfg(feature = "storage_read_matrix")]
    /// **FIFO count** (OpenBSD's `SDMMC_STATUS_FIFO_COUNT`, the databook's `fifo_count`): how many FIFO
    /// words are filled, bits 29:17 of `STATUS_REGISTER`.
    const fn queue_count(status: u32) -> u32 {
        (status >> 17) & 0x1fff
    }

    /// The first used partition's start, or `None` when nothing is partitioned.
    fn first_partition_start_of(master_boot_record: MasterBootRecord) -> Option<u32> {
        master_boot_record
            .entries
            .iter()
            .filter(|e| e.is_used())
            .map(|e| e.start)
            .min()
    }

    /// How many sectors the write test uses at most: 4 KiB, one multi-block command each way.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3141. The most sectors the write test uses; the maximum is spelled out. Renamed from `SCRATCH_SECTORS`.
    pub const MAXIMUM_SCRATCH_SECTORS: u32 = 8;

    /// What the read-only probe found.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3141. What the read-only probe found, named for the probe that returns it. Renamed from `ReadReport`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReadOnlyProbeReport {
        /// The card, identified.
        pub card: Card,
        /// Sector 0, parsed, if it holds an MBR.
        #[doc(alias = "mbr")]
        pub master_boot_record: Option<MasterBootRecord>,
        /// The first partition's first sector ends in `55 aa` (a FAT boot sector does).
        pub partition_boot_signature: bool,
        /// The first partition's first sector's bytes 82..90 (FAT32's `"FAT32   "` file system
        /// type), for the transcript.
        pub file_system_type: [u8; 8],
        /// The block the timed read started at: the first partition's, or 0 without one.
        pub timed_from: u64,
        /// Blocks read in the timed sequential read.
        pub timed_blocks: u64,
        /// Microseconds the timed read took.
        pub timed_microseconds: u64,
    }

    /// Where the read-only probe stopped.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3141. Where the read-only probe stopped, named for the probe. Renamed from `ReadFailure`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ReadOnlyProbeFailure {
        /// Identification failed.
        Initialization(InitializationError),
        /// A read failed: the block and the error.
        Read(u64, BlockTransferError),
    }

    impl core::fmt::Display for ReadOnlyProbeFailure {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Self::Initialization(e) => write!(f, "identification failed: {e}"),
                Self::Read(block, e) => write!(f, "reading block {block} failed: {e}"),
            }
        }
    }

    impl core::error::Error for ReadOnlyProbeFailure {}

    /// **The read-only probe.** `buffer` is the transfer buffer (a whole number of blocks; the timed
    /// read moves `buffer.len()` bytes per command), `timed_blocks` how many blocks to time.
    ///
    /// # Errors
    ///
    /// The first step that failed.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3141. The function that runs the read-only probe; a verb phrase says that it acts on the card. Renamed from `read_only`.
    pub fn run_read_only_probe<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
        host: &mut HostController<R, D>,
        bus_width: u8,
        buffer: &mut [u8],
        timed_blocks: u64,
    ) -> Result<ReadOnlyProbeReport, ReadOnlyProbeFailure> {
        let card = sd::identify(host, bus_width).map_err(ReadOnlyProbeFailure::Initialization)?;
        let mut sector = [0u8; BLOCK_BYTES];
        sd::read_blocks(host, &card, 0, &mut sector)
            .map_err(|e| ReadOnlyProbeFailure::Read(0, e))?;
        let master_boot_record = MasterBootRecord::parse(&sector);
        let first = master_boot_record
            .and_then(first_partition_start_of)
            .map_or(0, u64::from);
        let mut partition_boot_signature = false;
        let mut file_system_type = [0u8; 8];
        if first != 0 {
            sd::read_blocks(host, &card, first, &mut sector)
                .map_err(|e| ReadOnlyProbeFailure::Read(first, e))?;
            partition_boot_signature = sector[510..512] == [0x55, 0xaa];
            file_system_type.copy_from_slice(&sector[82..90]);
        }
        let (timed, timed_microseconds) = timed_read(host, &card, first, buffer, timed_blocks)?;
        Ok(ReadOnlyProbeReport {
            card,
            master_boot_record,
            partition_boot_signature,
            file_system_type,
            timed_from: first,
            timed_blocks: timed,
            timed_microseconds,
        })
    }

    /// **The timed sequential read**: up to `timed_blocks` blocks from `first`, `buffer.len()` bytes a
    /// command, stopping before the card's end. Returns the blocks read and the microseconds taken.
    fn timed_read<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
        host: &mut HostController<R, D>,
        card: &Card,
        first: u64,
        buffer: &mut [u8],
        timed_blocks: u64,
    ) -> Result<(u64, u64), ReadOnlyProbeFailure> {
        let per = (buffer.len() / BLOCK_BYTES) as u64;
        let mut timed = 0u64;
        let start = host.registers_mut().now_microseconds();
        if per > 0 {
            while timed < timed_blocks && first + timed + per <= card.blocks {
                sd::read_blocks(host, card, first + timed, buffer)
                    .map_err(|e| ReadOnlyProbeFailure::Read(first + timed, e))?;
                timed += per;
            }
        }
        Ok((
            timed,
            host.registers_mut().now_microseconds().wrapping_sub(start),
        ))
    }

    /// What the IDMAC's half of the read-only probe found.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. `idmac` and the names built on it spell out
    /// `internal_direct_memory_access`; the databook's `idmac` is kept as `doc(alias = "idmac")`.
    #[doc(alias = "idmac")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InternalDirectMemoryAccessReport {
        /// The first transfer's bytes are the polled host's for the same blocks.
        pub matches: bool,
        /// Blocks read in the timed sequential read.
        pub timed_blocks: u64,
        /// Microseconds the timed read took.
        pub timed_microseconds: u64,
    }

    /// **The IDMAC's half of the read-only probe**, on a host
    /// `HostController::with_internal_direct_memory_access` made from the one [`run_read_only_probe`] ran on and reported
    /// `report`. `reference` is the polled host's read of `reference.len()` bytes from
    /// `report.timed_from`; the same blocks are read through the IDMAC and compared, then
    /// [`run_read_only_probe`]'s timed read runs again from there in `buffer.len()`-byte commands, so the two
    /// rates differ only in the data path.
    ///
    /// # Errors
    ///
    /// The read that failed.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. `idmac` and the names built on it spell out
    /// `internal_direct_memory_access`; the databook's `idmac` is kept as `doc(alias = "idmac")`.
    #[doc(alias = "idmac")]
    pub fn internal_direct_memory_access_read<
        R: RegisterWindowAndClock,
        D: DirectMemoryAccessRegion,
    >(
        host: &mut HostController<R, D>,
        report: &ReadOnlyProbeReport,
        reference: &[u8],
        buffer: &mut [u8],
        timed_blocks: u64,
    ) -> Result<InternalDirectMemoryAccessReport, ReadOnlyProbeFailure> {
        let (card, first) = (&report.card, report.timed_from);
        let n = reference.len().min(buffer.len());
        sd::read_blocks(host, card, first, &mut buffer[..n])
            .map_err(|e| ReadOnlyProbeFailure::Read(first, e))?;
        // An empty reference never gets here: a read of no blocks is refused as `Length`.
        let matches = buffer[..n] == reference[..n];
        let (timed_blocks, timed_microseconds) =
            timed_read(host, card, first, buffer, timed_blocks)?;
        Ok(InternalDirectMemoryAccessReport {
            matches,
            timed_blocks,
            timed_microseconds,
        })
    }

    /// The byte the write test puts at offset `i` of the sector at `logical_block_address`: never
    /// constant across a sector, different for each sector, so a write that lands one sector off or
    /// repeats a block is seen on readback.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3141. The byte of the pattern the write test puts in a scratch sector. Renamed from `pattern`.
    #[must_use]
    pub const fn scratch_pattern_byte(logical_block_address: u64, i: usize) -> u8 {
        (i as u8).wrapping_mul(31)
            ^ (logical_block_address as u8).wrapping_mul(7)
            ^ 0x5a
            ^ (i >> 8) as u8
    }

    /// How the scratch write test ended.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3141. The verdict of the scratch write test, named for the test. Renamed from `WriteVerdict`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ScratchWriteVerdict {
        /// No MBR, a GPT disk, or no gap before the first partition: nothing was written.
        Refused,
        /// The pattern was written, read back intact, and the original contents written back and read
        /// back intact: `(first sector, count, the original was all zero)`.
        Verified(u32, u32, bool),
        /// The pattern did not read back: the first wrong sector. The original was written back.
        PatternMismatch(u64),
        /// The original did not read back after it was restored: the first wrong sector. This is the
        /// one verdict that leaves the scratch range changed, and it is inside the gap no partition
        /// owns.
        RestoreMismatch(u64),
        /// A command failed: which phase (0 read original, 1 write pattern, 2 read pattern, 3 write
        /// original, 4 read original back) and the error.
        Failed(u8, BlockTransferError),
    }

    /// **The scratch write test.** `master_boot_record` is what [`run_read_only_probe`] parsed from this card;
    /// `original` and `scratch` are two buffers of at least [`MAXIMUM_SCRATCH_SECTORS`] blocks.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3141. The function that runs the scratch write test; a verb phrase says that it acts on the card. Renamed from `scratch_write`.
    pub fn run_scratch_write_test<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
        host: &mut HostController<R, D>,
        card: &Card,
        master_boot_record: Option<&MasterBootRecord>,
        original: &mut [u8],
        scratch: &mut [u8],
    ) -> ScratchWriteVerdict {
        let Some((first, count)) =
            master_boot_record.and_then(|m| m.scratch_sector_range(MAXIMUM_SCRATCH_SECTORS))
        else {
            return ScratchWriteVerdict::Refused;
        };
        let bytes = count as usize * BLOCK_BYTES;
        if original.len() < bytes || scratch.len() < bytes {
            return ScratchWriteVerdict::Failed(0, BlockTransferError::Length);
        }
        let logical_block_address = u64::from(first);
        let (original, scratch) = (&mut original[..bytes], &mut scratch[..bytes]);
        if let Err(e) = sd::read_blocks(host, card, logical_block_address, original) {
            return ScratchWriteVerdict::Failed(0, e);
        }
        let blank = original.iter().all(|&b| b == 0);
        for (s, block) in scratch
            .as_chunks_mut::<BLOCK_BYTES>()
            .0
            .iter_mut()
            .enumerate()
        {
            for (i, b) in block.iter_mut().enumerate() {
                *b = scratch_pattern_byte(logical_block_address + s as u64, i);
            }
        }
        let mut verdict = None;
        if let Err(e) = sd::write_blocks(host, card, logical_block_address, scratch) {
            verdict = Some(ScratchWriteVerdict::Failed(1, e));
        } else {
            let mut back = [0u8; BLOCK_BYTES];
            for s in 0..u64::from(count) {
                if let Err(e) = sd::read_blocks(host, card, logical_block_address + s, &mut back) {
                    verdict = Some(ScratchWriteVerdict::Failed(2, e));
                    break;
                }
                let at = s as usize * BLOCK_BYTES;
                if back[..] != scratch[at..at + BLOCK_BYTES] {
                    verdict = Some(ScratchWriteVerdict::PatternMismatch(
                        logical_block_address + s,
                    ));
                    break;
                }
            }
        }
        // Whatever happened, put back what was there.
        if let Err(e) = sd::write_blocks(host, card, logical_block_address, original) {
            return ScratchWriteVerdict::Failed(3, e);
        }
        if let Err(e) = sd::read_blocks(host, card, logical_block_address, scratch) {
            return ScratchWriteVerdict::Failed(4, e);
        }
        if let Some(s) = scratch
            .as_chunks::<BLOCK_BYTES>()
            .0
            .iter()
            .zip(original.as_chunks::<BLOCK_BYTES>().0)
            .position(|(a, b)| a != b)
        {
            return ScratchWriteVerdict::RestoreMismatch(logical_block_address + s as u64);
        }
        verdict.unwrap_or(ScratchWriteVerdict::Verified(first, count, blank))
    }

    #[cfg(feature = "storage_read_matrix")]
    /// **A register window that remembers how full the queue got**: every `STATUS_REGISTER` read updates the
    /// highest fill seen and counts the reads that found the queue full. It answers the multi-block
    /// read diagnosis: whether the polled CPU falls behind the card.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. `fifo` becomes `queue` in every name this tree authors;
    /// the databook's `fifo` is kept as `doc(alias = "fifo")`.
    #[doc(alias = "fifo")]
    pub struct QueueWatch<R: RegisterWindowAndClock> {
        inner: R,
        /// The highest `STATUS_REGISTER` queue count seen since the last [`QueueWatch::clear`].
        pub maximum_fill: u32,
        /// `STATUS_REGISTER` reads that found the FIFO-full flag set since the last [`QueueWatch::clear`].
        pub full_reads: u32,
    }

    #[cfg(feature = "storage_read_matrix")]
    impl<R: RegisterWindowAndClock> QueueWatch<R> {
        /// Watch the queue of `inner`.
        ///
        /// Name: ratified 2026-11-02 (UTC), #3092. The constructor of the queue watch.
        pub const fn new(inner: R) -> Self {
            Self {
                inner,
                maximum_fill: 0,
                full_reads: 0,
            }
        }

        /// Start counting again.
        ///
        /// Name: ratified 2026-11-02 (UTC), #3092. The method that resets both counters.
        pub fn clear(&mut self) {
            self.maximum_fill = 0;
            self.full_reads = 0;
        }
    }

    #[cfg(feature = "storage_read_matrix")]
    impl<R: RegisterWindowAndClock> RegisterWindowAndClock for QueueWatch<R> {
        fn read(&mut self, offset: u32) -> u32 {
            let v = self.inner.read(offset);
            if offset == STATUS_REGISTER {
                self.maximum_fill = self.maximum_fill.max(queue_count(v));
                if v & STATUS_QUEUE_FULL != 0 {
                    self.full_reads += 1;
                }
            }
            v
        }
        fn write(&mut self, offset: u32, value: u32) {
            self.inner.write(offset, value);
        }
        fn now_microseconds(&mut self) -> u64 {
            self.inner.now_microseconds()
        }
    }

    #[cfg(feature = "storage_read_matrix")]
    /// One multi-block read of the matrix: its card clock, length, outcome and what the queue did.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. One row of the matrix of multi-block reads.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MatrixRow {
        /// The card clock asked for, in hertz.
        ///
        /// Name: ratified 2026-11-05 (UTC), #3276. The unit is spelled out.
        pub asked_hertz: u32,
        /// The card clock the divider gave, in hertz.
        ///
        /// Name: ratified 2026-11-05 (UTC), #3276. The unit is spelled out.
        pub card_hertz: u32,
        /// Blocks in the one `CMD18`.
        pub blocks: u32,
        /// What the read returned.
        pub result: Result<(), BlockTransferError>,
        /// Microseconds the read took.
        pub microseconds: u64,
        /// The highest queue fill seen during it, in words.
        pub maximum_fill: u32,
        /// `STATUS_REGISTER` reads that found the queue full.
        pub full_reads: u32,
        /// `STATUS_REGISTER`, `RINTSTS`, `TRANSFERRED_CARD_BYTE_COUNT` and `TRANSFERRED_HOST_BYTE_COUNT` read straight after it.
        pub after: [u32; 4],
    }

    #[cfg(feature = "storage_read_matrix")]
    /// The card clocks the matrix reads at: the transfer rate and two halvings of it.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3276. The card clocks of the matrix; the unit is spelled out as
    /// `HERTZ`.
    pub const MATRIX_HERTZ: [u32; 3] = [
        sd::TRANSFER_HERTZ,
        sd::TRANSFER_HERTZ / 2,
        sd::TRANSFER_HERTZ / 4,
    ];
    #[cfg(feature = "storage_read_matrix")]
    /// The `CMD18` lengths the matrix reads, in blocks.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. The `CMD18` lengths of the matrix, in blocks.
    pub const MATRIX_BLOCKS: [u32; 4] = [2, 8, 32, 128];

    #[cfg(feature = "storage_read_matrix")]
    /// **Read the same blocks at each clock and length, re-identifying the card before every read**,
    /// so a read that leaves the card mid-transfer cannot fail the next one. Read-only: `CMD18` from
    /// `logical_block_address` and identification, nothing else. `buffer` holds at least the longest
    /// read.
    ///
    /// # Errors
    ///
    /// Identification failing, which ends the matrix; a failed read is a row, not an error.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. The function that reads the whole matrix.
    pub fn read_matrix<R: RegisterWindowAndClock>(
        host: &mut HostController<QueueWatch<R>>,
        bus_width: u8,
        logical_block_address: u64,
        buffer: &mut [u8],
        mut row: impl FnMut(MatrixRow),
    ) -> Result<(), InitializationError> {
        for &hertz in &MATRIX_HERTZ {
            for &blocks in &MATRIX_BLOCKS {
                let card = sd::identify(host, bus_width)?;
                let card_hertz = host
                    .set_clock(hertz)
                    .map_err(InitializationError::Controller)?;
                let len = blocks as usize * BLOCK_BYTES;
                let Some(dst) = buffer.get_mut(..len) else {
                    continue;
                };
                host.registers_mut().clear();
                let start = host.registers_mut().now_microseconds();
                let result = sd::read_blocks(host, &card, logical_block_address, dst);
                let microseconds = host.registers_mut().now_microseconds().wrapping_sub(start);
                let watch = host.registers_mut();
                let after = [
                    watch.inner.read(STATUS_REGISTER),
                    watch.inner.read(RAW_INTERRUPT_STATUS_REGISTER),
                    watch.inner.read(TRANSFERRED_CARD_BYTE_COUNT),
                    watch.inner.read(TRANSFERRED_HOST_BYTE_COUNT),
                ];
                row(MatrixRow {
                    asked_hertz: hertz,
                    card_hertz,
                    blocks,
                    result,
                    microseconds,
                    maximum_fill: watch.maximum_fill,
                    full_reads: watch.full_reads,
                    after,
                });
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[cfg(feature = "storage_read_matrix")]
        /// `QueueWatch` is what says whether the VisionFive 2's queue filled: its peak and full count must come from
        /// `STATUS_REGISTER` alone, count every full read, and start again at `clear`.
        #[test_case]
        fn the_watch_counts_status_fill_and_nothing_else() {
            /// `STATUS_REGISTER` reads `status`; every other register reads all ones.
            struct Fixed {
                status: u32,
            }
            impl RegisterWindowAndClock for Fixed {
                fn read(&mut self, o: u32) -> u32 {
                    if o == STATUS_REGISTER {
                        self.status
                    } else {
                        u32::MAX
                    }
                }
                fn write(&mut self, _: u32, _: u32) {}
                fn now_microseconds(&mut self) -> u64 {
                    0
                }
            }
            // FIFO count in bits 29:17, the full flag in bit 3.
            let mut w = QueueWatch::new(Fixed { status: 5 << 17 });
            w.read(RAW_INTERRUPT_STATUS_REGISTER);
            assert_eq!((w.maximum_fill, w.full_reads), (0, 0));
            w.read(STATUS_REGISTER);
            assert_eq!((w.maximum_fill, w.full_reads), (5, 0));
            let mut w = QueueWatch::new(Fixed {
                status: (128 << 17) | STATUS_QUEUE_FULL,
            });
            w.read(STATUS_REGISTER);
            w.read(STATUS_REGISTER);
            assert_eq!((w.maximum_fill, w.full_reads), (128, 2));
            w.clear();
            assert_eq!((w.maximum_fill, w.full_reads), (0, 0));
        }

        /// The pattern, worked by hand for a few bytes:
        /// `i * 31 ^ logical_block_address * 7 ^ 0x5a ^ i >> 8`, each in eight bits. The `i >> 8` term is
        /// what makes a sector's two halves differ.
        #[test_case]
        fn the_write_pattern_is_the_formula_and_its_halves_differ() {
            assert_eq!(scratch_pattern_byte(0, 0), 0x5a);
            assert_eq!(scratch_pattern_byte(0, 1), 31 ^ 0x5a);
            assert_eq!(scratch_pattern_byte(1, 0), 7 ^ 0x5a);
            assert_eq!(scratch_pattern_byte(3, 5), 155 ^ 21 ^ 0x5a);
            assert_eq!(scratch_pattern_byte(0, 256), 0x5a ^ 1);
            assert!(
                (0..256)
                    .map(|i| scratch_pattern_byte(9, i))
                    .ne((256..512).map(|i| scratch_pattern_byte(9, i)))
            );
        }

        /// The failure prints a sentence naming the block.
        #[test_case]
        fn the_read_failure_names_its_block() {
            use core::fmt::Write;
            struct Text([u8; 96], usize);
            impl Write for Text {
                fn write_str(&mut self, s: &str) -> core::fmt::Result {
                    self.0[self.1..self.1 + s.len()].copy_from_slice(s.as_bytes());
                    self.1 += s.len();
                    Ok(())
                }
            }
            let mut t = Text([0; 96], 0);
            write!(
                t,
                "{}",
                ReadOnlyProbeFailure::Read(7, BlockTransferError::Length)
            )
            .unwrap();
            assert_eq!(
                &t.0[..t.1],
                b"reading block 7 failed: the buffer is empty or not a whole number of blocks"
            );
        }
    }
}
