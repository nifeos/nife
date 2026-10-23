//! What RISC-V machine is this? Parsed out of the device tree, because there is no CPUID.
//!
//! The answer is `riscv,isa-extensions` and `riscv,isa-base` (Linux 6.6 and later) or the older
//! single `riscv,isa` string; [`Isa::from_device_tree`] parses both. [`Isa::common`] is the
//! intersection over every hart that may speak for the machine and [`Isa::any`] the union, and
//! `mmu-type` is the narrowest any such hart declares. See `notes/isa-discovery.md`.

//! # BUGS
//!
//! - A `status = "disabled"` hart, and a hart whose own `riscv,isa` disclaims supervisor mode,
//!   contribute nothing to the record: the kernel never runs on either. Both still count in
//!   [`Isa::harts`], an honest node count rather than a roster.
//! - An absent `zicsr` or `zifencei` in a legacy string is not evidence of absence: both left the
//!   base `I` in 2019, and older strings omit them. So neither is in [`REQUIRED`], though the
//!   kernel uses both; `M`, `A` and `C` gate the boot.

use device_tree_blob::{DeviceTreeBlob, Error};

/// The most CPU nodes [`Isa::from_device_tree`] will read. Sixteen is twice the kernel's
/// `MAX_CPUS` and costs one pointer pair each on the stack; a machine with more is reported as
/// truncated ([`Isa::harts`] saturates) rather than silently half-read.
pub const MAX_HARTS: usize = 16;

/// The base integer ISA the machine declares (`riscv,isa-base`, or the prefix of `riscv,isa`).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Base {
    /// No CPU node declared one. Not the same as "not RV64": see [`Isa::described`].
    #[default]
    Unknown,
    Rv32,
    Rv64,
    Rv128,
}

impl Base {
    /// The name to print. Deliberately spelled as the device tree spells it.
    pub fn name(self) -> &'static str {
        match self {
            Base::Unknown => "unknown",
            Base::Rv32 => "rv32i",
            Base::Rv64 => "rv64i",
            Base::Rv128 => "rv128i",
        }
    }

    /// Decode the `rv32` / `rv64` / `rv128` prefix of an ISA string. Ignores everything after it, so
    /// it works on `riscv,isa-base` (`"rv64i"`) and on `riscv,isa` (`"rv64imafdc_..."`) alike.
    pub fn from_isa_string(s: &[u8]) -> Base {
        // `rv128` first: `rv1` is not a prefix of anything else, but checking `rv32`/`rv64` first
        // and falling through would be fine too. Order stated so nobody has to re-derive it.
        if s.starts_with(b"rv128") {
            Base::Rv128
        } else if s.starts_with(b"rv64") {
            Base::Rv64
        } else if s.starts_with(b"rv32") {
            Base::Rv32
        } else {
            Base::Unknown
        }
    }
}

/// The widest page-table format any hart's MMU declares (`mmu-type`).
///
/// Ordered by how much address space each reaches, with [`MmuType::Unknown`] at the bottom so that
/// `mmu >= MmuType::Sv39` is false when nothing declared one. That comparison is the only reason the
/// ordering exists; see [`Isa::missing_requirements`].
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum MmuType {
    /// No CPU node declared `mmu-type`.
    #[default]
    Unknown,
    /// `"riscv,none"`: no MMU at all. A machine we cannot run on.
    None,
    Sv32,
    Sv39,
    Sv48,
    Sv57,
}

impl MmuType {
    pub fn name(self) -> &'static str {
        match self {
            MmuType::Unknown => "not declared",
            MmuType::None => "none",
            MmuType::Sv32 => "sv32",
            MmuType::Sv39 => "sv39",
            MmuType::Sv48 => "sv48",
            MmuType::Sv57 => "sv57",
        }
    }

    /// Decode a `mmu-type` value. The binding spells these `"riscv,sv39"`; the vendor prefix is
    /// stripped here so a tree that omits it (they exist) still parses.
    pub fn from_property(value: &[u8]) -> MmuType {
        let s = value.split(|&b| b == 0).next().unwrap_or(b"");
        let s = s.strip_prefix(b"riscv,").unwrap_or(s);
        match s {
            b"sv57" => MmuType::Sv57,
            b"sv48" => MmuType::Sv48,
            b"sv39" => MmuType::Sv39,
            b"sv32" => MmuType::Sv32,
            b"none" => MmuType::None,
            _ => MmuType::Unknown,
        }
    }
}

/// A set of the extensions this kernel has a reason to name. **Not every extension a machine has**:
/// the QEMU `virt` default declares thirty-odd, and a bitset of all of them would be a list nobody
/// maintains. See [`TABLE`] for the rows and the reason each one is there.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Extensions(u32);

impl Extensions {
    pub const NONE: Extensions = Extensions(0);

