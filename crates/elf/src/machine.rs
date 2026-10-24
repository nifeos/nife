//! **Which machines an ELF file may be for**: the `e_machine` values nife runs on, the one this
//! build accepts, and the ones it must refuse.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// `e_machine` for the three architectures nife runs on. All three are live in every build:
/// [`KNOWN_MACHINES`] reads them.
pub(crate) const EM_AARCH64: u16 = 183;

pub(crate) const EM_RISCV: u16 = 243;

pub(crate) const EM_X86_64: u16 = 62;

/// Every `e_machine` nife runs, the one list the host-dependent constants below derive from.
///
/// Adding an architecture is one edit here and one arm of `EXPECTED_MACHINE`; forgetting either
/// fails the build (see [`FOREIGN_MACHINES`]). Exported for callers with a per-machine obligation
/// of their own, such as `crates/elf/tests/fuzz_seed.rs`. See notes/elf/parser.md.
pub const KNOWN_MACHINES: [u16; 3] = [EM_AARCH64, EM_RISCV, EM_X86_64];

/// The machine this build accepts, a compile-time fact: each architecture's build, the host test
/// build included, accepts that architecture's ELFs.
///
/// Every architecture has its own arm; a default arm naming one architecture would catch the next
/// one added (milestone 161 (the x86_64 kernel port: bring up the HAL's third architecture)). See
/// notes/elf/parser.md.
#[cfg(target_arch = "riscv64")]
pub(crate) const EXPECTED_MACHINE: u16 = EM_RISCV;

#[cfg(target_arch = "x86_64")]
pub(crate) const EXPECTED_MACHINE: u16 = EM_X86_64;

#[cfg(not(any(target_arch = "riscv64", target_arch = "x86_64")))]
pub(crate) const EXPECTED_MACHINE: u16 = EM_AARCH64;

/// `EXPECTED_MACHINE`, exported, so a test that forges a header can write the machine that gets it
/// past the machine check to the property under test.
pub const NATIVE_MACHINE: u16 = EXPECTED_MACHINE;

/// The machines nife runs that this build does not: [`KNOWN_MACHINES`] minus [`NATIVE_MACHINE`].
///
/// Tests iterate it to hold the machine check symmetric on every host. `foreign_machines`'s asserts
/// fail the build when `EXPECTED_MACHINE` is missing from `KNOWN_MACHINES`; a known machine with no
/// `EXPECTED_MACHINE` arm compiles on purpose. See notes/elf/parser.md.
///
/// The spelling is **provisional**, minted by milestone 288's lane on 2026-09-14, as are
/// [`KNOWN_MACHINES`] and [`machine_no_nife_build_accepts`]. calef names public items and has not
/// ruled on these three. `script/names` does not enumerate this surface (see design/naming.md's
/// `BUGS`), so this paragraph is the record rather than a `Name:` block.
pub const FOREIGN_MACHINES: [u16; KNOWN_MACHINES.len() - 1] = foreign_machines();

pub(crate) const fn foreign_machines() -> [u16; KNOWN_MACHINES.len() - 1] {
    let mut out = [0u16; KNOWN_MACHINES.len() - 1];
    let mut read = 0;
    let mut written = 0;
    while read < KNOWN_MACHINES.len() {
        if KNOWN_MACHINES[read] != EXPECTED_MACHINE {
            // Before the write, so the build error names the cause rather than an array index.
            assert!(
                written < out.len(),
                "EXPECTED_MACHINE is not in KNOWN_MACHINES: an architecture reached the accept check \
                 without reaching the list it is derived from"
            );
            out[written] = KNOWN_MACHINES[read];
            written += 1;
        }
        read += 1;
    }
    assert!(
        written == out.len(),
        "KNOWN_MACHINES names EXPECTED_MACHINE more than once"
    );
    out
}

/// A machine number no nife build accepts, checked at compile time.
///
/// Returns `machine` after asserting it is in none of [`KNOWN_MACHINES`], so a `const` naming a
/// machine that becomes a target fails the build on every host. See notes/elf/parser.md.
pub const fn machine_no_nife_build_accepts(machine: u16) -> u16 {
    let mut i = 0;
    while i < KNOWN_MACHINES.len() {
        assert!(
            KNOWN_MACHINES[i] != machine,
            "this is one of the machines nife runs, so some build accepts it"
        );
        i += 1;
    }
    machine
}
