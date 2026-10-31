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
/// silently mean nothing). A test that enumerates is a claim about a set, so a new right joins
/// this list in the change that defines it.
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
/// arithmetic putting it outside the capability table, where `CAPABILITY_INSERT` would refuse it and every
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

macro_rules! pinned {
    ($($path:path => $expected:expr,)*) => {
        $(assert_eq!($path, $expected, stringify!($path));)*
    };
}

/// **Every public number the syscall surface spells is pinned as a literal** (milestone 956 (the linked interfaces carry checkable versions) step 4,
/// the enforcement calef ruled 2026-10-29 UTC, PR #2852, question 5: a pinned-values test now, the
/// rustdoc diff when a consumer outside the tree exists). A program built against one generation of
/// this crate carries these numbers in its binary, so a renumber is a break that bumps
/// [`ABI_VERSION`](crate::ABI_VERSION). Changing a line below is the deliberate act a renumber
/// should be: skipping it fails CI, and changing it without bumping the version is what
/// `script/lint`'s "format values move with their constants" check catches.
///
/// A new public number joins this list in the change that defines it. The list is exhaustive
/// over every `pub const` of type `u64` in this crate's modules; the enums and the object
/// modules that are types rather than numbers are pinned beside them
/// (`every_error_is_pinned_to_its_number`, `cycle_counter`'s own tests).
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
#[test]
fn the_syscall_numbers_and_the_first_object_modules_are_pinned() {
    assert_eq!(crate::ABI_VERSION, 1);
    pinned! {
        crate::SYS_EXIT => 0,
        crate::SYS_YIELD => 1,
        crate::SYS_INVOKE => 2,
        crate::SYS_CAPABILITY_DELETE => 3,
        crate::SYS_EXIT_THREAD => 4,
        crate::CAPABILITY_TABLE_SLOTS => 64,
        crate::INTERFACE_ID => 1,
        crate::address_space::MAP_INTO => 0,
        crate::address_space::MAP_RO => 0,
        crate::address_space::MAP_RW => 1,
        crate::address_space::MAP_CODE => 2,
        crate::address_space::LIST => 1,
        crate::address_space::UNMAP => 2,
        crate::address_space::WAIT => 3,
        crate::address_space::WAKE => 4,
        crate::console::WRITE => 0,
        crate::fault::FAULT_EP_SLOT => 63,
        crate::fault::EVENT_FAULT => 1,
        crate::fault::EVENT_EXIT => 2,
        crate::futex::SIZE_U8 => 0,
        crate::futex::SIZE_U16 => 1,
        crate::futex::SIZE_U32 => 2,
        crate::futex::SIZE_U64 => 3,
        crate::futex::SIZE_MASK => 3,
        crate::futex::PRIVATE => 128,
        crate::interrupt::WAIT => 0,
        crate::interrupt::ACK => 1,
        crate::memory_region::MAP => 0,
        crate::memory_region::RETYPE => 1,
        crate::memory_region::RETYPE_OBJ => 2,
        crate::memory_region::SPLIT => 3,
        crate::memory_region::DESTROY => 4,
        crate::memory_region::USAGE => 5,
    }
}