    /// Is every extension in `other` present here?
    pub const fn contains(self, other: Extensions) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Extensions) -> Extensions {
        Extensions(self.0 | other.0)
    }

    pub const fn intersection(self, other: Extensions) -> Extensions {
        Extensions(self.0 & other.0)
    }

    /// Everything in `self` that is not in `other`. Used to report *which* requirement is missing
    /// rather than only that one is.
    pub const fn difference(self, other: Extensions) -> Extensions {
        Extensions(self.0 & !other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// One row of [`TABLE`]: an extension this kernel names, and why it names it.
///
/// The `why` is not decoration. An extension with no reason on this list is an extension somebody
/// added because it was in the tree, and the whole discipline of this milestone is that the record
/// stays small enough to read.
pub struct Row {
    /// The name as `riscv,isa-extensions` spells it, lowercase.
    pub name: &'static str,
    pub bit: Extensions,
    /// Does the kernel refuse to boot without it? See the module's BUGS for why `zicsr` and
    /// `zifencei` are used everywhere and still not required.
    pub required: bool,
    pub why: &'static str,
}

/// The one function in this crate that a coverage report will always show as unreached, and
/// deliberately so: every caller below is a `const` initializer, so the compiler folds it and the
/// body never executes at runtime. Left uncalled rather than given a test, because the only
/// assertion available (`bit(3)` is `1 << 3`) restates the body and proves nothing. Contrast
/// [`eid`], which is also const-only and *is* tested, because there the thing that can be wrong is
/// the tag rather than the arithmetic.
const fn bit(n: u32) -> Extensions {
    Extensions(1 << n)
}

pub const I: Extensions = bit(0);
pub const M: Extensions = bit(1);
pub const A: Extensions = bit(2);
pub const C: Extensions = bit(3);
pub const F: Extensions = bit(4);
pub const D: Extensions = bit(5);
pub const H: Extensions = bit(6);
pub const V: Extensions = bit(7);
pub const ZICSR: Extensions = bit(8);
pub const ZIFENCEI: Extensions = bit(9);
pub const SSTC: Extensions = bit(10);
pub const SVINVAL: Extensions = bit(11);
pub const ZICBOM: Extensions = bit(12);

/// Every extension this kernel names, with the reason. Printed in this order at boot.
///
/// The four `required` rows are exactly the target triple: the kernel and userspace are both built
/// for `rv64imac` (`targets/riscv64-unknown-nife.json` says `+m,+a,+c`), so the compiler will
/// emit multiply, atomic and compressed instructions whether or not the machine has them.
pub const TABLE: [Row; 13] = [
    Row {
        name: "i",
        bit: I,
        required: true,
        why: "the base integer ISA; nothing runs without it",
    },
    Row {
        name: "m",
        bit: M,
        required: true,
        why: "multiply/divide, emitted by the compiler (target is rv64imac)",
    },
    Row {
        name: "a",
        bit: A,
        required: true,
        why: "atomics; every lock in the kernel is lr/sc or amo",
    },
    Row {
        name: "c",
        bit: C,
        required: true,
        why: "compressed encodings, emitted throughout (target is rv64imac)",
    },
    Row {
        name: "f",
        bit: F,
        required: false,
        why: "single-precision float; the kernel and userspace are both soft-float (lp64)",
    },
    Row {
        name: "d",
        bit: D,
        required: false,
        why: "double-precision float; same, soft-float, so this is information not need",
    },
    Row {
        name: "h",
        bit: H,
        required: false,
        why: "hypervisor; nothing uses it, and its absence would scope a future milestone",
    },
    Row {
        name: "v",
        bit: V,
        required: false,
        why: "vector; the QEMU rva23s64 model has it and no real board here does",
    },
    Row {
        name: "zicsr",
        bit: ZICSR,
        required: false,
        why: "CSR access; used on every trap, but see BUGS on the legacy string",
    },
    Row {
        name: "zifencei",
        bit: ZIFENCEI,
        required: false,
        why: "fence.i; arch::sync_icache is one, but see BUGS on the legacy string",
    },
    Row {
        name: "sstc",
        bit: SSTC,
        required: false,
        why: "stimecmp: would let the timer arm itself instead of paying an SBI ecall",
    },
    Row {
        name: "svinval",
        bit: SVINVAL,
        required: false,
        why: "fine-grained TLB invalidation: the alternative to sfence.vma's blunt sweep",
    },
    Row {
        name: "zicbom",
        bit: ZICBOM,
        required: false,
        why: "cache-block management: what non-coherent DMA on a real board would need",
    },
];

/// The extensions the kernel refuses to boot without, derived from [`TABLE`] so the two cannot drift.
pub const REQUIRED: Extensions = {
    let mut acc = Extensions::NONE;
    let mut i = 0;
    while i < TABLE.len() {
        if TABLE[i].required {
            acc = acc.union(TABLE[i].bit);
        }
        i += 1;
    }
    acc
};

/// Every bit any row names. Used by the "no duplicate bits" assertion below.
const NAMED: Extensions = {
    let mut acc = Extensions::NONE;
    let mut i = 0;
    while i < TABLE.len() {
        acc = acc.union(TABLE[i].bit);
        i += 1;
    }
    acc
};

// A bit used by two rows would make one of them silently unreportable, and a bit used by none would
// be a constant nothing can ever set. Both are the kind of mistake a table invites and a compile
// error is the cheapest place to catch it.
const _: () = {
    assert!(
        NAMED.0.count_ones() as usize == TABLE.len(),
        "two rows of TABLE share a bit"
    );
    assert!(REQUIRED.contains(I.union(M).union(A).union(C)));
};

/// **The SBI extensions the kernel calls.** Firmware, not silicon, so this is a third source
/// alongside the device tree and the probes: the answer comes from `sbi_probe_extension`.
///
/// Four of the five are called unconditionally, which is why those four are required. That is worth
/// naming precisely because it is the failure that would be silent otherwise: `sbi_remote_sfence_vma`
/// on firmware without RFENCE returns an error nobody reads, the other harts never invalidate, and a
/// migrated thread runs on a stale translation.
///
/// PMU is the fifth and is optional, because it is an instrument rather than a facility the kernel
/// runs on. See [`SbiRow::required`].
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct SbiExtensions(u32);

impl SbiExtensions {
    pub const NONE: SbiExtensions = SbiExtensions(0);

    pub const fn contains(self, other: SbiExtensions) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: SbiExtensions) -> SbiExtensions {
        SbiExtensions(self.0 | other.0)
    }

    pub const fn difference(self, other: SbiExtensions) -> SbiExtensions {
        SbiExtensions(self.0 & !other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// One row of [`SBI_TABLE`]: an SBI extension, its EID, and which kernel function calls it.
pub struct SbiRow {
    pub name: &'static str,
    pub bit: SbiExtensions,
    /// The extension id to pass to `sbi_probe_extension` (base extension, function 3).
    pub eid: usize,
    /// **Does the kernel refuse to boot without it?** [`SBI_REQUIRED`] is the `true` rows and
    /// nothing else, so a `false` row is probed, reported on the boot line, and never fatal.
    ///
    /// The same field [`Row`] carries one source over, for the same reason: an optional facility
    /// and a required one differ in exactly one bit of policy, and splitting them into two tables
    /// would give a reader two places to look for "does this firmware have PMU". PMU is the first
    /// `false` row (milestone 74) and it must be one: a cycle counter is an instrument, and a
    /// kernel that would not boot without an instrument has confused the measurement with the
    /// thing measured.
    pub required: bool,
    pub why: &'static str,
}

pub const SBI_TIME: SbiExtensions = SbiExtensions(1 << 0);
pub const SBI_IPI: SbiExtensions = SbiExtensions(1 << 1);
pub const SBI_RFENCE: SbiExtensions = SbiExtensions(1 << 2);
pub const SBI_HSM: SbiExtensions = SbiExtensions(1 << 3);
pub const SBI_PMU: SbiExtensions = SbiExtensions(1 << 4);

/// An SBI extension id is the big-endian ASCII of a short tag the specification assigns, so the id
/// is **derived from the tag** here rather than written as hex.
///
/// That is not tidiness. The first draft of this table wrote `TIME` as `0x0054_494D`, which is the
/// ASCII of `"TIM"`, and the failure it buys is one of the nastier shapes there is: the probe asks
/// firmware about an extension that does not exist, gets "no", and the kernel refuses to boot on
/// correct firmware. Deriving it makes the tag the only thing anyone can get wrong, and a wrong tag
/// is legible.
pub const fn eid(tag: &str) -> usize {
    let b = tag.as_bytes();
    let mut acc = 0usize;
    let mut i = 0;
    while i < b.len() {
        acc = (acc << 8) | b[i] as usize;
        i += 1;
    }
    acc
}

pub const EID_TIME: usize = eid("TIME");
/// `sPI`, not `IPI`. The specification's own tag, and one of the two that are not simply the
/// extension's human name.
pub const EID_IPI: usize = eid("sPI");
/// `RFNC`, the other one.
pub const EID_RFENCE: usize = eid("RFNC");
pub const EID_HSM: usize = eid("HSM");
/// The performance monitoring unit extension (milestone 74). Optional: it is how the kernel asks
/// firmware to start a hardware counter for CPU cycles, and a machine without it still boots.
pub const EID_PMU: usize = eid("PMU");
/// The base extension, which is how the other four are probed at all: function 3 is
/// `probe_extension(eid)`, and functions 0 through 2 report the spec version and the implementation.
/// The one id that is a number rather than a tag.
pub const EID_BASE: usize = 0x10;

/// Every SBI extension the kernel calls, and the call site.
///
/// Four are required and one is not: see [`SbiRow::required`], and [`SBI_REQUIRED`] for the set the
/// boot refuses to run without.
pub const SBI_TABLE: [SbiRow; 5] = [
    SbiRow {
        name: "TIME",
        bit: SBI_TIME,
        eid: EID_TIME,
        required: true,
        why: "arch::timer::init arms every tick through sbi_set_timer",
    },
    SbiRow {
        name: "IPI",
        bit: SBI_IPI,
        eid: EID_IPI,
        required: true,
        why: "arch::sbi_send_ipi is how one hart reschedules another",
    },
    SbiRow {
        name: "RFENCE",
        bit: SBI_RFENCE,
        eid: EID_RFENCE,
        required: true,
        why: "arch::sbi_remote_sfence_vma; RISC-V has no hardware TLB broadcast",
    },
    SbiRow {
        name: "HSM",
        bit: SBI_HSM,
        eid: EID_HSM,
        required: true,
        why: "arch::cpu_start starts every secondary hart",
    },
    SbiRow {
        name: "PMU",
        bit: SBI_PMU,
        eid: EID_PMU,
        required: false,
        why: "arch::pmu asks firmware to start a hardware counter for CPU cycles",
    },
];

/// The SBI extensions the kernel refuses to boot without: the [`SBI_TABLE`] rows whose
/// [`required`](SbiRow::required) is set, and no others.
pub const SBI_REQUIRED: SbiExtensions = {
    let mut acc = SbiExtensions::NONE;
    let mut i = 0;
    while i < SBI_TABLE.len() {
        if SBI_TABLE[i].required {
            acc = acc.union(SBI_TABLE[i].bit);
        }
        i += 1;
    }
    acc
};

/// What the firmware underneath us is, and what it can do.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Sbi {
    /// SBI spec version, from `sbi_get_spec_version`. `0.0` means the base extension itself did not
    /// answer, which no conforming firmware does.
    ///
    /// **The minor number is 24 bits wide, not 16**, and the major is the seven bits above it. That
    /// is a lopsided split and it is the specification's; a decoder that assumes the obvious 16/16
    /// reads QEMU's `0x0300_0000` (SBI 3.0) as version 0.0 and concludes the firmware never
    /// answered. This one did, on the machine, after the host tests were green.
    pub spec_major: u8,
    pub spec_minor: u32,
    /// `sbi_get_impl_id`. 1 is `OpenSBI`, 0 is BBL, 4 is `RustSBI`; see [`Sbi::impl_name`].
    pub impl_id: u32,
    /// `sbi_get_impl_version`. The implementation's own encoding, printed raw because only the
    /// implementation knows how to read it.
    pub impl_version: u32,
    pub extensions: SbiExtensions,
}

impl Sbi {
    /// **Did the base extension answer at all?**
    ///
    /// No conforming firmware reports spec version 0.0, so a zero here means the `ecall` went
    /// nowhere: SBI v0.1 (2019) predates the base extension, and firmware that old cannot be asked
    /// what it implements.
    ///
    /// This is the same rule the device-tree half follows, one source over: **silence is not
    /// evidence of absence.** Refusing to boot because a probe could not run would refuse on a
    /// machine that works, so [`Isa::missing_requirements`] skips the SBI check entirely when this
    /// is false, and the boot line says the four extensions are unverified rather than present.
    pub fn has_answered(self) -> bool {
        self.spec_major != 0 || self.spec_minor != 0
    }

    /// The name of the firmware, for the boot line. The ids are assigned in the SBI specification's
    /// implementation-id registry; unknown ones print as a number rather than a guess.
    pub fn impl_name(self) -> Option<&'static str> {
        let mut i = 0;
        while i < IMPLEMENTATIONS.len() {
            if IMPLEMENTATIONS[i].id == self.impl_id {
                return Some(IMPLEMENTATIONS[i].name);
            }
            i += 1;
        }
        None
    }
}

/// One row of [`IMPLEMENTATIONS`].
pub struct Implementation {
    /// The id `sbi_get_impl_id` returns.
    pub id: u32,
    pub name: &'static str,
}

/// **The SBI implementations the specification has assigned ids to.**
///
/// A table rather than a `match`, for the same reason [`TABLE`] and
/// [`crate::aarch64::IMPLEMENTERS`] are tables: a duplicated key in a `match` is legal and silent
/// and makes the second arm unreachable, while a duplicate here is the compile error below. Ids are
/// sequential from zero, so the assertion also catches a row inserted in the wrong place.
pub const IMPLEMENTATIONS: [Implementation; 7] = [
    Implementation { id: 0, name: "BBL" },
    Implementation {
        id: 1,
        name: "OpenSBI",
    },
    Implementation {
        id: 2,
        name: "Xvisor",
    },
    Implementation { id: 3, name: "KVM" },
    Implementation {
        id: 4,
        name: "RustSBI",
    },
    Implementation {
        id: 5,
        name: "Diosix",
    },
    Implementation {
        id: 6,
        name: "Coffer",
    },
];

// The registry assigns these sequentially, so "row i has id i" is both the duplicate check and a
// check that nobody inserted a row in the middle and shifted the rest off their real ids.
const _: () = assert!(
    ids_are_sequential(&IMPLEMENTATIONS),
    "IMPLEMENTATIONS is out of order or has a gap; the registry assigns ids sequentially"
);

/// Does row `i` of `table` carry id `i`, for every row? A function for the reason
/// [`crate::aarch64`]'s duplicate check is one: so a test can show it refuses a bad table.
const fn ids_are_sequential(table: &[Implementation]) -> bool {
    let mut i = 0;
    while i < table.len() {
        if table[i].id as usize != i {
            return false;
        }
        i += 1;
    }
    true
}

/// **What this RISC-V machine is.** One record, populated once at boot, printed at boot.
///
/// Filled from three sources, and the field docs say which:
///
/// * the device tree ([`from_device_tree`](Isa::from_device_tree)), which is firmware's claim,
/// * the firmware itself over SBI ([`sbi`](Isa::sbi)), which the kernel fills,
/// * a measurement ([`asid_bits`](Isa::asid_bits)), which the kernel fills after paging is on.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Isa {
    /// Tier 1. The narrowest base any hart declares.
    pub base: Base,
    /// Tier 1. Present on **every** hart that describes itself: the set the kernel may assume.
    pub common: Extensions,
    /// Tier 1. Present on **at least one** hart. Differs from [`common`](Isa::common) exactly when
    /// the machine is heterogeneous, which is the only way to see that from a boot line.
    pub any: Extensions,
    /// Tier 1. The narrowest `mmu-type` any hart declares.
    pub mmu: MmuType,
    /// How many `cpu@` nodes the tree has, saturating at [`MAX_HARTS`].
    pub harts: u16,
    /// How many of those declared an ISA at all, not counting disabled harts or harts whose own
    /// `riscv,isa` disclaims supervisor mode (the module BUGS say why neither may speak).
    /// `common` is an intersection over these, so `described == 0` means `common` is meaningless
    /// and the tree simply did not say.
    pub described: u16,
    /// Did the answer come from the deprecated `riscv,isa` string rather than
    /// `riscv,isa-extensions`? Changes how an absent `zicsr` should be read (see the module BUGS).
    pub legacy_isa_string: bool,
    /// Tier 2, filled by `arch::mmu::init`'s probe. `0` until it runs. This is the fact no emulator
    /// can tell us: QEMU reports 16 implemented `satp.ASID` bits on every CPU model it offers,
    /// including `sifive-u54`, so the number here is only interesting on real silicon.
    pub asid_bits: u8,
    /// Firmware, filled by `arch::isa::init` over SBI.
    pub sbi: Sbi,
}

/// What a machine is missing that this kernel needs. Every field is empty on a machine we can run.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Missing {
    /// The tree declares a base other than RV64. `false` when it declares nothing at all: a tree
    /// that says nothing is not evidence of a machine that cannot run us, and the kernel is
    /// demonstrably executing by the time this is read.
    pub base: bool,
    /// Required extensions (see [`REQUIRED`]) absent from at least one described hart.
    pub extensions: Extensions,
    /// The tree declares an MMU narrower than Sv39. `false` when it declares nothing.
    pub mmu: bool,
    /// Required SBI extensions (see [`SBI_REQUIRED`]) the firmware says it does not implement.
    /// Empty when the base extension did not answer at all (see [`Sbi::has_answered`]).
    pub sbi: SbiExtensions,
}

