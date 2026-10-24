//! Ask the RISC-V machine what it is, once, at boot, and refuse one this kernel cannot run on.
//!
//! The decoding is host-tested in [`machine_discovery::riscv64`]. Here are the reads only a boot
//! can make: the device tree, SBI's account of itself, the T-Head vendor probe, and (from
//! `mmu::init`) the measured `satp.ASID` width. [`init`] runs after `memory::init` and before
//! `mmu::init`.
//!
//! See notes/riscv-port/firmware.md.

use machine_discovery::riscv64::{EID_BASE, Isa, SBI_TABLE, Sbi, SbiExtensions};

use crate::sync::{IrqSafeMutex, rank};
use crate::{print, println};

/// The record, written once by [`init`]. `None` until then, so an early read panics rather than
/// returning a plausible all-zero machine.
static ISA: IrqSafeMutex<Option<Isa>> = IrqSafeMutex::new(rank::ISA, None);

/// Discover the machine from the device tree at physical `dtb_ptr` and from SBI, and record it.
///
/// # Panics
///
/// When the machine lacks something the kernel needs, after printing what. A tree that does not
/// describe its ISA, or firmware too old to answer, is reported as unknown and the boot continues.
pub fn init(dtb_ptr: usize) {
    // SAFETY: the same pointer `memory::init` has already validated the magic of, named through the
    // boot table's direct map because we are running virtual.
    let dt = unsafe {
        device_tree_blob::DeviceTreeBlob::from_ptr(
            super::mmu::phys_to_virt(dtb_ptr as u64) as *const u8
        )
    }
    .expect("device tree is unreadable");

    let mut cpu = Isa::from_device_tree(&dt).expect("cannot read the CPU nodes");
    cpu.sbi = probe_sbi();

    let missing = cpu.missing_requirements();
    if missing.any() {
        println!();
        println!("nife cannot run on this machine:");
        if missing.base {
            println!(
                "  base ISA    : the device tree declares {}, not rv64i",
                cpu.base.name()
            );
        }
        for row in &machine_discovery::riscv64::TABLE {
            if missing.extensions.contains(row.bit) {
                println!("  extension   : {} is absent ({})", row.name, row.why);
            }
        }
        if missing.mmu {
            println!(
                "  mmu         : {} cannot hold the kernel's Sv39 high half",
                cpu.mmu.name()
            );
        }
        for row in &SBI_TABLE {
            if missing.sbi.contains(row.bit) {
                println!("  sbi         : {} is absent ({})", row.name, row.why);
            }
        }
        panic!("required hardware or firmware facility is absent");
    }

    *ISA.lock() = Some(cpu);

    probe_thead();
}

/// T-Head's JEDEC vendor id, as `mvendorid` reports it (Linux's `THEAD_VENDOR_ID`).
const THEAD_VENDOR_ID: usize = 0x5b7;
/// `th.sxstatus.MAEE`: the hart reads memory types from PTE bits 63:59.
const TH_SXSTATUS_MAEE: u64 = 1 << 21;

/// What [`probe_thead`] found: `0` before it ran, then [`NOT_THEAD`], [`THEAD`] or
/// [`THEAD_MAE`]. A byte, not a lock, because `fp::init` reads it on every hart.
static VENDOR: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
const NOT_THEAD: u8 = 1;
const THEAD: u8 = 2;
const THEAD_MAE: u8 = 3;

/// Whether this is a T-Head hart (C906, C910), for `fp::init`. False before [`init`], the safe
/// default: a non-T-Head hart must not touch T-Head's bits.
///
/// Name: provisional, milestone 89 (Scaleway EM-RV1)'s lane, 2026-10-06 (UTC).
pub fn is_thead() -> bool {
    VENDOR.load(core::sync::atomic::Ordering::Relaxed) >= THEAD
}

