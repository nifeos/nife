//! What the machine told us on the way in, `x86_64`: the PVH `hvm_start_info` structure, and what
//! the CPU says about itself when asked.
//!
//! PVH hands over one pointer to a structure carrying the memory map, the ACPI root pointer, the
//! command line and any loaded modules, the same shape as the device-tree handoff in another
//! format. [`BootInfo::parse`] reads the memory-map fields only when the version says they are
//! there (version 0 stops at offset 40). [`Isa`] decides what a `CPUID` answer means, which is the
//! part that can be wrong. See `notes/machine-discovery.md` ("x86_64") and
//! `notes/x86-port/acpi-and-pci.md`.

//! # BUGS
//!
//! - Nothing checks that the memory map is sorted or non-overlapping. The specification promises
//!   neither, and the frame allocator that consumes it is where that check belongs.

/// The magic word at offset 0 of `hvm_start_info`, and the same value the boot CPU is handed in
/// `eax`. "xEn3" read little-endian.
pub const MAGIC: u32 = 0x336e_c578;

/// The size of the version-0 structure, which is where the memory-map fields begin.
const V0_LEN: usize = 40;
/// The size of the version-1 structure.
const V1_LEN: usize = 56;

/// The size of one `hvm_memmap_table_entry`.
pub const MEMMAP_ENTRY_LEN: usize = 24;

/// Why a `hvm_start_info` could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootInfoError {
    /// The magic word is not [`MAGIC`]. Either the pointer is wrong or the loader is not speaking
    /// PVH, and in both cases nothing after offset 4 means anything.
    BadMagic(u32),
    /// The bytes end before the structure does.
    Truncated,
}

/// What the loader said, decoded.
///
/// Every address in here is **physical**, because that is what a loader running with paging off can
/// mean. The kernel reaches them through its direct map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootInfo {
    /// The structure version. 0 has no memory map; 1 and later do.
    pub version: u32,
    /// Loader-defined flags. Xen defines bit 0 (`SecureBoot`) and bit 1 (`SecureBootEnabled`); QEMU
    /// sets neither.
    pub flags: u32,
    /// How many modules (initial ramdisk images) were loaded.
    pub module_count: u32,
    /// Where the module list is, or 0.
    pub modules: u64,
    /// Where the NUL-terminated command line is, or 0.
    pub cmdline: u64,
    /// **The ACPI RSDP**, the root of every table x86 uses in place of a device tree: the MADT (the
    /// APICs), the MCFG (the PCIe ECAM window) and the DMAR (the IOMMU) are all reached from here.
    /// 0 when the loader did not say, which on a machine with ACPI means it must be found by
    /// scanning the BIOS area instead.
    pub rsdp: u64,
    /// Where the memory map is, or 0 on a version-0 handoff.
    pub memmap: u64,
    /// How many entries the memory map has. Zero on a version-0 handoff.
    pub memmap_entries: u32,
}

impl BootInfo {
    /// Decode `bytes`, which must begin at the structure.
    ///
    /// The magic is checked first and everything else is refused until it passes, which is the same
    /// discipline `device_tree_blob::DeviceTreeBlob::from_ptr` follows and for the same reason:
    /// this is the first thing the kernel does with a pointer somebody else chose, so a wrong
    /// pointer must produce an error rather than a plausible-looking memory map.
    pub fn parse(bytes: &[u8]) -> Result<Self, BootInfoError> {
        if bytes.len() < V0_LEN {
            return Err(BootInfoError::Truncated);
        }
        let magic = u32(bytes, 0);
        if magic != MAGIC {
            return Err(BootInfoError::BadMagic(magic));
        }
        let version = u32(bytes, 4);

        // The memory-map fields exist only from version 1. Reading them from a version-0 structure
        // would be reading whatever the loader left after its own last field, which is exactly the
        // kind of plausible garbage this crate exists to keep out of the kernel.
        let (memmap, memmap_entries) = if version >= 1 {
            if bytes.len() < V1_LEN {
                return Err(BootInfoError::Truncated);
            }
            (u64(bytes, 40), u32(bytes, 48))
        } else {
            (0, 0)
        };

        Ok(BootInfo {
            version,
            flags: u32(bytes, 8),
            module_count: u32(bytes, 12),
            modules: u64(bytes, 16),
            cmdline: u64(bytes, 24),
            rsdp: u64(bytes, 32),
            memmap,
            memmap_entries,
        })
    }
}