impl Missing {
    pub const fn any(self) -> bool {
        self.base || !self.extensions.is_empty() || self.mmu || !self.sbi.is_empty()
    }
}

impl Isa {
    /// **Can this machine run us?** The only verb on this record that a call site is meant to
    /// branch on, and it is meant to branch exactly one way: say what is missing, and stop.
    ///
    /// Deliberately silent about what the tree does not mention. A device tree with no
    /// `riscv,isa` at all describes a machine that is nonetheless executing this function, so
    /// treating silence as failure would refuse to boot on hardware that works. Silence is
    /// reported by [`described`](Isa::described) being zero and printed as such.
    pub fn missing_requirements(&self) -> Missing {
        Missing {
            base: self.base != Base::Unknown && self.base != Base::Rv64,
            extensions: if self.described == 0 {
                Extensions::NONE
            } else {
                REQUIRED.difference(self.common)
            },
            mmu: self.mmu != MmuType::Unknown && self.mmu < MmuType::Sv39,
            sbi: if self.sbi.has_answered() {
                SBI_REQUIRED.difference(self.sbi.extensions)
            } else {
                SbiExtensions::NONE
            },
        }
    }

    /// Do the harts differ from each other?
    pub fn is_heterogeneous(&self) -> bool {
        self.common != self.any
    }

