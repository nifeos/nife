//! **A device tree for a machine that has none**: the ACPI tables an aarch64 server describes
//! itself with, written out as the flattened tree this kernel already reads.
//!
//! Handing the kernel an ACPI root pointer, as `x86_64` does, would mean a second discovery path
//! through `kernel/src/memory.rs`, `smp.rs`, `console.rs` and `arch/aarch64/`, and a change to the
//! handoff's wire format (`x0` is a device tree blob, Linux's arm64 boot protocol, which `boot.s`
//! cites), which is an architect's call. So the loader reads the tables while the firmware is alive
//! and **writes the tree the kernel would have been handed**. The translation loses information,
//! which is why the firmware's own tree stays first in the aarch64 loader's order (see BUGS).

//! # Tested by the reader, not by inspection
//!
//! Every test below decodes what this writes with `crates/device_tree_blob` and the
//! `machine_discovery` decoders the **kernel** uses (`gic::discover`, `cpu_list::CpuList`,
//! `aarch64::Psci`, `interrupt_id::of_node`, `DeviceTreeBlob::memory_regions`), so the writer and
//! the readers cannot drift apart without a host test failing in milliseconds.

//! # BUGS (the full accounts are in notes/device-tree.md)
//!
//! - **No PCI**: a PCI node's `ranges` and `interrupt-map` live in AML (`_CRS`, `_PRT`), so no
//!   `pci-host-ecam-generic` node is written and a bus that is there is invisible.
//! - **No virtio-mmio, no `fw_cfg`** (QEMU describes them in AML), **and no ITS node**: the MADT's
//!   ITS is printed, but the kernel drives no ITS (milestone 317 (the interrupt-remapping flags)).
//! - **The memory map is the UEFI one, merged**, with runtime-services and ACPI regions excluded.
//! - **Register block sizes are architectural, not stated** (the GIC's 64 KiB frames, the PL011's).
//! - **One `/memory` node per merged run, capped at [`MAX_RAM`]**; more runs are described short.

//! # Names
//!
//! Names in this module are **provisional** (2026-09-23): the module, [`Machine`], [`Cpu`],
//! [`Uart`], [`build`] and the root node's `compatible` string all name things a reader meets, and
//! naming is an architect's call.

use machine_discovery::acpi::{GTDT_ACTIVE_LOW, GTDT_EDGE_TRIGGERED, Gtdt};
use machine_discovery::gic::Gic;

use crate::device_tree_patch::{BEGIN_NODE, END, END_NODE, Error, HEADER_LEN, MAGIC, Out, PROP};

/// The most cores this writes a `cpu@` node for. Sixteen, matching
/// `machine_discovery::cpu_list::MAX_CPU_NODES`, which is what would read them back.
pub const MAX_CPUS: usize = 16;

/// The most `/memory` nodes this writes. Sixteen, matching `kernel::memory`'s own `MAX_REGIONS`.
pub const MAX_RAM: usize = 16;

/// The GIC distributor's register frame, which the architecture fixes at 64 KiB and the MADT never
/// states. See this module's BUGS.
pub const GICD_LEN: u64 = 0x1_0000;

/// A GICv2 CPU interface's frame. 8 KiB, which is the architected `GICC` block plus its alias page.
pub const GICC_LEN: u64 = 0x2000;

/// A PL011's register page. The SPCR gives an address and no length, and every PL011 occupies one
/// 4 KiB page.
const PL011_LEN: u64 = 0x1000;

/// The phandle the interrupt controller gets, and the root's `interrupt-parent`. Any nonzero value
/// would do; this tree has exactly one controller, so it gets the first.
const INTC_PHANDLE: u32 = 1;

/// The GIC's SPI bank base: an absolute INTID of `32 + n` is `interrupts = <0 n ...>`.
const GIC_SPI_BASE: u32 = 32;
/// The GIC's PPI bank base: an absolute INTID of `16 + n` is `interrupts = <1 n ...>`.
const GIC_PPI_BASE: u32 = 16;

/// A device tree interrupt flag: the line is level-triggered and asserted high.
const IRQ_TYPE_LEVEL_HIGH: u32 = 4;
/// Level-triggered, asserted low.
const IRQ_TYPE_LEVEL_LOW: u32 = 8;
/// Edge-triggered, rising.
const IRQ_TYPE_EDGE_RISING: u32 = 1;
/// Edge-triggered, falling.
const IRQ_TYPE_EDGE_FALLING: u32 = 2;

/// One core, as the MADT's GIC CPU interface entry describes it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Cpu {
    /// `MPIDR_EL1[39:0]`, which becomes the node's `reg` and is the id `CPU_ON` takes.
    pub mpidr: u64,
    /// Firmware will start this one. A disabled entry still gets a node, marked `status =
    /// "disabled"`, because a tree that omitted it would be claiming a smaller machine.
    pub enabled: bool,
}

