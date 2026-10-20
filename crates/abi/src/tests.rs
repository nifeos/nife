use super::Error;

/// **A firmware's refusal reaches the caller as the reason it is** (milestone 805, calef's
/// ruling on §251's amendment item 3). No QEMU machine this tree boots can be made to refuse a
/// reset (`virt` always has PSCI and SRST, and `q35` resets or exits), so the mapping is pinned
/// here: each firmware code to its error, and each error back through the decode a caller
/// uses.
#[test]
fn a_reset_refusal_maps_to_its_reason_and_survives_the_wire() {
    use crate::reboot::{from_psci, from_sbi};
    assert_eq!(from_psci(-1), Error::ResetNotSupported);
    assert_eq!(from_psci(-3), Error::ResetDenied);
    assert_eq!(from_psci(-2), Error::ResetDidNotHappen);
    assert_eq!(from_psci(0), Error::ResetDidNotHappen);
    assert_eq!(from_sbi(-2), Error::ResetNotSupported);
    assert_eq!(from_sbi(-4), Error::ResetDenied);
    assert_eq!(from_sbi(-1), Error::ResetDidNotHappen);
    let all = [
        Error::NoResetMechanism,
        Error::ResetNotSupported,
        Error::ResetDenied,
        Error::ResetDidNotHappen,
    ];
    for (i, e) in all.iter().enumerate() {
        assert_eq!(Error::from_ret(*e as i64), Some(*e), "{e:?}");
        assert!(!all[i + 1..].contains(e), "{e:?} is listed twice");
    }
}

/// `usage::is_known` is the kernel's answer to "is this a record I serve": exactly the seven
/// numbered `SIZE..=CHILDREN`, and nothing past the last.
#[test]
fn usage_records_known_are_exactly_the_seven() {
    use crate::usage;
    for r in [
        usage::SIZE,
        usage::COMMITTED,
        usage::FRAMES,
        usage::RENDEZVOUS,
        usage::ADDRESS_SPACES,
        usage::THREADS,
        usage::CHILDREN,
    ] {
        assert!(usage::is_known(r), "record {r}");
    }
    assert!(!usage::is_known(usage::CHILDREN + 1));
    assert!(!usage::is_known(u64::MAX));
}

/// Every variant round-trips through `from_ret`. The enum's `#[repr(i64)]` discriminants and
/// `from_ret`'s match arms are two lists that must agree, and this crate is the one place a
/// drift between them would not be a compile error: add a variant, forget the match arm, and
/// userspace decodes a real kernel error as `None`. (The new variant must be added to `ALL`
/// for this to guard it; that edit is at least in the same file.)
#[test]
fn every_error_round_trips_through_from_ret() {
    const ALL: &[Error] = &[
        Error::NoSuchSlot,
        Error::WrongObject,
        Error::NotPermitted,
        Error::BadPointer,
        Error::BadMethod,
        Error::BadSyscall,
        Error::OutOfMemory,
        Error::DeviceRefused,
        Error::StillAlive,
        Error::NotSupervised,
        Error::Gone,
        Error::NoResetMechanism,
        Error::ResetNotSupported,
        Error::ResetDenied,
        Error::ResetDidNotHappen,
    ];
    for &e in ALL {
        assert_eq!(Error::from_ret(e as i64), Some(e));
    }
}

/// Success values and out-of-range negatives are not errors. `>= 0` is a result by the ABI's
/// own rule, and an unknown negative must surface as "not an error I know" rather than being
/// silently mapped onto the nearest variant.
#[test]
fn non_errors_decode_to_none() {
    assert_eq!(Error::from_ret(0), None);
    assert_eq!(Error::from_ret(1), None);
    assert_eq!(Error::from_ret(-16), None);
    assert_eq!(Error::from_ret(i64::MIN), None);
}

