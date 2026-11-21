//! **Which machine an ELF file may be for**: the `e_machine` values this crate names, the one
//! [`Elf::parse`](crate::Elf::parse) accepts, and the test support that holds the check symmetric.
//!
//! Name: ratified 2026-11-21 (UTC).

/// `e_machine` for 64-bit Arm. Pass it to [`Elf::parse_for_machine`](crate::Elf::parse_for_machine)
/// to validate an aarch64 program on any host.
///
/// Name: ratified 2026-11-21 (UTC).
#[doc(alias = "EM_AARCH64")]
pub const MACHINE_AARCH64: u16 = 183;

/// `e_machine` for RISC-V, which ELF64 uses for RV64.
///
/// Name: ratified 2026-11-21 (UTC), renamed from `MACHINE_RISCV`.
#[doc(alias = "EM_RISCV")]
pub const MACHINE_RISCV64: u16 = 243;

/// `e_machine` for `x86_64`.
///
/// Name: ratified 2026-11-21 (UTC).
#[doc(alias = "EM_X86_64")]
pub const MACHINE_X86_64: u16 = 62;

/// The machine this build accepts, a compile-time fact: each architecture's build, the host test
/// build included, accepts that architecture's ELFs.
///
/// Every architecture has its own arm; a default arm naming one architecture would catch the next
/// one added.
#[cfg(target_arch = "riscv64")]
pub(crate) const EXPECTED_MACHINE: u16 = MACHINE_RISCV64;

#[cfg(target_arch = "x86_64")]
pub(crate) const EXPECTED_MACHINE: u16 = MACHINE_X86_64;

#[cfg(not(any(target_arch = "riscv64", target_arch = "x86_64")))]
pub(crate) const EXPECTED_MACHINE: u16 = MACHINE_AARCH64;

/// The machine [`Elf::parse`](crate::Elf::parse) accepts in this build: the `e_machine` of the
/// architecture the crate was compiled for.
///
/// A caller that forges a header to test what follows the machine check writes this, so the
/// forgery reaches the property under test on every host. A caller that validates a program for
/// another machine names it with one of the `MACHINE_` constants and
/// [`Elf::parse_for_machine`](crate::Elf::parse_for_machine) instead.
pub const NATIVE_MACHINE: u16 = EXPECTED_MACHINE;

/// Every machine this crate names, the one list the test-only constants below derive from.
///
/// Adding an architecture is one edit here and one arm of `EXPECTED_MACHINE`; forgetting either
/// fails the test build (see [`FOREIGN_MACHINES`]).
#[cfg(test)]
pub(crate) const KNOWN_MACHINES: [u16; 3] = [MACHINE_AARCH64, MACHINE_RISCV64, MACHINE_X86_64];

/// The machines this crate names that this build does not accept: [`KNOWN_MACHINES`] minus
/// [`NATIVE_MACHINE`].
///
/// Tests iterate it to hold the machine check symmetric on every host. `foreign_machines`'s asserts
/// fail the build when `EXPECTED_MACHINE` is missing from `KNOWN_MACHINES`; a known machine with no
/// `EXPECTED_MACHINE` arm compiles on purpose.
#[cfg(test)]
pub(crate) const FOREIGN_MACHINES: [u16; KNOWN_MACHINES.len() - 1] = foreign_machines();

#[cfg(test)]
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

/// A machine number none of [`KNOWN_MACHINES`] is, checked at compile time.
///
/// Returns `machine` after asserting it is none of them, so a `const` naming a machine that joins
/// the list fails the build on every host.
#[cfg(test)]
pub(crate) const fn unknown_machine(machine: u16) -> u16 {
    let mut i = 0;
    while i < KNOWN_MACHINES.len() {
        assert!(
            KNOWN_MACHINES[i] != machine,
            "this is one of the machines the crate names, so some build accepts it"
        );
        i += 1;
    }
    machine
}