    /// Read the CPU nodes. Fills the tier-1 fields and leaves `asid_bits` and `sbi` at their
    /// defaults for the kernel to complete.
    pub fn from_device_tree(dt: &DeviceTreeBlob<'_>) -> Result<Isa, Error> {
        let mut extensions = [None; MAX_HARTS];
        let harts = dt.node_props(b"cpu@", b"riscv,isa-extensions", &mut extensions)?;

        let mut legacy = [None; MAX_HARTS];
        dt.node_props(b"cpu@", b"riscv,isa", &mut legacy)?;

        let mut isa_base = [None; MAX_HARTS];
        dt.node_props(b"cpu@", b"riscv,isa-base", &mut isa_base)?;

        let mut mmu_type = [None; MAX_HARTS];
        dt.node_props(b"cpu@", b"mmu-type", &mut mmu_type)?;

        // Which harts may speak for the machine at all. A disabled hart never runs kernel code,
        // and neither does one whose own ISA string disclaims supervisor mode, so what they
        // declare says nothing about the machine the kernel schedules on; see the module BUGS.
        // Both gates are needed: mainline JH7110 trees exclude the S7 by `status`, and the vendor
        // tree marks it "okay" so only the missing `s` in `rv64imacu` excludes it.
        let mut status = [None; MAX_HARTS];
        dt.node_props(b"cpu@", b"status", &mut status)?;
        let enabled = |i: usize| {
            status[i].is_none_or(crate::cpu_list::is_okay)
                && legacy[i].is_none_or(|s| supervisor_mode_claim(s) != Some(false))
        };

        let mut out = Isa {
            harts: harts.min(u16::MAX as usize) as u16,
            ..Isa::default()
        };
        // The intersection starts full and is narrowed by each hart; the union starts empty. With no
        // described hart the intersection would stay full, which would be a lie, so `described`
        // gates every reader of `common` and the loop below only touches it when a hart speaks.
        let mut common = Extensions(!0);

        for i in 0..harts.min(MAX_HARTS) {
            if !enabled(i) {
                continue;
            }
            let (base, exts) = match (extensions[i], legacy[i]) {
                // The modern stringlist wins where both are present: it is the one firmware is
                // expected to keep current, and QEMU emits both.
                (Some(list), _) => (Base::Unknown, parse_extension_list(list)),
                (None, Some(s)) => {
                    out.legacy_isa_string = true;
                    (Base::from_isa_string(s), parse_isa_string(s))
                }
                (None, None) => continue,
            };

            // `riscv,isa-base` is the modern spelling and beats the prefix of the legacy string.
            let base = match isa_base[i] {
                Some(b) => Base::from_isa_string(b),
                None => base,
            };
            if base != Base::Unknown && (out.base == Base::Unknown || base < out.base) {
                out.base = base;
            }

            common = common.intersection(exts);
            out.any = out.any.union(exts);
            out.described += 1;
        }

        if out.described > 0 {
            // Mask off bits no row names, so an all-ones seed cannot leak into the record.
            out.common = common.intersection(NAMED);
        }

        for (i, m) in mmu_type.iter().enumerate().take(harts.min(MAX_HARTS)) {
            let Some(m) = m else { continue };
            // Same gate as the ISA loop: an unschedulable hart's MMU must not touch what the
            // schedulable harts declare, in either direction. Mainline spells the S7's truth
            // (`riscv,none`, which would read as a machine that cannot run us); the vendor tree
            // spells a lie (`riscv,sv39` on a core with no MMU at all), and neither is a fact
            // about the machine the kernel schedules on.
            if !enabled(i) {
                continue;
            }
            let t = MmuType::from_property(m);
            if t != MmuType::Unknown && (out.mmu == MmuType::Unknown || t < out.mmu) {
                out.mmu = t;
            }
        }

        Ok(out)
    }
}