/// The firmware's console UART, from the SPCR.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Uart {
    /// Its register block, physical.
    pub address: u64,
    /// The absolute INTID it raises, when the table states one.
    pub interrupt: Option<u32>,
}

/// A span of physical memory.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Ram {
    /// First byte.
    pub start: u64,
    /// How many.
    pub len: u64,
}

/// **Everything read out of ACPI and the firmware's memory map**, which is exactly what [`build`]
/// turns into a tree. Assembled by the caller, because reading physical memory is the caller's
/// unsafety and this module is pure.
#[derive(Copy, Clone, Debug)]
pub struct Machine {
    /// The interrupt controller, in the same record `machine_discovery::gic::discover` hands the
    /// kernel back. Writing the node this type reads, and reading it back in a test, is what keeps
    /// the two halves honest.
    pub gic: Gic,
    /// The cores, in MADT order.
    pub cpus: [Cpu; MAX_CPUS],
    /// How many of [`Machine::cpus`] are real.
    pub cpu_count: usize,
    /// `Some(true)` for PSCI over `hvc`, `Some(false)` for `smc`, `None` for a machine whose FADT
    /// does not claim PSCI. The kernel already treats a missing `/psci` as "one core", so `None`
    /// costs the secondaries and not the boot.
    pub psci_hvc: Option<bool>,
    /// The architected timer's interrupts, when there is a GTDT.
    pub timer: Option<Gtdt>,
    /// The console UART, when there is an SPCR naming one.
    pub uart: Option<Uart>,
    /// RAM, merged into runs.
    pub ram: [Ram; MAX_RAM],
    /// How many of [`Machine::ram`] are real.
    pub ram_count: usize,
}

/// The property names this writer uses, as one NUL-separated block. Every `FDT_PROP` token carries
/// an offset into it, and the block is emitted verbatim: the tree is small and fixed enough that a
/// table beats an interning pass.
const STRINGS: &[u8] = b"#address-cells\0#size-cells\0#interrupt-cells\0compatible\0reg\0device_type\0interrupt-controller\0interrupt-parent\0interrupts\0phandle\0method\0enable-method\0status\0";

/// Offset of `name` in [`STRINGS`].
///
/// # Panics
///
/// If `name` is not in the table, which is a bug in this file rather than anything a machine can
/// cause: every caller passes a literal that is written above.
fn string_offset(name: &[u8]) -> u32 {
    let mut at = 0usize;
    while at < STRINGS.len() {
        let end = at
            + STRINGS[at..]
                .iter()
                .position(|&b| b == 0)
                .expect("the strings block is NUL-terminated throughout");
        if &STRINGS[at..end] == name {
            return at as u32;
        }
        at = end + 1;
    }
    panic!("a property name this writer emits is missing from its own strings block")
}

/// The tree-writing verbs, on top of the byte-level [`Out`] the patcher already had.
trait WriteNode {
    fn begin(&mut self, name: &[u8]) -> Result<(), Error>;
    fn end_node(&mut self) -> Result<(), Error>;
    fn prop(&mut self, name: &[u8], value: &[u8]) -> Result<(), Error>;
    fn prop_cells(&mut self, name: &[u8], cells: &[u32]) -> Result<(), Error>;
    fn prop_empty(&mut self, name: &[u8]) -> Result<(), Error>;
}

impl WriteNode for Out<'_> {
    fn begin(&mut self, name: &[u8]) -> Result<(), Error> {
        self.u32(BEGIN_NODE)?;
        self.bytes(name)?;
        self.bytes(&[0])?;
        self.pad_to(4)
    }

    fn end_node(&mut self) -> Result<(), Error> {
        self.u32(END_NODE)
    }

    fn prop(&mut self, name: &[u8], value: &[u8]) -> Result<(), Error> {
        self.u32(PROP)?;
        self.u32(value.len() as u32)?;
        self.u32(string_offset(name))?;
        self.bytes(value)?;
        self.pad_to(4)
    }

    fn prop_cells(&mut self, name: &[u8], cells: &[u32]) -> Result<(), Error> {
        self.u32(PROP)?;
        self.u32((cells.len() * 4) as u32)?;
        self.u32(string_offset(name))?;
        for cell in cells {
            self.u32(*cell)?;
        }
        Ok(())
    }

    fn prop_empty(&mut self, name: &[u8]) -> Result<(), Error> {
        self.prop(name, &[])
    }
}