/// Record whether this is a T-Head hart and whether it reads memory types from its page tables
/// (`th.sxstatus.MAEE`), and panic if the build's `mmu::Format` disagrees, in either direction.
///
/// `mvendorid` comes from SBI, since S-mode cannot read it. See notes/riscv-port/firmware.md.
fn probe_thead() {
    const GET_MVENDORID: usize = 4;
    let vendor = sbi_call(EID_BASE, GET_MVENDORID, 0);
    let found = if vendor != THEAD_VENDOR_ID {
        NOT_THEAD
    } else if super::instructions::read_th_sxstatus() & TH_SXSTATUS_MAEE != 0 {
        THEAD_MAE
    } else {
        THEAD
    };
    VENDOR.store(found, core::sync::atomic::Ordering::Relaxed);

    let built_for_mae = cfg!(feature = "board_th1520");
    if (found == THEAD_MAE) != built_for_mae {
        println!();
        if built_for_mae {
            println!(
                "nife cannot run on this machine: this kernel was built for a T-Head hart with"
            );
            println!(
                "  XTheadMae on (feature board_th1520), and this hart does not read memory types"
            );
            println!("  from its page tables, so the bits this kernel sets there are reserved.");
        } else {
            println!(
                "nife cannot run on this machine: this T-Head hart reads memory types from PTE"
            );
            println!("  bits 63:59 (th.sxstatus.MAEE is set), and this kernel writes zero there.");
            println!("  Build with --features board_th1520 (script/board-image --th1520).");
        }
        panic!("the page-table format this kernel was built for does not match the hart");
    }
}

/// The record. Panics if read before [`init`].
pub fn get() -> Isa {
    ISA.lock()
        .expect("arch::isa::get() before arch::isa::init()")
}

/// Record the measured `satp.ASID` width `bits`, from `mmu::init` right after its probe.
pub(crate) fn record_asid_bits(bits: usize) {
    if let Some(cpu) = ISA.lock().as_mut() {
        cpu.asid_bits = bits.min(u8::MAX as usize) as u8;
    }
}

/// Print the boot lines: what the silicon is, and what the firmware under it is.
pub(crate) fn print_summary() {
    let cpu = get();

    print!("  isa         : {}", cpu.base.name());
    for row in &machine_discovery::riscv64::TABLE {
        // `i` is skipped: the base already said it, and "rv64i i m a c" reads like a stutter.
        if cpu.common.contains(row.bit) && row.bit != machine_discovery::riscv64::I {
            print!(" {}", row.name);
        }
    }
    if cpu.described == 0 {
        print!(" (the device tree does not describe this machine's ISA)");
    }
    println!();

    print!(
        "              : {} hart(s), mmu {} declared and sv39 in use, satp.ASID {} bits measured",
        cpu.harts,
        cpu.mmu.name(),
        cpu.asid_bits,
    );
    if cpu.is_heterogeneous() {
        // What some harts have and others lack: on a JH7110, "no FPU on one core".
        print!("; harts differ (");
        for row in &machine_discovery::riscv64::TABLE {
            if cpu.any.contains(row.bit) && !cpu.common.contains(row.bit) {
                print!("{} ", row.name);
            }
        }
        print!("on some but not all)");
    }
    if cpu.legacy_isa_string {
        print!("; read from the deprecated riscv,isa");
    }
    println!();

    match VENDOR.load(core::sync::atomic::Ordering::Relaxed) {
        THEAD_MAE => {
            println!("  vendor      : T-Head, XTheadMae on, so leaves carry T-Head memory types");
        }
        THEAD => println!("  vendor      : T-Head, XTheadMae off, so leaves are plain Sv39"),
        _ => {}
    }

    print!("  firmware    : ");
    if !cpu.sbi.has_answered() {
        println!("SBI base extension did not answer, so nothing here is verified");
        return;
    }
    match cpu.sbi.impl_name() {
        Some(name) => print!("{name}"),
        None => print!("implementation {}", cpu.sbi.impl_id),
    }
    print!(
        " {:#x}, SBI {}.{},",
        cpu.sbi.impl_version, cpu.sbi.spec_major, cpu.sbi.spec_minor,
    );
    for row in &SBI_TABLE {
        if cpu.sbi.extensions.contains(row.bit) {
            print!(" {}", row.name);
        }
    }
    println!();
}

/// Ask the firmware what it is and what it implements, through the SBI base extension.
///
/// Returns `Sbi::default()` when the base extension does not answer (SBI v0.1 predates it), which
/// `Sbi::has_answered` tells apart from a refusal.
fn probe_sbi() -> Sbi {
    const GET_SPEC_VERSION: usize = 0;
    const GET_IMPL_ID: usize = 1;
    const GET_IMPL_VERSION: usize = 2;
    const PROBE_EXTENSION: usize = 3;

    let version = sbi_call(EID_BASE, GET_SPEC_VERSION, 0);
    if version == 0 {
        return Sbi::default();
    }

    let mut extensions = SbiExtensions::NONE;
    for row in &SBI_TABLE {
        // Zero means absent; present is any extension-defined nonzero.
        if sbi_call(EID_BASE, PROBE_EXTENSION, row.eid) != 0 {
            extensions = extensions.union(row.bit);
        }
    }

    Sbi {
        // Minor is the low 24 bits and major the seven above, not 16 and 16: `0x0300_0000` is 3.0.
        spec_major: ((version >> 24) & 0x7f) as u8,
        spec_minor: (version & 0x00ff_ffff) as u32,
        impl_id: sbi_call(EID_BASE, GET_IMPL_ID, 0) as u32,
        impl_version: sbi_call(EID_BASE, GET_IMPL_VERSION, 0) as u32,
        extensions,
    }
}

