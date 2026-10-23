//! **What the confined syscall fuzzer and its conductor agree on** (milestone 779 (fuzz the surface
//! a confined process can reach), provisional).
//!
//! `fixtures/src/confined_syscall_fuzzer.rs` is an EL0 program that issues random syscalls from
//! inside a confined process. `system_tests/src/user/confined_fuzzer_tests.rs` spawns it, lets it
//! take one call at a time, and after each call checks that it reached nothing it was not granted.
//! Both binaries need the same slot layout, the same two conductor words and the same report-page
//! layout, so those live here (AGENTS.md rule 7: anything two binaries agree on is a crate).
//! Every name in this crate but the crate's own is provisional.
//!
//! Name: ratified 2026-10-18 (calef): "ratify `confined_syscall_fuzzer_contract`". It matches the
//! fixture program it serves, `confined_syscall_fuzzer`.

#![no_std]

/// **The conductor's channel**, `READ` only and in slot 0. The fuzzer waits here between calls.
/// The generator never names this slot, and without `GRANT` it cannot be delegated, so the one
/// capability the fuzzer needs to be conducted is the one it cannot fuzz (the fixture's `BUGS`).
/// Slot 0 because `user::run` grants a `Spawn`'s capabilities into slots 0, 1, 2, ... in order,
/// with no way to choose one, and the witness holds only this capability, so the slot it lands
/// in must be the slot the generator excludes.
pub const GO: u64 = 0;
/// The fuzzer's own memory region, `WRITE` only.
pub const REGION: u64 = 1;
/// The outsiders' server endpoint, badged per fuzzer, `WRITE | GRANT`. Every message an outsider
/// takes off it must carry one of the fuzzers' badges.
pub const SERVER: u64 = 2;
/// The endpoint the outsiders send to the fuzzers on, `READ` only.
pub const CLIENT: u64 = 3;
/// An endpoint the two fuzzers share, unbadged, every right.
pub const SHARED: u64 = 4;
/// A notification the two fuzzers share, every right.
pub const NOTE: u64 = 5;
/// One page frame of the fuzzer's own, `READ | WRITE | GRANT`.
pub const GIFT: u64 = 6;
/// How many slots the endowment fills (slot 0 is the conductor's): the generator favors slots
/// below this and never slot 0.
pub const ENDOWED: u64 = 7;

/// Word 0 the conductor sends on [`GO`]: make one call.
pub const GO_STEP: u64 = 1;
/// Word 0 the conductor sends on [`GO`]: exit.
pub const GO_EXIT: u64 = 2;

/// `arg1` at spawn: the role. Two fuzzers share the endowment's endpoints; the witness holds only
/// [`GO`], so everything it tries must be refused.
pub const ROLE_A: u64 = 0;
/// See [`ROLE_A`].
pub const ROLE_B: u64 = 1;
/// See [`ROLE_A`].
pub const ROLE_WITNESS: u64 = 2;

/// Where the report page is mapped, read-write, outside the playground.
pub const REPORT_VA: u64 = address_space_map::service_window(0x5000_0000);
/// The window the generator aims most mapping addresses at, so a `MAP` usually lands somewhere it
/// can succeed and the page oracle can probe every page of it directly.
pub const PLAYGROUND_VA: u64 = address_space_map::service_window(0x5100_0000);
/// Pages in the playground.
pub const PLAYGROUND_PAGES: u64 = 32;

/// The report page, as `u64` word indices. The fuzzer writes the call before making it and the
/// registers after, so a fuzzer that faults mid-call still names the call that killed it.
pub mod report {
    /// How many calls the fuzzer has completed.
    pub const STEPS: usize = 0;
    /// The syscall number of the call in progress or just made.
    pub const NUMBER: usize = 1;
    /// The six argument registers, `ARGS..ARGS + 6`.
    pub const ARGS: usize = 2;
    /// The six registers the kernel handed back, `RETURNS..RETURNS + 6`.
    pub const RETURNS: usize = 8;
    /// Words in use.
    pub const WORDS: usize = 14;
}

pub use abi::SYS_CAP_DELETE as SYS_CAPABILITY_DELETE;
/// The ABI names these with `CAP` today. Both binaries this crate serves spell `capability` out
/// (calef's ruling, 2026-10-06; see `design/naming/capability-spelling-worklist.md`), so each pre-sweep
/// name appears exactly once in this crate, aliased to the spelling the sweep itself proposes,
/// and nowhere else in the milestone's code.
pub use abi::rendezvous::{
    NO_CAP as NO_CAPABILITY, RECEIVE_CAP as RECEIVE_CAPABILITY, SEND_CAP as SEND_CAPABILITY,
};