/// A node name with a unit address, written into `buf` and returned as the slice that was used.
///
/// Device tree unit addresses are lower-case hex with no `0x`, and the node name is what the kernel
/// matches on (`console::UART_NODE` is the literal `pl011@9000000`), so the spelling is load-bearing
/// rather than cosmetic.
fn unit_name<'a>(prefix: &[u8], address: u64, buf: &'a mut [u8; 40]) -> &'a [u8] {
    buf[..prefix.len()].copy_from_slice(prefix);
    let mut at = prefix.len();
    buf[at] = b'@';
    at += 1;
    let mut shift = 60i32;
    let mut started = false;
    while shift >= 0 {
        let nibble = ((address >> shift) & 0xf) as u8;
        shift -= 4;
        if nibble == 0 && !started && shift >= 0 {
            continue;
        }
        started = true;
        buf[at] = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        };
        at += 1;
    }
    &buf[..at]
}

/// The two cells of a 64-bit value, high first, which is how a `#address-cells = <2>` tree spells
/// an address.
const fn cells64(v: u64) -> [u32; 2] {
    [(v >> 32) as u32, v as u32]
}

/// A GTDT timer's `(GSIV, flags)` as the three cells a GIC's `interrupts` entry wants.
///
/// The architected timers are all PPIs, so a GSIV below [`GIC_PPI_BASE`] is a table this writer does
/// not understand rather than a number to subtract into the negatives; such a timer is written as
/// PPI 0, which arms nothing, and the kernel's own `TIMER_INTID` constant is what it actually uses.
/// Nothing here can make it arm the wrong line.
fn timer_cells(timer: (u32, u32)) -> [u32; 3] {
    let (gsiv, flags) = timer;
    let ppi = gsiv.saturating_sub(GIC_PPI_BASE);
    [1, ppi, interrupt_flags(flags)]
}

/// GTDT interrupt flags as a device tree interrupt type.
const fn interrupt_flags(flags: u32) -> u32 {
    match (
        flags & GTDT_EDGE_TRIGGERED != 0,
        flags & GTDT_ACTIVE_LOW != 0,
    ) {
        (false, false) => IRQ_TYPE_LEVEL_HIGH,
        (false, true) => IRQ_TYPE_LEVEL_LOW,
        (true, false) => IRQ_TYPE_EDGE_RISING,
        (true, true) => IRQ_TYPE_EDGE_FALLING,
    }
}

/// **An upper bound on the tree [`build`] will write**, which is what the caller allocates.
///
/// Deliberately generous and deliberately not exact: every node here is a few dozen bytes, the
/// whole tree is under four kilobytes on any machine this can describe, and an allocation sized by
/// a second copy of the layout is a second place to be wrong.
pub const fn output_len() -> usize {
    // Header, reservation block, a fixed part (root, psci, cpus, intc, timer, uart), the per-core
    // and per-region nodes, and the strings block.
    HEADER_LEN + 16 + 1024 + (MAX_CPUS + MAX_RAM) * 128 + STRINGS.len() + 64
}