// `Base` needs an ordering for "narrowest wins" above, and the derive would order it by declaration,
// which puts `Unknown` first. That is what we want (it is handled separately) but it is worth being
// explicit that the ordering is by address width.
impl PartialOrd for Base {
    fn partial_cmp(&self, other: &Base) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Base {
    fn cmp(&self, other: &Base) -> core::cmp::Ordering {
        fn width(b: Base) -> u8 {
            match b {
                Base::Unknown => 0,
                Base::Rv32 => 32,
                Base::Rv64 => 64,
                Base::Rv128 => 128,
            }
        }
        width(*self).cmp(&width(*other))
    }
}

/// Look a name up in [`TABLE`]. Not `pub`: the whole point of the table is that the set of names
/// this kernel cares about is closed.
fn lookup(name: &[u8]) -> Extensions {
    let mut i = 0;
    while i < TABLE.len() {
        if TABLE[i].name.as_bytes() == name {
            return TABLE[i].bit;
        }
        i += 1;
    }
    Extensions::NONE
}

/// Parse `riscv,isa-extensions`: a device-tree stringlist, which is NUL-separated and
/// NUL-terminated, one full extension name per entry.
pub fn parse_extension_list(value: &[u8]) -> Extensions {
    let mut acc = Extensions::NONE;
    for entry in value.split(|&b| b == 0) {
        if !entry.is_empty() {
            acc = acc.union(lookup(entry));
        }
    }
    acc
}

/// Parse the deprecated `riscv,isa` string, e.g. `"rv64imafdc_zicsr_zifencei"`.
///
/// The grammar is not quite as regular as it looks. After the `rv64` prefix comes a run of
/// single-letter extensions with no separator, and after the first `_` comes a run of multi-letter
/// extensions separated by `_`. **`g` is not an extension**, it is an abbreviation for `imafd`
/// plus `zicsr` and `zifencei` (the 2019 spelling), and a machine that writes `rv64gc` is saying
/// exactly what `rv64imafdc_zicsr_zifencei` says.
pub fn parse_isa_string(value: &[u8]) -> Extensions {
    let mut acc = Extensions::NONE;
    let mut parts = strip_base_prefix(value).split(|&b| b == b'_');

    // The single-letter run.
    for &c in parts.next().unwrap_or(b"") {
        let c = c.to_ascii_lowercase();
        if c == b'g' {
            acc = acc
                .union(I)
                .union(M)
                .union(A)
                .union(F)
                .union(D)
                .union(ZICSR)
                .union(ZIFENCEI);
        } else {
            acc = acc.union(lookup(&[c]));
        }
    }

    // The multi-letter run.
    for part in parts {
        if !part.is_empty() {
            acc = acc.union(lookup(part));
        }
    }

    acc
}

/// A legacy `riscv,isa` value up to its NUL, with the `rv32`/`rv64`/`rv128` prefix removed:
/// the single-letter run first, then the `_`-separated multi-letter extensions.
fn strip_base_prefix(value: &[u8]) -> &[u8] {
    let s = value.split(|&b| b == 0).next().unwrap_or(b"");
    for p in [&b"rv128"[..], b"rv64", b"rv32"] {
        if let Some(tail) = s.strip_prefix(p) {
            return tail;
        }
    }
    s
}

/// What a legacy `riscv,isa` string says about supervisor mode. Modern strings spell no privilege
/// letters, so a bare `u` is the witness that the writer was spelling them, and only then is a
/// missing `s` a statement.
///
/// - `Some(true)`: a bare `s` in the single-letter run. The hart claims S-mode.
/// - `Some(false)`: a bare `u` and no `s`. The hart cannot run this kernel, whatever else it says.
/// - `None`: neither letter, which is silence, not absence.
///
/// Only the run before the first `_` is scanned, so `_sstc` never reads as a bare `s`. See
/// `notes/visionfive2/hart-roster-stops.md`.
pub fn supervisor_mode_claim(value: &[u8]) -> Option<bool> {
    let single_letters = strip_base_prefix(value)
        .split(|&b| b == b'_')
        .next()
        .unwrap_or(b"");
    let mut saw_s = false;
    let mut saw_u = false;
    for &c in single_letters {
        match c.to_ascii_lowercase() {
            b's' => saw_s = true,
            b'u' => saw_u = true,
            _ => {}
        }
    }
    match (saw_s, saw_u) {
        (true, _) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
    }
}

/// What `sbi_pmu_counter_get_info` said about one counter (milestone 74 (cycle counters)), decoded
/// from one XLEN-wide word (SBI v3.0, `src/ext-pmu.adoc`, "Get details of a counter (FID #1)"):
/// ```text
///     counter_info[11:0] = CSR (12bit CSR number)
///     counter_info[17:12] = Width (One less than number of bits in CSR)
///     counter_info[XLEN-2:18] = Reserved for future use
///     counter_info[XLEN-1] = Type (0 = hardware and 1 = firmware)
/// ```
/// [`CounterInfo::bits`] adds the one back, and [`CounterInfo::csr`] is `None` for a firmware
/// counter, whose `csr` and `width` the specification says to ignore.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CounterInfo {
    /// `true` when the counter is maintained by the SBI implementation rather than by hardware.
    /// A firmware counter has no CSR and is read with `sbi_pmu_counter_fw_read` (FID #5).
    pub firmware: bool,
    raw_csr: u16,
    raw_width: u8,
}