/// What a memory-map entry says the range is for. The numbers are E820's, which PVH reuses rather
/// than inventing its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    /// Ordinary RAM. **The only kind a frame allocator may hand out.**
    Ram,
    /// Firmware or hardware owns it. Never allocatable.
    Reserved,
    /// ACPI tables live here. Not allocatable until they have been read, and this kernel has not
    /// read them, so it is not allocatable at all yet.
    AcpiReclaimable,
    /// ACPI non-volatile storage. Never allocatable.
    AcpiNvs,
    /// RAM the firmware found faulty.
    Unusable,
    /// Present but disabled.
    Disabled,
    /// Persistent memory.
    Persistent,
    /// A type this decoder does not know. **Treated as not-RAM by [`MemoryEntry::is_usable_ram`]**,
    /// which is the safe direction: a new E820 type has never meant "more RAM than you thought".
    Unknown(u32),
}

impl MemoryKind {
    /// Decode the 32-bit type word.
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => MemoryKind::Ram,
            2 => MemoryKind::Reserved,
            3 => MemoryKind::AcpiReclaimable,
            4 => MemoryKind::AcpiNvs,
            5 => MemoryKind::Unusable,
            6 => MemoryKind::Disabled,
            7 => MemoryKind::Persistent,
            other => MemoryKind::Unknown(other),
        }
    }

    /// A short word for the boot print.
    pub const fn name(self) -> &'static str {
        match self {
            MemoryKind::Ram => "ram",
            MemoryKind::Reserved => "reserved",
            MemoryKind::AcpiReclaimable => "acpi",
            MemoryKind::AcpiNvs => "acpi-nvs",
            MemoryKind::Unusable => "unusable",
            MemoryKind::Disabled => "disabled",
            MemoryKind::Persistent => "pmem",
            MemoryKind::Unknown(_) => "unknown",
        }
    }
}

/// One range of physical memory and what it is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryEntry {
    pub addr: u64,
    pub size: u64,
    pub kind: MemoryKind,
}

impl MemoryEntry {
    /// May a frame allocator hand this range out?
    ///
    /// Only [`MemoryKind::Ram`], and deliberately **not** [`MemoryKind::AcpiReclaimable`] even
    /// though the name promises it can be reclaimed. It can be, *after* the tables in it have been
    /// read, and this kernel has not read them; reclaiming it now would hand out the MADT that the
    /// APIC bring-up is going to need.
    pub const fn is_usable_ram(&self) -> bool {
        matches!(self.kind, MemoryKind::Ram)
    }

    /// One past the last byte, saturating rather than wrapping so a malformed entry claiming a size
    /// near `u64::MAX` cannot make a range look empty.
    pub const fn end(&self) -> u64 {
        self.addr.saturating_add(self.size)
    }
}

/// Decode memory-map entry `index` out of `bytes`, which must begin at the map.
///
/// Indexed rather than iterated because the kernel side reads the map through its direct map at a
/// physical address, one entry at a time, and never has a slice of the whole thing. `None` when the
/// bytes end before that entry does.
pub fn memory_entry(bytes: &[u8], index: usize) -> Option<MemoryEntry> {
    let at = index.checked_mul(MEMMAP_ENTRY_LEN)?;
    if bytes.len() < at.checked_add(MEMMAP_ENTRY_LEN)? {
        return None;
    }
    Some(MemoryEntry {
        addr: u64(bytes, at),
        size: u64(bytes, at + 8),
        kind: MemoryKind::from_raw(u32(bytes, at + 16)),
    })
}

/// The size of one `hvm_modlist_entry`.
pub const MODULE_ENTRY_LEN: usize = 32;

/// A module the loader put in RAM for the kernel, which on this system means the initial ramdisk:
/// x86's answer to `/chosen/linux,initrd-start`. Each carries a command line of its own, which
/// nothing here uses. PVH allows several; QEMU writes one per `-initrd`, and the kernel takes the
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Module {
    /// Where the module's bytes are, physically.
    pub addr: u64,
    /// How many bytes there are.
    pub size: u64,
    /// Where this module's own NUL-terminated command line is, or 0. Unused here; decoded because
    /// leaving a field out of a decoder is how a reader comes to believe the structure is smaller
    /// than it is.
    pub cmdline: u64,
}

impl Module {
    /// One past the last byte, saturating for [`MemoryEntry::end`]'s reason: a malformed size near
    /// `u64::MAX` must not make the range look empty to a caller that reserves it.
    pub const fn end(&self) -> u64 {
        self.addr.saturating_add(self.size)
    }
}

/// Decode module `index` out of `bytes`, which must begin at the module list.
///
/// Indexed rather than iterated for [`memory_entry`]'s reason: the kernel reads the list through
/// its direct map, one entry at a time, and never holds a slice of the whole thing. `None` when the
/// bytes end before that entry does.
pub fn module(bytes: &[u8], index: usize) -> Option<Module> {
    let at = index.checked_mul(MODULE_ENTRY_LEN)?;
    if bytes.len() < at.checked_add(MODULE_ENTRY_LEN)? {
        return None;
    }
    Some(Module {
        addr: u64(bytes, at),
        size: u64(bytes, at + 8),
        cmdline: u64(bytes, at + 16),
    })
}