/// **Write `machine` as a flattened device tree into `out`**, returning its length.
///
/// `out` must be at least [`output_len`] bytes. The result has no `/chosen`;
/// `device_tree_patch::with_initial_ramdisk` adds one, which is the same call the firmware's own
/// tree goes through, so this module has one job and the archive handoff keeps its single writer.
pub fn build(machine: &Machine, out: &mut [u8]) -> Result<usize, Error> {
    let mut o = Out { buf: out, at: 0 };
    o.bytes(&[0; HEADER_LEN])?; // filled in last, once the offsets are known
    o.pad_to(8)?;
    let off_rsvmap = o.at;
    // The reservation block's terminator, and nothing before it: what this tree reserves is said
    // with nodes, and the firmware's own reservations went away with `ExitBootServices`.
    o.bytes(&[0; 16])?;
    let off_struct = o.at;

    o.begin(b"")?;
    o.prop_cells(b"#address-cells", &[2])?;
    o.prop_cells(b"#size-cells", &[2])?;
    // Provisional (2026-09-23). A root `compatible` is required by the specification and read by
    // nothing in this kernel; what it is for is a person reading a dump and asking where this tree
    // came from, so it says.
    o.prop(b"compatible", b"nife,machine-from-acpi\0")?;
    o.prop_cells(b"interrupt-parent", &[INTC_PHANDLE])?;

    write_psci(&mut o, machine)?;
    write_cpus(&mut o, machine)?;
    write_intc(&mut o, machine)?;
    write_timer(&mut o, machine)?;
    write_uart(&mut o, machine)?;
    write_memory(&mut o, machine)?;

    o.end_node()?;
    o.u32(END)?;

    let size_struct = o.at - off_struct;
    let off_strings = o.at;
    o.bytes(STRINGS)?;
    let size_strings = o.at - off_strings;
    let total = o.at;

    let header: [u32; 10] = [
        MAGIC,
        total as u32,
        off_struct as u32,
        off_strings as u32,
        off_rsvmap as u32,
        17, // version
        16, // last compatible version
        0,  // boot_cpuid_phys: this writer does not know which core is running it
        size_strings as u32,
        size_struct as u32,
    ];
    for (i, word) in header.iter().enumerate() {
        o.buf[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    Ok(total)
}

/// `/psci`, when the FADT claims PSCI. A machine that does not gets no node, which
/// `machine_discovery::aarch64::Psci::from_device_tree` reads as `None` and the kernel reports as
/// one core rather than as a failure.
fn write_psci(o: &mut Out<'_>, machine: &Machine) -> Result<(), Error> {
    let Some(hvc) = machine.psci_hvc else {
        return Ok(());
    };
    o.begin(b"psci")?;
    // `arm,psci-1.0` first, which is what the FADT's flag means as of ACPI 6: a machine claiming
    // the bit implements at least 0.2, and the 1.0 string is what QEMU's own tree states. Both are
    // listed because both are accepted, and the decoder takes the standard `CPU_ON` id from either.
    o.prop(b"compatible", b"arm,psci-1.0\0arm,psci-0.2\0")?;
    o.prop(b"method", if hvc { b"hvc\0" } else { b"smc\0" })?;
    o.end_node()
}

/// `/cpus`, one `cpu@<mpidr>` per MADT GIC CPU interface entry.
fn write_cpus(o: &mut Out<'_>, machine: &Machine) -> Result<(), Error> {
    o.begin(b"cpus")?;
    // Two address cells, because an `MPIDR_EL1` affinity value is 40 bits and the ARM CPU binding
    // requires two the moment `Aff3` is nonzero. One would be right on QEMU and wrong on a machine
    // with clusters, and the decoder reads the declaration rather than assuming either.
    o.prop_cells(b"#address-cells", &[2])?;
    o.prop_cells(b"#size-cells", &[0])?;
    for cpu in &machine.cpus[..machine.cpu_count.min(MAX_CPUS)] {
        let mut buf = [0u8; 40];
        o.begin(unit_name(b"cpu", cpu.mpidr, &mut buf))?;
        o.prop(b"device_type", b"cpu\0")?;
        o.prop(b"compatible", b"arm,armv8\0")?;
        o.prop_cells(b"reg", &cells64(cpu.mpidr))?;
        o.prop(b"enable-method", b"psci\0")?;
        // A disabled MADT entry is firmware saying "a socket is here and I will not start it",
        // which is exactly what `status = "disabled"` says to `cpu_list::Cpu::is_startable`.
        if !cpu.enabled {
            o.prop(b"status", b"disabled\0")?;
        }
        o.end_node()?;
    }
    o.end_node()
}

/// The interrupt controller, in the binding `machine_discovery::gic::discover` reads back.
fn write_intc(o: &mut Out<'_>, machine: &Machine) -> Result<(), Error> {
    let (compatible, distributor, second) = match machine.gic {
        Gic::V2 {
            distributor,
            cpu_interface,
        } => (&b"arm,cortex-a15-gic\0"[..], distributor, cpu_interface),
        Gic::V3 {
            distributor,
            redistributors,
        } => (&b"arm,gic-v3\0"[..], distributor, redistributors),
    };
    let mut buf = [0u8; 40];
    o.begin(unit_name(b"intc", distributor.start, &mut buf))?;
    o.prop(b"compatible", compatible)?;
    o.prop_empty(b"interrupt-controller")?;
    // Three cells is the GIC binding: <type number flags>. `machine_discovery::interrupt_id` reads
    // this through the root's `interrupt-parent` phandle to decode the UART's line, so a wrong
    // count here would not be a wrong answer, it would be `None` and the documented fallback.
    o.prop_cells(b"#interrupt-cells", &[3])?;
    o.prop_cells(b"#address-cells", &[2])?;
    o.prop_cells(b"#size-cells", &[2])?;
    o.prop_cells(b"phandle", &[INTC_PHANDLE])?;
    let mut reg = [0u32; 8];
    reg[..2].copy_from_slice(&cells64(distributor.start));
    reg[2..4].copy_from_slice(&cells64(distributor.size));
    reg[4..6].copy_from_slice(&cells64(second.start));
    reg[6..8].copy_from_slice(&cells64(second.size));
    o.prop_cells(b"reg", &reg)?;
    o.end_node()
}

/// The architected timer, from the GTDT. Nothing on the interrupt path reads it (`TIMER_INTID` is
/// a constant, and PPI 11 is architected), but `arch::aarch64::timer` cross-checks `CNTFRQ_EL0`
/// against this node's `clock-frequency` when one is stated, and a dump that names the machine's
/// timer is worth the forty bytes.
fn write_timer(o: &mut Out<'_>, machine: &Machine) -> Result<(), Error> {
    let Some(gtdt) = machine.timer else {
        return Ok(());
    };
    o.begin(b"timer")?;
    o.prop(b"compatible", b"arm,armv8-timer\0")?;
    // The binding's order: secure EL1, non-secure EL1, virtual, and the EL2 physical timer. The
    // same four the GTDT states, in the same order, which is why this is a copy and not a decision.
    let mut cells = [0u32; 12];
    cells[0..3].copy_from_slice(&timer_cells(gtdt.secure_el1));
    cells[3..6].copy_from_slice(&timer_cells(gtdt.non_secure_el1));
    cells[6..9].copy_from_slice(&timer_cells(gtdt.virtual_el1));
    cells[9..12].copy_from_slice(&timer_cells(gtdt.el2));
    o.prop_cells(b"interrupts", &cells)?;
    // **No `clock-frequency`.** The GTDT has no such field; the rate is `CNTFRQ_EL0`, which the
    // kernel reads directly. Writing the register's own value back as a property would turn the
    // kernel's cross-check into a comparison of a number with itself.
    o.end_node()
}

/// The console UART, from the SPCR, named so that `console::UART_NODE`'s prefix match finds it.
fn write_uart(o: &mut Out<'_>, machine: &Machine) -> Result<(), Error> {
    let Some(uart) = machine.uart else {
        return Ok(());
    };
    let mut buf = [0u8; 40];
    o.begin(unit_name(b"pl011", uart.address, &mut buf))?;
    o.prop(b"compatible", b"arm,pl011\0arm,primecell\0")?;
    let mut reg = [0u32; 4];
    reg[..2].copy_from_slice(&cells64(uart.address));
    reg[2..4].copy_from_slice(&cells64(PL011_LEN));
    o.prop_cells(b"reg", &reg)?;
    if let Some(gsiv) = uart.interrupt {
        // A UART raises an SPI. A GSIV below the SPI base would be a PPI or an SGI, which no UART
        // raises and this writer will not translate into a number that means something else; the
        // property is omitted and the kernel says it fell back, which is the honest report.
        if let Some(spi) = gsiv.checked_sub(GIC_SPI_BASE) {
            // Level-high is what every PL011 binding states and what QEMU's own tree says (`<0 1
            // 4>`). The SPCR has no polarity field to read, so this is the binding's value rather
            // than the machine's claim, and it is the one place in this file that is neither.
            o.prop_cells(b"interrupts", &[0, spi, IRQ_TYPE_LEVEL_HIGH])?;
        }
    }
    o.end_node()
}

/// One `/memory` node per merged run of RAM.
fn write_memory(o: &mut Out<'_>, machine: &Machine) -> Result<(), Error> {
    for run in &machine.ram[..machine.ram_count.min(MAX_RAM)] {
        let mut buf = [0u8; 40];
        o.begin(unit_name(b"memory", run.start, &mut buf))?;
        o.prop(b"device_type", b"memory\0")?;
        let mut reg = [0u32; 4];
        reg[..2].copy_from_slice(&cells64(run.start));
        reg[2..4].copy_from_slice(&cells64(run.len));
        o.prop_cells(b"reg", &reg)?;
        o.end_node()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use device_tree_blob::{DeviceTreeBlob, Region};
    use machine_discovery::aarch64::{Conduit, Psci};
    use machine_discovery::cpu_list::{CpuList, EnableMethod};
    use machine_discovery::{gic, interrupt_id};

    use super::*;

    /// QEMU `virt` with `acpi=on,gic-version=3`, as its tables describe it: the machine the boot
    /// this milestone measured actually ran on.
    fn qemu_virt() -> Machine {
        let mut cpus = [Cpu::default(); MAX_CPUS];
        for (i, cpu) in cpus[..4].iter_mut().enumerate() {
            *cpu = Cpu {
                mpidr: i as u64,
                enabled: true,
            };
        }
        let mut ram = [Ram::default(); MAX_RAM];
        ram[0] = Ram {
            start: 0x4000_0000,
            len: 0x1000_0000,
        };
        Machine {
            gic: Gic::V3 {
                distributor: Region {
                    start: 0x0800_0000,
                    size: GICD_LEN,
                },
                redistributors: Region {
                    start: 0x080a_0000,
                    size: 0x00f6_0000,
                },
            },
            cpus,
            cpu_count: 4,
            psci_hvc: Some(true),
            timer: Some(Gtdt {
                secure_el1: (29, 0),
                non_secure_el1: (30, 0),
                virtual_el1: (27, 0),
                el2: (26, 0),
            }),
            uart: Some(Uart {
                address: 0x0900_0000,
                interrupt: Some(33),
            }),
            ram,
            ram_count: 1,
        }
    }

    fn built(machine: &Machine) -> (usize, [u8; 8192]) {
        let mut out = [0u8; 8192];
        assert!(output_len() <= out.len(), "the fixture buffer is generous");
        let len = build(machine, &mut out).expect("the tree is written");
        (len, out)
    }

    /// **The whole claim, read back through the kernel's own decoders.**
    ///
    /// Every assertion here is a call the kernel makes on the real boot path, against the bytes
    /// this module writes. A writer proved by inspection is proved by nothing; this is the rule
    /// `handoff.rs` and `device_tree_patch.rs` already follow, and it is what would catch a cell
    /// count or a node name drifting out from under `memory::init` in milliseconds rather than in
    /// a QEMU run.
    #[test]
    fn what_this_writes_is_what_the_kernel_reads() {
        let machine = qemu_virt();
        let (len, out) = built(&machine);
        let dt = DeviceTreeBlob::from_bytes(&out[..len]).expect("a well-formed device tree");

        // `memory::init`, first call: the interrupt controller, found by its binding.
        assert_eq!(
            gic::discover(&dt).expect("the GIC node parses"),
            Some(machine.gic),
            "the GIC written is not the GIC read back"
        );

        // `memory::init`, second call: RAM.
        let mut regions = [Region { start: 0, size: 0 }; MAX_RAM];
        let n = dt
            .memory_regions(&mut regions)
            .expect("the memory nodes parse");
        assert_eq!(n, 1);
        assert_eq!(
            (regions[0].start, regions[0].size),
            (0x4000_0000, 0x1000_0000)
        );

        // `memory::init`, third call: the console UART's interrupt, decoded through the root's
        // `interrupt-parent` phandle and the controller's `#interrupt-cells`. 33 is SPI 1, which
        // is what QEMU's own tree states and what the kernel's fallback constant also says; the
        // point is that it came from the machine.
        assert_eq!(
            interrupt_id::of_node(&dt, b"pl011@9000000").expect("the UART node parses"),
            Some(33)
        );

        // `smp::read_cpu_list`.
        let list = CpuList::from_device_tree(&dt).expect("/cpus parses");
        assert_eq!(list.described, 4);
        for (i, cpu) in list.cpus().iter().enumerate() {
            assert_eq!(cpu.hwid, i as u64);
            assert!(
                cpu.is_startable(),
                "an enabled MADT entry is a startable core"
            );
            assert_eq!(cpu.enable_method, EnableMethod::Psci);
        }

        // `arch::aarch64::isa::init`.
        let psci = Psci::from_device_tree(&dt)
            .expect("/psci parses")
            .expect("this machine claims PSCI");
        assert_eq!(psci.conduit, Some(Conduit::Hvc));
        assert!(psci.can_start_a_cpu());

        // `arch::aarch64::timer::check_frequency_against_device_tree` looks for this node, and
        // finds no `clock-frequency` on it, which is the path that returns without comparing.
        assert_eq!(
            dt.node_prop_compatible(b"arm,armv8-timer", b"clock-frequency")
                .expect("the timer node parses"),
            None,
            "the GTDT states no rate, so neither does this node"
        );
    }

    /// **The virtual timer's GSIV becomes the PPI the binding spells**, which is the one conversion
    /// in this file that is arithmetic rather than a copy. 27 absolute is PPI 11, because PPIs
    /// start at INTID 16; getting the direction wrong would write 43 and describe a different
    /// interrupt entirely.
    #[test]
    fn a_gtdt_gsiv_becomes_the_bank_relative_number_the_binding_wants() {
        let (len, out) = built(&qemu_virt());
        let dt = DeviceTreeBlob::from_bytes(&out[..len]).expect("a well-formed device tree");
        let interrupts = dt
            .node_prop_compatible(b"arm,armv8-timer", b"interrupts")
            .expect("the timer node parses")
            .expect("the node states its interrupts");
        assert_eq!(interrupts.len(), 12 * 4, "four timers, three cells each");
        let cell = |i: usize| {
            u32::from_be_bytes([
                interrupts[i * 4],
                interrupts[i * 4 + 1],
                interrupts[i * 4 + 2],
                interrupts[i * 4 + 3],
            ])
        };
        // Secure 29, non-secure 30, virtual 27, EL2 26, each as <1 (gsiv - 16) 4>.
        for (slot, gsiv) in [(0usize, 29u32), (1, 30), (2, 27), (3, 26)] {
            assert_eq!(cell(slot * 3), 1, "a PPI");
            assert_eq!(cell(slot * 3 + 1), gsiv - 16);
            assert_eq!(cell(slot * 3 + 2), IRQ_TYPE_LEVEL_HIGH);
        }
    }

    /// A GICv2 machine, which is what QEMU `virt` presents by default and what this milestone's
    /// first ACPI boot actually ran on. The second `reg` block is the CPU interface rather than a
    /// redistributor array, and `gic::discover` is what has to tell them apart.
    #[test]
    fn a_gicv2_machine_reads_back_as_a_gicv2() {
        let mut machine = qemu_virt();
        machine.gic = Gic::V2 {
            distributor: Region {
                start: 0x0800_0000,
                size: GICD_LEN,
            },
            cpu_interface: Region {
                start: 0x0801_0000,
                size: GICC_LEN,
            },
        };
        let (len, out) = built(&machine);
        let dt = DeviceTreeBlob::from_bytes(&out[..len]).expect("a well-formed device tree");
        assert_eq!(
            gic::discover(&dt).expect("the GIC node parses"),
            Some(machine.gic)
        );
    }

    /// **Every optional table really is optional.** A machine with nothing but a GIC and RAM still
    /// yields a tree the kernel can walk: no `/psci` (one core, said at bring-up rather than as a
    /// failure), no timer node, no UART node (the kernel falls back to its constant and says so).
    #[test]
    fn a_machine_with_only_a_gic_and_ram_still_yields_a_readable_tree() {
        let mut machine = qemu_virt();
        machine.psci_hvc = None;
        machine.timer = None;
        machine.uart = None;
        machine.cpu_count = 1;
        let (len, out) = built(&machine);
        let dt = DeviceTreeBlob::from_bytes(&out[..len]).expect("a well-formed device tree");
        assert_eq!(
            gic::discover(&dt).expect("the GIC node parses"),
            Some(machine.gic)
        );
        assert_eq!(Psci::from_device_tree(&dt).expect("/psci parses"), None);
        assert_eq!(
            interrupt_id::of_node(&dt, b"pl011@9000000").expect("no such node"),
            None
        );
        assert_eq!(
            CpuList::from_device_tree(&dt)
                .expect("/cpus parses")
                .described,
            1
        );
    }

    /// **A disabled MADT entry becomes a core the kernel will not start**, and stays in the list.
    /// Dropping it would describe a smaller machine than the one in front of us; marking it
    /// `status = "disabled"` is what `cpu_list::Cpu::is_startable` already knows how to refuse.
    #[test]
    fn a_disabled_cpu_is_described_and_not_startable() {
        let mut machine = qemu_virt();
        machine.cpus[2].enabled = false;
        let (len, out) = built(&machine);
        let dt = DeviceTreeBlob::from_bytes(&out[..len]).expect("a well-formed device tree");
        let list = CpuList::from_device_tree(&dt).expect("/cpus parses");
        assert_eq!(list.described, 4, "all four are described");
        let startable = list.cpus().iter().filter(|c| c.is_startable()).count();
        assert_eq!(startable, 3, "the disabled one is refused");
    }

    /// **The archive handoff is unchanged**: the written tree goes through the same
    /// `device_tree_patch::with_initial_ramdisk` the firmware's own tree does, and the kernel reads
    /// `/chosen` back. This is the seam where a tree with no `/chosen` at all would have been a
    /// silent boot with no userspace.
    #[test]
    fn the_written_tree_takes_an_initial_ramdisk_like_any_other() {
        let (len, out) = built(&qemu_virt());
        let capacity =
            crate::device_tree_patch::output_len(&out[..len]).expect("the tree is well formed");
        let mut patched = [0u8; 8192];
        assert!(capacity <= patched.len());
        crate::device_tree_patch::with_initial_ramdisk(
            &out[..len],
            0x4ac6_0000,
            0x4b70_0000,
            &mut patched,
        )
        .expect("the initial ramdisk is named");
        let dt = DeviceTreeBlob::from_bytes(&patched).expect("still a device tree");
        let initial_ramdisk = dt
            .initial_ramdisk()
            .expect("/chosen parses")
            .expect("the archive is named");
        assert_eq!(initial_ramdisk.start, 0x4ac6_0000);
        assert_eq!(initial_ramdisk.size, 0x4b70_0000 - 0x4ac6_0000);
        // And the machine survived the rewrite, which is the half a `/chosen` test usually forgets.
        assert!(gic::discover(&dt).expect("the GIC node parses").is_some());
    }

    /// A buffer smaller than the tree is refused rather than half-written. The caller allocates
    /// [`output_len`] pages, so this is the guard on a future machine with more cores or more RAM
    /// regions than that bound covers.
    #[test]
    fn a_short_buffer_is_refused() {
        let machine = qemu_virt();
        let (len, _) = built(&machine);
        for short in [0usize, 8, HEADER_LEN, len / 2, len - 1] {
            let mut out = [0u8; 8192];
            assert_eq!(
                build(&machine, &mut out[..short]),
                Err(Error::OutputTooSmall),
                "{short} bytes is short of the {len} this tree needs"
            );
        }
    }

    /// Unit addresses are lower-case hex with no prefix and no leading zeros, and a zero address is
    /// the single digit `0`. The kernel matches `console::UART_NODE` against this spelling
    /// literally, so it is a contract rather than a formatting preference.
    #[test]
    fn unit_addresses_are_spelled_the_way_a_device_tree_spells_them() {
        let mut buf = [0u8; 40];
        assert_eq!(unit_name(b"cpu", 0, &mut buf), b"cpu@0");
        assert_eq!(unit_name(b"pl011", 0x0900_0000, &mut buf), b"pl011@9000000");
        assert_eq!(
            unit_name(b"memory", 0xffff_0000_0000_0000, &mut buf),
            b"memory@ffff000000000000"
        );
    }
    /// The digit after nine is `a`, the first letter, and every value across the boundary spells
    /// itself: the existing cases hit `0` and `f` and nothing between 9 and a.
    #[test]
    fn the_digits_either_side_of_nine_are_spelled_right() {
        let mut buf = [0u8; 40];
        assert_eq!(unit_name(b"x", 0x9a, &mut buf), b"x@9a");
        assert_eq!(unit_name(b"x", 0xa9, &mut buf), b"x@a9");
        assert_eq!(
            unit_name(b"x", 0xabcdef0123456789, &mut buf),
            b"x@abcdef0123456789"
        );
    }

    /// An address cell pair is high word first, and a value with a high word keeps it. Every
    /// address in the fixtures sits below 4 GiB, so a writer that dropped the high cell passed.
    #[test]
    fn a_wide_value_keeps_its_high_cell() {
        assert_eq!(cells64(0x0000_0001_0000_0002), [1, 2]);
        assert_eq!(cells64(0xffff_0000_8000_0001), [0xffff_0000, 0x8000_0001]);
    }

    /// A name this writer does not carry is a bug in this file and says so, rather than reading
    /// past the end of the block looking for it.
    #[test]
    #[should_panic(expected = "missing from its own strings block")]
    fn a_property_name_outside_the_table_is_named_as_the_bug() {
        let _ = string_offset(b"not-a-property");
    }

    /// Every name this writer emits is found at its own offset, and the empty property writes the
    /// three words a property header is: token, zero length, name offset.
    #[test]
    fn an_empty_property_is_a_header_and_nothing_else() {
        let mut buf = [0xaau8; 32];
        let mut o = Out {
            buf: &mut buf,
            at: 0,
        };
        o.prop_empty(b"interrupt-controller").expect("fits");
        assert_eq!(o.at, 12);
        let mut want = [0u8; 12];
        want[..4].copy_from_slice(&PROP.to_be_bytes());
        want[8..].copy_from_slice(&string_offset(b"interrupt-controller").to_be_bytes());
        assert_eq!(&buf[..12], &want);
        assert_eq!(buf[12], 0xaa, "nothing is written past the header");
    }

    /// The header's own bookkeeping: the struct block runs from its offset to the strings block's,
    /// the strings block to the end, and `totalsize` is the length returned. The readers in the
    /// kernel tolerate a wrong size here, which is why nothing else would notice.
    #[test]
    fn the_header_sizes_describe_the_blocks_that_follow() {
        let (len, out) = built(&qemu_virt());
        let word =
            |i: usize| u32::from_be_bytes(out[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
        let (total, off_struct, off_strings) = (word(1), word(2), word(3));
        let (size_strings, size_struct) = (word(8), word(9));
        assert_eq!(total, len);
        assert_eq!(size_struct, off_strings - off_struct);
        assert_eq!(size_strings, total - off_strings);
        assert_eq!(size_strings, STRINGS.len());
    }

    /// **The allocation bound covers the largest tree this writer can be asked for, and is not
    /// absurdly larger.** Every core enabled and every RAM region present, at the widest addresses,
    /// written into a buffer of exactly [`output_len`] bytes. The upper bound is the module's own
    /// claim that the whole tree is a few kilobytes.
    #[test]
    fn the_bound_holds_the_largest_tree_and_not_a_great_deal_more() {
        let mut machine = qemu_virt();
        for (i, cpu) in machine.cpus.iter_mut().enumerate() {
            *cpu = Cpu {
                mpidr: 0xff00_ffff_ffff_0000 + i as u64,
                enabled: false,
            };
        }
        machine.cpu_count = MAX_CPUS;
        for (i, ram) in machine.ram.iter_mut().enumerate() {
            *ram = Ram {
                start: 0xffff_0000_0000_0000 + (i as u64) * 0x1000_0000,
                len: 0xffff_ffff,
            };
        }
        machine.ram_count = MAX_RAM;
        let mut out = vec![0u8; output_len()];
        let len = build(&machine, &mut out).expect("the bound holds the largest tree");
        assert!(len <= output_len());
        assert!(
            output_len() < len + 2048,
            "{} bytes allocated for a {len}-byte tree",
            output_len()
        );
    }
}