impl CounterInfo {
    /// Decode one `sbiret.value` from `sbi_pmu_counter_get_info`.
    ///
    /// `XLEN` is 64 here and everywhere this kernel runs (the target triple is `riscv64`), so the
    /// type bit is bit 63 and there is no RV32 case to get wrong.
    pub const fn from_raw(value: u64) -> CounterInfo {
        CounterInfo {
            firmware: value >> 63 != 0,
            raw_csr: (value & 0xfff) as u16,
            raw_width: ((value >> 12) & 0x3f) as u8,
        }
    }

    /// The CSR that reads this counter, or `None` for a firmware counter.
    pub const fn csr(self) -> Option<u16> {
        if self.firmware {
            None
        } else {
            Some(self.raw_csr)
        }
    }

    /// How many bits wide the counter is, or `None` for a firmware counter.
    ///
    /// The specification's field is *one less* than this. Every caller wants the real width (to
    /// know where the counter wraps), so the `+ 1` lives here and nowhere else.
    pub const fn bits(self) -> Option<u32> {
        if self.firmware {
            None
        } else {
            Some(self.raw_width as u32 + 1)
        }
    }
}

/// The `event_idx` for "one CPU cycle", the only event milestone 74 asks for: type 0 (hardware
/// general events) in bits 19:16 and code 1 (`SBI_PMU_HW_CPU_CYCLES`) in 15:0, written as the two
/// fields so each can be checked against the specification. The event counts the cycles the
/// `cycle` CSR counts, which may vary in frequency and stop while the clock is halted.
pub const EVENT_HW_CPU_CYCLES: usize = pmu_event(0, 1);