fn u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn u64(bytes: &[u8], at: usize) -> u64 {
    let mut w = [0u8; 8];
    w.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(w)
}

/// Machine-checked proofs over the PVH handoff and the `CPUID` decode (milestone 319 (the crate
/// that parses firmware)): the first structure the x86 kernel reads through a pointer somebody
/// else chose.
///
/// Names: provisional (milestone 319).
#[cfg(kani)]
mod verification;

// ---------------------------------------------------------------------------------------------
// What the CPU itself is, and whether it can run this kernel.
// ---------------------------------------------------------------------------------------------

/// What the boot does about a feature the part does not report.
///
/// Three values where the other architectures need two, because a feature this kernel is built
/// on can be one a hypervisor declines to promise on the guest's behalf: QEMU's TCG never reports
/// the invariant TSC. So the row says which machine it refuses, and the decision reads the
/// hypervisor bit beside the feature bits.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Gate {
    /// The boot refuses without it, on every machine. [`REQUIRED`] is exactly these rows.
    Refuse,
    /// The boot refuses without it on bare metal, where the bit is the part's own promise. Under
    /// a hypervisor (`CPUID.1:ECX[31]` set; see [`HYPERVISOR_CITE`]) the hypervisor owns the
    /// guest's clock and decides what to advertise, so the boot warns on its own line instead.
    /// [`REQUIRED_UNLESS_HYPERVISOR`] is exactly these rows.
    ///
    /// Name: provisional. Spells calef's ruling of 2026-10-22 00:54 UTC on #2534, "refuse a CPU
    /// without invariant TSC unless the hypervisor bit is set; under a hypervisor, warn."
    /// Considered `RefuseOnBareMetal`, which names the case that refuses but not the bit that
    /// decides it.
    RefuseUnlessHypervisor,
    /// Useful to know and not load-bearing. Reported on the boot line and never fatal.
    Report,
}

/// Where `CPUID` says a hypervisor is present, spelled the way [`Row::cite`] spells a feature.
/// Bit 31 of leaf 1's `ECX` reads zero on silicon and is set by a hypervisor that wants its guest
/// to know, which QEMU does under TCG and under KVM.
///
/// Name: provisional. `_CITE` because it is what [`Row::cite`] is for a row: the leaf, register
/// and bit, for a refusal or warning line to print.
pub const HYPERVISOR_CITE: &str = "CPUID.1:ECX[31]";

/// One row of [`TABLE`]: a `CPUID` feature this kernel names, and why it names it.
///
/// The `why` is the same field [`riscv64::Row`](crate::riscv64::Row) carries and for the same
/// reason: a feature on this list with no reason beside it is a feature somebody added because
/// `CPUID` reported it, and the record has to stay small enough to read.
pub struct Row {
    /// The name as Intel's and AMD's manuals spell it.
    pub name: &'static str,
    pub bit: Features,
    /// What the boot does when the part does not report it.
    pub gate: Gate,
    /// Which `CPUID` leaf, register and bit says so, spelled the way the manuals cite it, so a
    /// reader can check the decode against the document without reading [`Isa::decode`].
    pub cite: &'static str,
    pub why: &'static str,
}

/// A set of the features [`TABLE`] names. Same shape as
/// [`riscv64::Extensions`](crate::riscv64::Extensions), deliberately.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Features(u32);

impl Features {
    pub const NONE: Features = Features(0);

    pub const fn union(self, other: Features) -> Features {
        Features(self.0 | other.0)
    }

    pub const fn contains(self, other: Features) -> bool {
        self.0 & other.0 == other.0
    }

