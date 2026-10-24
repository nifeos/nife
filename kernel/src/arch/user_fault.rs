//! **What a user-mode fault was**, decoded the same way on every architecture.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// Which access a user thread was attempting when it faulted.
///
/// `Fetch` is not "a read of an instruction": the two arrive through different exception classes on
/// aarch64 (`0x20` vs `0x24`) and different `scause` codes on RISC-V (12 vs 13), and a test that
/// wanted a data read would be satisfied by an instruction fetch if we collapsed them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UserFaultAccess {
    Read,
    Write,
    Fetch,
}

impl UserFaultAccess {
    const fn encode(self) -> u64 {
        match self {
            UserFaultAccess::Read => 0,
            UserFaultAccess::Write => 1,
            UserFaultAccess::Fetch => 2,
        }
    }

    const fn decode(n: u64) -> Self {
        match n {
            0 => UserFaultAccess::Read,
            1 => UserFaultAccess::Write,
            _ => UserFaultAccess::Fetch,
        }
    }
}

/// **What the last user fault was, in terms both instruction sets can state.**
///
/// `Permission` means the page is mapped and its permission bits refused the access: the
/// privilege boundary working. `Translation` means nothing was mapped. aarch64 is told which by
/// `ESR_EL1`'s fault status code; RISC-V derives it by walking the tables, with the caveat
/// `riscv64::exceptions::user_fault` gives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UserFault {
    /// The address translated, and the permission bits refused this access.
    Permission(UserFaultAccess),
    /// The address had no translation. Nothing was mapped, so nothing was refused.
    Translation(UserFaultAccess),
    /// Not a memory access at all: an illegal instruction, a breakpoint, a misaligned access.
    Other,
}

impl UserFault {
    /// Pack into the `u64` the fault path stores, so recording a fault is one relaxed store and
    /// needs no lock on a path that is already handling a fault. 0 is reserved for "no user thread
    /// has faulted yet", which is why every real code is non-zero.
    pub(crate) const fn encode(self) -> u64 {
        match self {
            UserFault::Permission(a) => 1 + a.encode(),
            UserFault::Translation(a) => 4 + a.encode(),
            UserFault::Other => 7,
        }
    }

    /// Unpack [`encode`](Self::encode). `None` for 0: no user thread has faulted.
    pub(crate) const fn decode(code: u64) -> Option<Self> {
        match code {
            0 => None,
            1..=3 => Some(UserFault::Permission(UserFaultAccess::decode(code - 1))),
            4..=6 => Some(UserFault::Translation(UserFaultAccess::decode(code - 4))),
            _ => Some(UserFault::Other),
        }
    }
}