/// Assemble an `event_idx` from its type and code, per SBI v3.0 `src/ext-pmu.adoc`:
/// `event_idx[19:16] = type`, `event_idx[15:0] = code`.
pub const fn pmu_event(event_type: usize, code: usize) -> usize {
    (event_type << 16) | code
}

/// `SBI_PMU_CFG_FLAG_CLEAR_VALUE`, bit 1: zero the counter as part of configuring it.
pub const PMU_CFG_CLEAR_VALUE: usize = 1 << 1;
/// `SBI_PMU_CFG_FLAG_AUTO_START`, bit 2: start the counter once a matching one is found, so
/// configure and start are one `ecall` instead of two.
pub const PMU_CFG_AUTO_START: usize = 1 << 2;

/// Machine-checked proofs over the RISC-V ISA string and the counter decode (milestone 319 (the
/// crate that parses firmware had no proofs)). The interesting claim is the privilege-letter
/// reading, the one place the grammar is ambiguous and no type catches a wrong reading.
///
/// Names: provisional (milestone 319).
#[cfg(kani)]
mod verification {
    use super::*;

    /// Letters of symbolic single-letter run. Small on purpose: the property is about where the
    /// scan stops, and the scan stops at the first `_` however long the run is.
    const RUN: usize = 4;

    /// **A multi-letter extension can never be mistaken for a privilege letter**: appending one to
    /// an ISA string never changes what it says about privilege modes, for every single-letter run.
    /// QEMU writes `_sstc` and the JH7110 dtsi `_smstateen`, and a scan past the first `_` would
    /// turn the `Some(false)` that refuses the S7 into `Some(true)`;
    /// [`crate::cpu_list::Cpu::is_startable`] has no second opinion.
    /// Falsification: replayable `crates/machine_discovery/falsifications/riscv64.verification.a_multi_letter_extension_is_never_read_as_a_privilege_letter.patch`
    #[kani::proof]
    #[kani::unwind(16)]
    fn a_multi_letter_extension_is_never_read_as_a_privilege_letter() {
        let run: [u8; RUN] = kani::any();
        // The single-letter run is what precedes the first `_`, and it ends at the string's NUL.
        // Assuming neither byte appears in it is what makes `run` a run rather than two of them.
        for &c in &run {
            kani::assume(c != b'_' && c != 0);
        }

        let mut bare = [0u8; 4 + RUN];
        bare[..4].copy_from_slice(b"rv64");
        bare[4..].copy_from_slice(&run);

        // The same string with `_sstc` after it: a real extension from QEMU's own `virt` today,
        // whose name begins with the letter this decode must not see.
        let mut extended = [0u8; 4 + RUN + 5];
        extended[..4 + RUN].copy_from_slice(&bare);
        extended[4 + RUN..].copy_from_slice(b"_sstc");

        assert_eq!(
            supervisor_mode_claim(&bare),
            supervisor_mode_claim(&extended),
            "a multi-letter extension says nothing about privilege modes"
        );
        kani::cover!(
            supervisor_mode_claim(&bare) == Some(false),
            "a run that spells u and not s is the S7, and it must stay refused"
        );
    }