    /// What is in `self` and not in `other`. This is how a missing-requirement set is computed.
    pub const fn difference(self, other: Features) -> Features {
        Features(self.0 & !other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// No-execute. `CPUID.80000001H:EDX[20]`.
pub const NX: Features = Features(1 << 0);
/// The `syscall`/`sysret` instruction pair. `CPUID.80000001H:EDX[11]`.
pub const SYSCALL: Features = Features(1 << 1);
/// The TSC ticks at a rate that does not move with the core's frequency or idle state.
/// `CPUID.80000007H:EDX[8]`.
pub const INVARIANT_TSC: Features = Features(1 << 2);
/// `RDSEED`. `CPUID.(EAX=07H,ECX=0):EBX[18]`.
pub const RDSEED: Features = Features(1 << 3);

/// Every `CPUID` feature this kernel names, with the reason. Printed in this order at boot.
///
/// **Three of these are what the kernel is built on rather than what it would like**, and they do
/// not all carry the same [`Gate`]; [`INVARIANT_TSC`]'s row says why, and it is the row worth
/// reading.
pub const TABLE: [Row; 4] = [
    Row {
        name: "nx",
        bit: NX,
        gate: Gate::Refuse,
        cite: "CPUID.80000001H:EDX[20]",
        why: "the hardware bit W^X is made of; without it the page tables' no-execute is a comment",
    },
    Row {
        name: "syscall",
        bit: SYSCALL,
        gate: Gate::Refuse,
        cite: "CPUID.80000001H:EDX[11]",
        why: "the kernel's whole ring-3 entry path; absent, the ABI is #UD on the first call",
    },
    // **Refused on silicon, warned under a hypervisor, and the split is measured rather than
    // argued.** Real silicon has had this bit since about 2008, and xenon, milestone 87 (the
    // x86_64 bare-metal machine), reports it. QEMU's TCG refuses to advertise it at all:
    // `-cpu max,invtsc=on` answers
    // `warning: TCG doesn't support requested feature: CPUID[eax=80000007h].EDX.invtsc [bit 8]`
    // and clears it (QEMU 11, 2026-09-21). It is a KVM-only feature there, because QEMU will not
    // promise rate constancy across a migration it cannot control, and the x86_64 suite runs under
    // TCG. A guest's clock is the hypervisor's to keep, so the hypervisor bit decides which gate
    // applies (calef, 2026-10-22 00:54 UTC, #2534).
    Row {
        name: "invariant-tsc",
        bit: INVARIANT_TSC,
        gate: Gate::RefuseUnlessHypervisor,
        cite: "CPUID.80000007H:EDX[8]",
        why: "the boot measures the TSC's rate ONCE; without this there is no single rate to measure",
    },
    Row {
        name: "rdseed",
        bit: RDSEED,
        gate: Gate::Report,
        cite: "CPUID.(EAX=07H,ECX=0):EBX[18]",
        why: "a hardware entropy source; the service offers a different backend without it",
    },
];

/// The features the kernel refuses to boot without: the [`TABLE`] rows gated [`Gate::Refuse`],
/// and no others.
pub const REQUIRED: Features = gathered(Gate::Refuse);

/// The features the kernel refuses to boot without on bare metal and warns about under a
/// hypervisor: the [`Gate::RefuseUnlessHypervisor`] rows.
///
/// Name: provisional, after the variant it gathers.
pub const REQUIRED_UNLESS_HYPERVISOR: Features = gathered(Gate::RefuseUnlessHypervisor);

/// The [`TABLE`] rows carrying one gate, as a set. `const` so both sets above are derived from the
/// table rather than written twice; a row's gate is then the only thing anyone can get wrong.
const fn gathered(gate: Gate) -> Features {
    let mut acc = Features::NONE;
    let mut i = 0;
    while i < TABLE.len() {
        // `==` on an enum is not const, and `matches!` on a pair needs one arm per value; the
        // discriminant comparison is the shortest thing that works in a const context here.
        if TABLE[i].gate as u8 == gate as u8 {
            acc = acc.union(TABLE[i].bit);
        }
        i += 1;
    }
    acc
}

/// The `CPUID` leaves the kernel reads, as raw register words.
///
/// **Read unconditionally by the caller, believed selectively here.** x86 has no fault for a leaf
/// a part does not implement: `CPUID` above the maximum answers with some *other* leaf's data,
/// which is how an unchecked read turns one part's bits into another feature's answer. Which
/// reads are meaningful is therefore a rule, the rule can be wrong, and a rule that can be wrong
/// belongs where a host test can reach it rather than inside a booting kernel. So the kernel's
/// job is reduced to executing the instruction, and [`Isa::decode`] owns every decision about
/// what the words mean.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct CpuidWords {
    /// Leaf 0, all four registers: `EAX` is the maximum standard leaf, `EBX`/`EDX`/`ECX` are the
    /// vendor string in that order, which is not the register order anyone guesses.
    pub leaf0: [u32; 4],
    /// Leaf 1's `ECX`, for the hypervisor bit. Meaningful only when `leaf0[0] >= 1`.
    pub leaf1_ecx: u32,
    /// Leaf 7 subleaf 0. Meaningful only when `leaf0[0] >= 7`.
    pub leaf7_0: [u32; 4],
    /// Leaf `0x80000000`'s `EAX`: the maximum *extended* leaf, a separate space with its own
    /// maximum. A part can answer every standard leaf and implement no extended leaf at all.
    pub extended_max_leaf: u32,
    /// Leaf `0x80000001`'s `EDX`. Meaningful only when `extended_max_leaf >= 0x8000_0001`.
    pub extended_leaf1_edx: u32,
    /// Leaf `0x80000007`'s `EDX`. Meaningful only when `extended_max_leaf >= 0x8000_0007`.
    pub extended_leaf7_edx: u32,
    /// Leaves `0x80000002` through `0x80000004`, four registers each, in `EAX`/`EBX`/`ECX`/`EDX`
    /// order: the 48-character brand string. Meaningful only when
    /// `extended_max_leaf >= 0x8000_0004`, and a part that answers 2 and 3 but not 4 does not
    /// exist in the manuals, so the three leaves are gated together.
    pub brand: [u32; 12],
}

/// **What this `x86_64` machine is.** One record, populated once at boot, printed at boot.
///
/// The third answer to the question [`riscv64::Isa`](crate::riscv64::Isa) and
/// [`aarch64::Isa`](crate::aarch64::Isa) answer, and the easiest of the three to *ask*: one
/// instruction, architected since 1993, no firmware in the way. What it is not easier at is
/// deciding what the answer means, which is why this record has the same two verbs they do.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Isa {
    /// The 12-character vendor string: `GenuineIntel`, `AuthenticAMD`, or under QEMU whatever
    /// `-cpu` was asked for.
    pub vendor: [u8; 12],
    /// The 48-character brand string the vendor wrote, space-padded, or all zero on a part with
    /// no extended leaf 4. Trailing and leading spaces are the vendor's; see [`Isa::brand_str`].
    pub brand: [u8; 48],
    /// The maximum standard `CPUID` leaf this part answers.
    pub max_leaf: u32,
    /// The maximum extended (`0x8000_0000`-based) leaf. A separate space; see [`CpuidWords`].
    pub extended_max_leaf: u32,
    /// Which of [`TABLE`]'s features the part reports.
    pub features: Features,
    /// Whether `CPUID` says a hypervisor is present ([`HYPERVISOR_CITE`]). Decides what a
    /// [`Gate::RefuseUnlessHypervisor`] row does.
    pub under_hypervisor: bool,
}

impl Default for Isa {
    /// All zero, which is a part that reports no feature and answers no leaf. Deliberately not a
    /// plausible machine: the kernel's record is `None` until discovery runs, so nothing should
    /// ever read this, and if something does the boot line says `cpuid leaves 0..0x0`.
    fn default() -> Isa {
        Isa {
            vendor: [0; 12],
            brand: [0; 48],
            max_leaf: 0,
            extended_max_leaf: 0,
            features: Features::NONE,
            under_hypervisor: false,
        }
    }
}

/// What a machine is missing that this kernel needs. Empty on a machine we can run.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Missing {
    /// Required features the part does not report: [`REQUIRED`]'s, plus
    /// [`REQUIRED_UNLESS_HYPERVISOR`]'s on bare metal.
    pub features: Features,
}