/// The next third of the list above (milestone 956 (the linked interfaces carry checkable versions) step 4).
#[test]
fn the_middle_object_modules_are_pinned() {
    pinned! {
        crate::notification::SIGNAL => 0,
        crate::notification::WAIT => 1,
        crate::notification::POLL => 2,
        crate::notification::BIND => 3,
        crate::notification::BOUND => 2,
        crate::objtype::RENDEZVOUS => 1,
        crate::objtype::ADDRESS_SPACE => 2,
        crate::objtype::THREAD_CONTROL_BLOCK => 3,
        crate::objtype::NOTIFICATION => 4,
        crate::objtype::TIMER => 5,
        crate::objtype::PROCESS => 6,
        crate::page_frame::MAP => 0,
        crate::page_frame::REVOKE => 1,
        crate::page_frame::SLICE => 2,
        crate::port_range::REVOKE => 1,
        crate::process::DESTROY => 0,
        crate::reboot::REBOOT => 0,
        crate::rendezvous::SEND => 0,
        crate::rendezvous::RECEIVE => 1,
        crate::rendezvous::SEND_CAPABILITY => 2,
        crate::rendezvous::RECEIVE_CAPABILITY => 3,
        crate::rendezvous::CALL => 4,
        crate::rendezvous::REAP => 5,
        crate::rendezvous::SURVEY => 6,
        crate::rendezvous::BADGE => 7,
        crate::rendezvous::NO_CAPABILITY => 0xFFFF_FFFF_FFFF_FFFF,
        crate::rendezvous::REPLY_DELIVERED => 1,
        crate::reply::REPLY => 0,
        crate::reply::REPLY_CAPABILITY => 1,
        crate::rights::READ => 1,
        crate::rights::WRITE => 2,
        crate::rights::GRANT => 4,
    }
}

/// The next third of the list above (milestone 956 (the linked interfaces carry checkable versions) step 4).
#[test]
fn the_last_object_modules_are_pinned() {
    pinned! {
        crate::rights::ENUMERATE => 8,
        crate::rights::BIND => 16,
        crate::rights::ALL => 31,
        crate::rights::SPEND_ONLY => 27,
        crate::survey::DONE => 0,
        crate::survey::READY => 1,
        crate::survey::RUNNING => 2,
        crate::survey::BLOCKED => 3,
        crate::survey::DEAD => 4,
        crate::survey::record::STATE => 0,
        crate::survey::record::PLACEMENT => 1,
        crate::survey::record::NO_CPU => 0xFFFF_FFFF_FFFF_FFFF,
        crate::survey::record::CPU_TIME => 2,
        crate::survey::record::PROCESS => 3,
        crate::thread_control_block::CONFIGURE => 0,
        crate::thread_control_block::CAPABILITY_INSERT => 1,
        crate::thread_control_block::START => 2,
        crate::thread_control_block::SET_THREAD_POINTER => 3,
        crate::thread_control_block::SUSPEND => 4,
        crate::thread_control_block::RESUME => 5,
        crate::timer::ARM => 0,
        crate::timer::CANCEL => 1,
        crate::usage::SIZE => 0,
        crate::usage::COMMITTED => 1,
        crate::usage::FRAMES => 2,
        crate::usage::RENDEZVOUS => 3,
        crate::usage::ADDRESS_SPACES => 4,
        crate::usage::THREADS => 5,
        crate::usage::CHILDREN => 6,
        crate::virtio::READ_REGISTER => 0,
        crate::virtio::WRITE_REGISTER => 1,
        crate::virtio::SETUP_QUEUE => 2,
        crate::virtio::NOTIFY => 3,
        crate::virtio::DMA_PHYS_OFFSET => 4088,
    }
}

/// **Every error code is pinned to its number**, and the decode agrees. A program matches these
/// negative values after every syscall, so a renumber is an ABI break (milestone 956 step 7).
#[test]
fn every_error_is_pinned_to_its_number() {
    let all = [
        (Error::NoSuchSlot, -1),
        (Error::WrongObject, -2),
        (Error::NotPermitted, -3),
        (Error::BadPointer, -4),
        (Error::BadMethod, -5),
        (Error::BadSyscall, -6),
        (Error::OutOfMemory, -7),
        (Error::DeviceRefused, -8),
        (Error::StillAlive, -9),
        (Error::NotSupervised, -10),
        (Error::Gone, -11),
        (Error::NoResetMechanism, -12),
        (Error::ResetNotSupported, -13),
        (Error::ResetDenied, -14),
        (Error::ResetDidNotHappen, -15),
    ];
    for (error, number) in all {
        assert_eq!(error as i64, number, "{error:?}");
        assert_eq!(Error::from_ret(number), Some(error), "{error:?}");
    }
}