/// The rights words are an ABI, not an implementation detail: the kernel tests them with `&`
/// and userspace builds masks with `|`, so each must be a distinct single bit, and the exact
/// values are load-bearing on both sides of the syscall boundary. Milestone 85's mutation run
/// showed nothing pinned them (`1 << 1` could become `1 >> 1`, which is zero, and WRITE would
/// silently mean nothing).
///
/// `ENUMERATE` was added to this assertion by the 2026-08-17 documentation sweep. It landed on
/// 2026-08-17 and this test kept pinning three bits while its own comment said *each* must be a
/// distinct single bit, so the newest right was the one nothing held down. **A test that
/// enumerates is a claim about a set, and it rots exactly like prose does.**
#[test]
fn rights_are_distinct_single_bits() {
    use super::rights::{BIND, ENUMERATE, GRANT, READ, WRITE};
    assert_eq!([READ, WRITE, GRANT, ENUMERATE, BIND], [1, 2, 4, 8, 16]);
}

/// **`ALL` is every right and `SPEND_ONLY` is every right but `GRANT`**, as numbers. The kernel's
/// compile-time twin check runs only in a kernel build, so on the host nothing else would see a
/// budget minted with a right missing, or a leaf's budget that could still lend.
#[test]
fn all_is_every_right_and_spend_only_lacks_grant_alone() {
    use super::rights::{ALL, SPEND_ONLY};
    assert_eq!(ALL, 0b1_1111);
    assert_eq!(SPEND_ONLY, 0b1_1011);
}

/// The fault-endpoint slot is a valid slot index. Its value is `CAPABILITY_TABLE_SLOTS - 1` by
/// definition; what can actually be wrong (and what a mutant made wrong invisibly) is the
/// arithmetic putting it outside the capability table, where `CAP_INSERT` would refuse it and every
/// supervised spawn would fail.
#[test]
// Asserting on constants is this test's entire purpose: the constant is the thing a mutant
// rewrites, and the assertion is what notices (milestone 85).
#[allow(clippy::assertions_on_constants)]
fn the_fault_slot_is_inside_the_capability_table() {
    assert!(super::fault::FAULT_EP_SLOT < super::CAPABILITY_TABLE_SLOTS);
    assert_eq!(
        super::fault::FAULT_EP_SLOT,
        super::CAPABILITY_TABLE_SLOTS - 1
    );
}

/// **The futex call admits exactly one flags word**: private and 32-bit (§269 (how threads share a process)). Every other size,
/// and the shared form, is refused.
#[test]
fn the_futex_admits_only_a_private_32_bit_word() {
    use crate::futex::{PRIVATE, SIZE_U8, SIZE_U16, SIZE_U32, SIZE_U64, admitted};
    assert!(admitted(PRIVATE | SIZE_U32));
    assert!(!admitted(SIZE_U32));
    for size in [SIZE_U8, SIZE_U16, SIZE_U64] {
        assert!(!admitted(PRIVATE | size));
    }
}

/// **The JH7110's line status register is at 0x14, read 32 bits wide** (milestone 890 (riscv64 and `x86_64` join the shared boot path)). Every
/// QEMU machine this tree boots has byte registers, so no emulator run can tell a driver that
/// honors the shape from one that assumes QEMU's: radon's console did the second and waited
/// forever on LSR at 0x05. This pins the board's offsets through the word the kernel sends, and
/// that the zero word aarch64 and `x86_64` send is byte access.
#[test]
fn the_jh7110_shape_survives_the_start_word_and_puts_lsr_at_0x14() {
    use crate::uart::RegisterShape;
    const LSR: u64 = 5;
    let board = RegisterShape::from_word(RegisterShape::new(2, 4).to_word());
    assert_eq!(board, RegisterShape::new(2, 4));
    assert_eq!(board.offset(LSR), 0x14);
    assert!(board.is_word_access());
    let qemu = RegisterShape::from_word(0);
    assert_eq!(qemu, RegisterShape::BYTES);
    assert_eq!(qemu.offset(LSR), 0x05);
    assert!(!qemu.is_word_access());
    // A stated 4-byte width at byte stride stays byte access: a 32-bit load at LSR's 0x05 is
    // unaligned and traps, which is why the kernel's driver has the same guard.
    assert!(!RegisterShape::new(0, 4).is_word_access());
}