impl Missing {
    pub const fn any(self) -> bool {
        !self.features.is_empty()
    }
}

impl Isa {
    /// Decode the leaves the kernel read. Pure: no `cpuid` here, so this is host-testable against
    /// words captured from real parts and against words no part would report.
    ///
    /// **Every maximum-leaf gate lives here**, which is the point of taking raw words rather than
    /// a closure to call: absence and "the leaf answered with somebody else's data" are the same
    /// bits on the wire, and only the maximum tells them apart.
    pub fn decode(w: &CpuidWords) -> Isa {
        let mut vendor = [0u8; 12];
        vendor[0..4].copy_from_slice(&w.leaf0[1].to_le_bytes());
        vendor[4..8].copy_from_slice(&w.leaf0[3].to_le_bytes());
        vendor[8..12].copy_from_slice(&w.leaf0[2].to_le_bytes());

        let mut brand = [0u8; 48];
        if w.extended_max_leaf >= 0x8000_0004 {
            for (i, word) in w.brand.iter().enumerate() {
                brand[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
        }

        let mut features = Features::NONE;
        if w.extended_max_leaf >= 0x8000_0001 {
            if w.extended_leaf1_edx & (1 << 20) != 0 {
                features = features.union(NX);
            }
            if w.extended_leaf1_edx & (1 << 11) != 0 {
                features = features.union(SYSCALL);
            }
        }
        if w.extended_max_leaf >= 0x8000_0007 && w.extended_leaf7_edx & (1 << 8) != 0 {
            features = features.union(INVARIANT_TSC);
        }
        if w.leaf0[0] >= 7 && w.leaf7_0[1] & (1 << 18) != 0 {
            features = features.union(RDSEED);
        }

        Isa {
            vendor,
            brand,
            max_leaf: w.leaf0[0],
            extended_max_leaf: w.extended_max_leaf,
            features,
            under_hypervisor: w.leaf0[0] >= 1 && w.leaf1_ecx & (1 << 31) != 0,
        }
    }

    /// Can this machine run us? The only verb here a call site branches on: say what is missing,
    /// and stop.
    ///
    /// Silence is absence on x86, unlike on RISC-V: `CPUID` is the part describing itself, and a
    /// part that does not answer leaf `0x80000001` predates these features. On bare metal the
    /// [`Gate::RefuseUnlessHypervisor`] rows count as required; under a hypervisor they move to
    /// [`unpromised`](Isa::unpromised), so every absent row lands in exactly one of the two.
    pub fn missing_requirements(&self) -> Missing {
        let required = if self.under_hypervisor {
            REQUIRED
        } else {
            REQUIRED.union(REQUIRED_UNLESS_HYPERVISOR)
        };
        Missing {
            features: required.difference(self.features),
        }
    }

    /// **What the kernel is built on that this guest's hypervisor does not promise**: the
    /// [`Gate::RefuseUnlessHypervisor`] rows the part does not report, under a hypervisor. Always
    /// empty on bare metal, where those rows refuse instead.
    ///
    /// Separate from [`missing_requirements`](Isa::missing_requirements) because the two call
    /// sites do different things (one refuses, one prints) and because a reader has to be able to
    /// tell "we checked and it is fine" from "we checked, it is not fine, and we booted anyway".
    pub fn unpromised(&self) -> Features {
        if self.under_hypervisor {
            REQUIRED_UNLESS_HYPERVISOR.difference(self.features)
        } else {
            Features::NONE
        }
    }

    /// Does the part implement `RDSEED`, Intel's "read random seed" instruction, which returns bits
    /// straight from the hardware entropy source? The one row of [`TABLE`] anything outside the
    /// boot gate branches on, so it gets a name rather than making a call site spell the bit.
    ///
    /// Name: ratified 2026-09-24 (calef, #1255 review). Refused `has_rdseed` (the mnemonic is a
    /// decoder, not a word).
    pub fn has_random_seed_instruction(&self) -> bool {
        self.features.contains(RDSEED)
    }

    /// The brand string as text, trimmed. `None` when the part answered no extended leaf 4 (the
    /// bytes are all zero) or wrote something that is not UTF-8, which is not a case the manuals
    /// allow and is exactly why it is checked rather than assumed.
    pub fn brand_str(&self) -> Option<&str> {
        if self.brand[0] == 0 {
            return None;
        }
        let end = self
            .brand
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.brand.len());
        core::str::from_utf8(&self.brand[..end]).ok().map(str::trim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version-1 structure with the values QEMU's `q35` actually produced on 2026-08-23, read
    /// back out of the guest. Built by hand rather than captured as a blob so a reader can see
    /// which field is which.
    fn qemu_q35() -> [u8; V1_LEN] {
        let mut b = [0u8; V1_LEN];
        b[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        b[4..8].copy_from_slice(&1u32.to_le_bytes()); // version
        b[8..12].copy_from_slice(&0u32.to_le_bytes()); // flags
        b[12..16].copy_from_slice(&0u32.to_le_bytes()); // nr_modules
        b[16..24].copy_from_slice(&0u64.to_le_bytes()); // modlist
        b[24..32].copy_from_slice(&0u64.to_le_bytes()); // cmdline
        b[32..40].copy_from_slice(&0x000f_5a30u64.to_le_bytes()); // rsdp, in the BIOS area
        b[40..48].copy_from_slice(&0x0000_15b0u64.to_le_bytes()); // memmap
        b[48..52].copy_from_slice(&4u32.to_le_bytes()); // memmap_entries
        b
    }

    /// **The magic is checked before anything else is believed.** This is the first thing the
    /// kernel does with a pointer the loader chose, so a wrong pointer has to be an error rather
    /// than a memory map that looks reasonable.
    #[test]
    fn a_wrong_magic_is_refused_and_reports_what_it_saw() {
        let mut b = qemu_q35();
        b[0..4].copy_from_slice(&0xdead_beefu32.to_le_bytes());
        assert_eq!(
            BootInfo::parse(&b),
            Err(BootInfoError::BadMagic(0xdead_beef))
        );
    }

    /// **Every prefix shorter than the structure is refused, and the shortest one that is long
    /// enough is accepted**, for both of the two sizes a handoff can have.
    ///
    /// The acceptance half is the one an "it returns an error" test leaves out, and it is what
    /// separates `len < V0_LEN` from `len <= V0_LEN`: a guard one byte too tight refuses a
    /// version-0 handoff that is exactly complete, which no malformed input can show.
    fn every_short_prefix_is_refused(bytes: &[u8], fixed_len: usize) {
        for len in 0..fixed_len {
            assert_eq!(
                BootInfo::parse(&bytes[..len]),
                Err(BootInfoError::Truncated),
                "{len} bytes is short of the {fixed_len} this structure needs",
            );
        }
        BootInfo::parse(&bytes[..fixed_len])
            .expect("a structure that is exactly complete must not be refused");
    }

    #[test]
    fn no_prefix_of_a_handoff_reads_past_its_own_end() {
        let mut v0 = qemu_q35();
        v0[4..8].copy_from_slice(&0u32.to_le_bytes());
        every_short_prefix_is_refused(&v0[..V0_LEN], V0_LEN);
        every_short_prefix_is_refused(&qemu_q35(), V1_LEN);
    }

    /// Bytes that end inside the structure are refused rather than read past.
    #[test]
    fn a_truncated_structure_is_refused() {
        let b = qemu_q35();
        assert_eq!(BootInfo::parse(&b[..20]), Err(BootInfoError::Truncated));
        // Long enough for version 0, but the version says 1, so the memory-map fields must be
        // there and are not.
        assert_eq!(BootInfo::parse(&b[..V0_LEN]), Err(BootInfoError::Truncated));
    }

    /// **A version-0 handoff reports no memory map rather than reading past its own end.** The
    /// fields simply do not exist there, and inventing them from whatever follows is the failure
    /// this crate is written to prevent.
    #[test]
    fn a_version_0_structure_reports_no_memory_map() {
        let mut b = qemu_q35();
        b[4..8].copy_from_slice(&0u32.to_le_bytes());
        // Leave plausible garbage where the memory-map fields would be.
        b[40..48].copy_from_slice(&0xdead_beef_dead_beefu64.to_le_bytes());
        b[48..52].copy_from_slice(&99u32.to_le_bytes());
        let info = BootInfo::parse(&b).expect("version 0 is still a valid structure");
        assert_eq!(info.version, 0);
        assert_eq!(info.memmap, 0);
        assert_eq!(info.memmap_entries, 0);
        // The fields that DO exist in version 0 are still read.
        assert_eq!(info.rsdp, 0x000f_5a30);
    }

    /// Every field lands where the structure says it does.
    #[test]
    fn the_fields_are_read_from_the_offsets_the_specification_gives() {
        let info = BootInfo::parse(&qemu_q35()).expect("well-formed");
        assert_eq!(info.version, 1);
        assert_eq!(info.flags, 0);
        assert_eq!(info.module_count, 0);
        assert_eq!(info.rsdp, 0x000f_5a30);
        assert_eq!(info.memmap, 0x0000_15b0);
        assert_eq!(info.memmap_entries, 4);
    }

    /// **The words the boot print uses for each kind of range.**
    ///
    /// The memory map is printed once, at boot, and it is the only place anyone sees what the
    /// firmware said about a range before the frame allocator acts on it. A line that named every
    /// range the same way, or named none of them, would make the one useful thing about that
    /// print (which ranges are RAM and which only look like it) unreadable.
    #[test]
    fn every_kind_of_range_has_its_own_word_for_the_boot_print() {
        for (raw, word) in [
            (1u32, "ram"),
            (2, "reserved"),
            (3, "acpi"),
            (4, "acpi-nvs"),
            (5, "unusable"),
            (6, "disabled"),
            (7, "pmem"),
            (8, "unknown"),
        ] {
            assert_eq!(MemoryKind::from_raw(raw).name(), word, "type {raw}");
        }
    }

    /// Lay `entries` out as a memory map. A fixed-size buffer rather than a `Vec` because this
    /// crate is `no_std` in its test build too, which is the same constraint the kernel side has.
    fn entry_bytes<const N: usize>(entries: &[(u64, u64, u32)]) -> [u8; N] {
        assert_eq!(N, entries.len() * MEMMAP_ENTRY_LEN);
        let mut b = [0u8; N];
        for (i, (addr, size, kind)) in entries.iter().enumerate() {
            let at = i * MEMMAP_ENTRY_LEN;
            b[at..at + 8].copy_from_slice(&addr.to_le_bytes());
            b[at + 8..at + 16].copy_from_slice(&size.to_le_bytes());
            b[at + 16..at + 20].copy_from_slice(&kind.to_le_bytes());
        }
        b
    }

    /// The map q35 produces with `-m 256M`: low RAM, the BIOS hole, high RAM, and the reserved
    /// block below 4 GiB.
    #[test]
    fn a_real_memory_map_decodes_entry_by_entry() {
        let b = entry_bytes::<{ 4 * MEMMAP_ENTRY_LEN }>(&[
            (0x0000_0000, 0x0009_fc00, 1),
            (0x0009_fc00, 0x0000_0400, 2),
            (0x0010_0000, 0x0ff0_0000, 1),
            (0xfffc_0000, 0x0004_0000, 2),
        ]);
        assert_eq!(
            memory_entry(&b, 0),
            Some(MemoryEntry {
                addr: 0,
                size: 0x0009_fc00,
                kind: MemoryKind::Ram
            })
        );
        assert_eq!(memory_entry(&b, 1).unwrap().kind, MemoryKind::Reserved);
        assert_eq!(memory_entry(&b, 2).unwrap().end(), 0x1000_0000);
        assert_eq!(memory_entry(&b, 3).unwrap().addr, 0xfffc_0000);
        assert_eq!(
            memory_entry(&b, 4),
            None,
            "past the end is None, not garbage"
        );
    }

    /// **Only type 1 is allocatable.** `AcpiReclaimable` is the trap: the name says it can be
    /// reclaimed and it cannot be until the tables in it have been read, which this kernel has not
    /// done. Handing it out would hand out the MADT the APIC bring-up needs.
    #[test]
    fn only_plain_ram_is_allocatable_and_acpi_memory_is_not() {
        let b = entry_bytes::<{ 5 * MEMMAP_ENTRY_LEN }>(&[
            (0x1000, 0x1000, 1),
            (0x2000, 0x1000, 3),
            (0x3000, 0x1000, 4),
            (0x4000, 0x1000, 5),
            (0x5000, 0x1000, 4242),
        ]);
        assert!(memory_entry(&b, 0).unwrap().is_usable_ram());
        assert!(
            !memory_entry(&b, 1).unwrap().is_usable_ram(),
            "acpi-reclaimable holds the tables the APIC bring-up has not read yet"
        );
        assert!(!memory_entry(&b, 2).unwrap().is_usable_ram());
        assert!(!memory_entry(&b, 3).unwrap().is_usable_ram());
        assert_eq!(memory_entry(&b, 4).unwrap().kind, MemoryKind::Unknown(4242));
        assert!(
            !memory_entry(&b, 4).unwrap().is_usable_ram(),
            "an unknown type has never meant more RAM than you thought"
        );
    }

    /// A size near `u64::MAX` saturates rather than wrapping, so a malformed entry cannot make a
    /// huge range look like an empty one.
    #[test]
    fn an_absurd_size_saturates_rather_than_wrapping() {
        let b = entry_bytes::<MEMMAP_ENTRY_LEN>(&[(0x1000, u64::MAX, 1)]);
        assert_eq!(memory_entry(&b, 0).unwrap().end(), u64::MAX);
    }

    /// Build a module list of `entries`, each `(paddr, size, cmdline_paddr)`.
    fn module_bytes<const N: usize>(entries: &[(u64, u64, u64)]) -> [u8; N] {
        let mut b = [0u8; N];
        for (i, &(addr, size, cmdline)) in entries.iter().enumerate() {
            let at = i * MODULE_ENTRY_LEN;
            b[at..at + 8].copy_from_slice(&addr.to_le_bytes());
            b[at + 8..at + 16].copy_from_slice(&size.to_le_bytes());
            b[at + 16..at + 24].copy_from_slice(&cmdline.to_le_bytes());
            // The last eight bytes are `reserved` and stay zero, which is what makes an entry 32
            // bytes rather than 24. A decoder that stepped by 24 would read the second module's
            // address out of the first one's tail.
        }
        b
    }

    /// **A module list decodes entry by entry, at the 32-byte stride the structure gives.** The
    /// second entry is what makes this a test rather than a field read: the reserved word at the
    /// end of the first is invisible in a one-module list, and QEMU only ever produces one.
    #[test]
    fn a_module_list_decodes_entry_by_entry() {
        let b = module_bytes::<{ 2 * MODULE_ENTRY_LEN }>(&[
            (0x0100_0000, 0x0002_2000, 0),
            (0x0200_0000, 0x1000, 0x0f00),
        ]);
        assert_eq!(
            module(&b, 0),
            Some(Module {
                addr: 0x0100_0000,
                size: 0x0002_2000,
                cmdline: 0,
            })
        );
        assert_eq!(
            module(&b, 1),
            Some(Module {
                addr: 0x0200_0000,
                size: 0x1000,
                cmdline: 0x0f00,
            })
        );
        assert_eq!(module(&b, 2), None, "past the end is None, not garbage");
    }

    /// Bytes ending inside an entry are refused rather than read past, exactly as a truncated
    /// memory map is.
    #[test]
    fn a_truncated_module_entry_is_refused() {
        let b = module_bytes::<MODULE_ENTRY_LEN>(&[(0x0100_0000, 0x1000, 0)]);
        assert_eq!(module(&b[..MODULE_ENTRY_LEN - 1], 0), None);
    }

    /// A module claiming a size near `u64::MAX` saturates, for the memory map's reason: the kernel
    /// reserves `addr..end()` so the allocator cannot hand out the archive it is about to read, and
    /// a wrapped `end` would reserve nothing at all.
    #[test]
    fn an_absurd_module_size_saturates() {
        let b = module_bytes::<MODULE_ENTRY_LEN>(&[(0x1000, u64::MAX, 0)]);
        assert_eq!(module(&b, 0).unwrap().end(), u64::MAX);
    }

    /// RDSEED is reported only when the bit is set. A leaf that is there and says no is not a yes,
    /// and a union of a set with itself does not clear it.
    #[test]
    fn rdseed_needs_its_own_bit_and_a_union_is_an_or() {
        let mut w = CpuidWords::default();
        w.leaf0[0] = 7;
        assert!(!Isa::decode(&w).features.contains(RDSEED));
        w.leaf7_0[1] = 1 << 18;
        assert!(Isa::decode(&w).features.contains(RDSEED));
        assert_eq!(NX.union(NX), NX);
        assert!(NX.union(SYSCALL).contains(NX) && NX.union(SYSCALL).contains(SYSCALL));
    }
}