/// One SBI base-extension call `eid`/`fid` with `arg0`, returning `a1` and discarding the error.
///
/// Right only for the base getters, which cannot fail, and `probe_extension`, which reports absence
/// as a zero value. A firmware without the base extension leaves `a1` alone.
fn sbi_call(eid: usize, fid: usize, arg0: usize) -> usize {
    // SAFETY: the base extension's getters and `probe_extension` only report; they change no state
    // in the firmware or in this hart.
    unsafe { super::sbi::call(eid, fid, [arg0, 0, 0, 0, 0, 0]) }.value
}

#[cfg(test)]
mod tests {
    //! What the machine said, checked on the machine; the parsing is host-tested in
    //! `crates/machine_discovery/tests`.

    use super::*;

    /// Discovery ran and found a machine that can run us, with at least one hart describing itself.
    ///
    /// Not `described == harts`: the VisionFive 2 has five `cpu@` nodes and a disabled S7 core
    /// [`Isa::described`] does not count.
    #[test_case]
    fn the_machine_described_itself() {
        let cpu = get();

        assert!(
            cpu.harts >= 1,
            "a machine with no CPU node is not running this"
        );
        assert!(
            cpu.described >= 1,
            "described=0 means common is meaningless and the tree simply did not say"
        );
        assert!(
            cpu.described <= cpu.harts,
            "described ({}) cannot exceed the hart count ({}) it is drawn from",
            cpu.described,
            cpu.harts
        );
        assert_eq!(cpu.base, machine_discovery::riscv64::Base::Rv64);
        assert!(!cpu.missing_requirements().any());
    }

    /// Every hart that came online through SBI HSM is one the device tree described.
    ///
    /// Not an equality: the kernel starts at most `cpu::MAX_CPUS`, so the tree may describe more.
    #[test_case]
    fn every_online_hart_is_one_the_tree_described() {
        assert!(
            get().harts as usize >= crate::smp::online_count(),
            "more harts online than the device tree describes"
        );
    }

    /// The machine declares at least Sv39, the address space the kernel takes. QEMU's default CPU
    /// declares Sv57 and `sifive-u54` exactly Sv39, so only the floor is asserted.
    #[test_case]
    fn the_declared_mmu_is_wide_enough() {
        let mmu = get().mmu;
        assert!(
            mmu >= machine_discovery::riscv64::MmuType::Sv39,
            "we are running Sv39 on it"
        );
    }

    /// The ASID measurement reached the record, agrees with `mmu::asid_bits`, and the trust flag
    /// agrees with it on any width, zero included.
    ///
    /// A boot reordering that moved discovery after the probe would leave a zero here.
    #[test_case]
    fn the_asid_probe_reached_the_record() {
        let bits = get().asid_bits as usize;
        assert_eq!(bits, super::super::mmu::asid_bits());
        assert_eq!(
            super::super::mmu::asid_tagging_is_trusted(),
            bits >= super::super::mmu::ASID_BITS_NEEDED as usize,
            "asid_bits={bits}: the trust flag must agree with whether that width holds every \
             tag crates/address_space_identifier can hand out",
        );
    }

    /// The firmware answered and implements every extension the kernel calls unconditionally.
    ///
    /// Without RFENCE, for one, a shootdown is an error code nobody reads.
    #[test_case]
    fn the_firmware_implements_what_the_kernel_calls() {
        let sbi = get().sbi;

        assert!(
            sbi.has_answered(),
            "OpenSBI has had the base extension since 2020"
        );
        assert!(sbi.spec_major >= 1, "and speaks at least SBI 1.0");
        assert_eq!(sbi.impl_name(), Some("OpenSBI"), "which is what QEMU ships");
        assert!(
            sbi.extensions
                .contains(machine_discovery::riscv64::SBI_REQUIRED)
        );
    }
}