    /// **A counter width that is one less than the answer is widened before the one is added.**
    /// [`CounterInfo::bits`] has the shape of the DMAR's host address width, a firmware field
    /// defined as one less than the value wanted. A `u8` return would overflow at 255, which
    /// `from_raw`'s `& 0x3f` mask keeps out of reach; the proof holds both halves together.
    /// Falsification: replayable `crates/machine_discovery/falsifications/riscv64.verification.a_counter_width_is_widened_before_the_specifications_off_by_one_is_undone.patch`
    #[kani::proof]
    fn a_counter_width_is_widened_before_the_specifications_off_by_one_is_undone() {
        let value: u64 = kani::any();
        let info = CounterInfo::from_raw(value);
        match info.bits() {
            Some(bits) => {
                assert!(!info.firmware);
                // Six bits of field, so one more than the widest it can hold is 64, which is
                // exactly the width of `mcycle` on the machines this kernel runs on.
                assert!((1..=64).contains(&bits));
                assert_eq!(info.csr(), Some((value & 0xfff) as u16));
            }
            None => assert!(info.firmware),
        }
        kani::cover!(info.bits() == Some(64), "a 64-bit counter decodes as 64");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The build-time order check refuses a table whose first row is not id 0.
    #[test]
    fn the_order_check_refuses_a_row_off_its_id() {
        assert!(!ids_are_sequential(&[Implementation { id: 1, name: "" }]));
    }
}
