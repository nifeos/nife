//! **The std runtime contract**: where a `std` program on nife finds each authority it was given.
//!
//! Name: ratified 2026-10-18 (calef): "Yes". The suffix follows `environment_contract` and
//! `clock_contract`, which the same PAL reads; the stem says which runtime.
//!
//! A native program reads its capabilities by position in its spawner's list. std is written once
//! for every program and cannot, so a `std` program's slots are **fixed by number**, and an empty
//! slot is how std learns it was not given that authority: `std::fs` with slot 4 empty answers
//! `Unsupported`. This crate is the numbers, so the PAL, the progenitor and the kernel test harness
//! cannot disagree; what each slot means to std is `rt.rs`'s documentation. See notes/std.md.

//! ```text
//!   slot 0  untyped     the heap's budget            always
//!   slot 1  endpoint    stdout and stderr            always
//!   slot 2  endpoint    the network stack            only if given the network
//!   slot 3  untyped     socket frames' budget        only if given the network
//!   slot 4  endpoint    one directory                only if given a directory; page at FS_PAGE
//!   slot 5  page frame  the wall clock, READ         only if given a clock; page at CLOCK_PAGE
//!   slot 6  endpoint    the entropy service          only if given randomness
//!   slot 7  page frame  TZ, LANG and TERM, READ      only if given configuration; page at CONFIG_PAGE
//!   slot 8  page frame  the argv, READ               only if its line had words; page at ARGS_PAGE
//!   slot 9  endpoint    a name resolver, badged      only if given names to resolve; page at RESOLVER_PAGE
//!   slot 10 process     its own process, with BIND   only if it may spawn threads (milestone 812)
//!   slot 11 thread      its own first thread, WRITE  with slot 10
//!   slot 12 space       its own address space, READ  for futex waits; without it locks yield
//!   slots 22-28 endpoints  further resolver badges     one per further zone granted, in order
//!   slot 29 endpoint    standard input, badged       only for a program the owner's console types to
//!   slot 30 endpoint    standard input, a byte stream, READ   only if the line redirected or piped its input
//! ```
//!
//! Slots 13 to 21 are not this contract's: `grant_plan` places the installer, its reply, the
//! catalog and the index sources at 15 to 18 for `jig`, and the shell's configuration at 21.

//! **Generated verbatim into the PAL** by `cargo xtask std-src` (as `sys/pal/nife/runtimeproto.rs`),
//! like `abi` and every contract the PAL reads, so this file stays a valid non-root module: no crate
//! attributes survive the copy, and neither does the test module. Every address below is a row of
//! `crates/address_space_map` (milestone 206 (a program image has under 896 KiB),
//! DECISIONS §171 (where a program image starts) option D), restated as a number because the PAL
//! has no crate to name; the test module pins each to its band, the shape `byte_sink_contract`
//! uses for `abi`. A std program is linked at the map's `IMAGE_BASE` and its stack grows down from
//! `STACK_TOP_PAGE`.

//! # BUGS
//!
//! - **The numbers are restated, not derived.** A change to the map that moves the heap or the
//!   runtime windows fails this crate's host tests rather than failing to compile, which is rung
//!   two where the rest of the tree is on rung one.

#![cfg_attr(not(test), no_std)]

/// The untyped budget the global allocator maps heap pages from, lazily, at [`HEAP_BASE`].
pub const MEMORY_REGION_SLOT: u64 = 0;
/// The endpoint `stdout` and `stderr` both send on, in `byte_sink_contract`'s framing.
pub const STDOUT_SLOT: u64 = 1;
/// The network stack's client endpoint, which `std::net` speaks `socket_contract` over. The name
/// means the stack, not a call stack.
pub const STACK_SLOT: u64 = 2;
/// The untyped budget `std::net` mints each socket's shared frame from.
pub const NET_MEMORY_REGION_SLOT: u64 = 3;
/// The file service endpoint that **is** the directory capability: every name `std::fs` sends is
/// resolved under the one directory it reaches. Its shared page is at [`FS_PAGE`].
pub const FS_DIR_SLOT: u64 = 4;
/// A `READ` page frame capability naming the wall-clock page, mapped read-only at [`CLOCK_PAGE`].
pub const CLOCK_SLOT: u64 = 5;
/// The entropy service's request endpoint, `WRITE`: the right to ask for bytes.
pub const ENTROPY_SLOT: u64 = 6;
/// A `READ` page frame capability naming the inert-configuration page, mapped read-only at
/// [`CONFIG_PAGE`].
pub const CONFIG_SLOT: u64 = 7;
/// A `READ` page frame capability naming the argument page, mapped read-only at [`ARGS_PAGE`]
/// (milestone 205 (how a foreign program is told what to do), DECISIONS §170 (how a foreign program is told what to do); the layout is
/// `program_argument_contract`'s). Plain bytes that carry no authority. Provisional, with the layout: where
/// the page sits is one of the things the ruling on that layout fixes.
pub const ARGS_SLOT: u64 = 8;

/// A badged endpoint to the name resolver (milestone 384 (in a capability system the resolver is a
/// grant), §252 (a resolver grant is one zone per client badge)), which `std::net`'s `lookup_host`
/// asks. The badge carries the zone the spawner granted, so what `ToSocketAddrs` can resolve is
/// that zone and nothing else. Provisional, 2026-10-09 (UTC), milestone 801 (packages over the
/// internet).
pub const RESOLVER_SLOT: u64 = 9;

/// **Every slot a resolver badge may be at, in the order std asks them** (milestone 892 (a std
/// program at the prompt holds the network and a resolver); calef, 2026-10-19 (UTC), on #2405:
/// "First zone at slot 9, the rest at fixed slots above 12, probed in order."). One badge per
/// granted zone (§252 (a resolver grant is one zone per client badge)), filled from the front, so
/// the first empty slot ends the list. `lookup_host` moves to the next badge only when the
/// resolver answers that the name is outside this one's zone. Eight, the resolver's
/// `GRANTS_MAX`.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const RESOLVER_SLOTS: [u64; 8] = [RESOLVER_SLOT, 22, 23, 24, 25, 26, 27, 28];

/// **Standard input: the terminal endpoint, badged `line_editor::proto::STDIN_BADGE`** (milestone
/// 892; calef, 2026-10-19 (UTC), on #2405: "a"). std's `Stdin::read` asks it for the line being
/// typed, eight bytes at a time (`OPERATION_READ_LINE_BYTES`); an empty slot is end of input at
/// once. Placed only for a program whose manifest sets `grant_plan::Manifest::console_input`.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const STDIN_SLOT: u64 = 29;

/// **The word std `CALL`s [`STDIN_SLOT`] with**: `line_editor::proto::OPERATION_READ_LINE_BYTES` in
/// the opcode bits, no length. Restated as a number for the PAL, which cannot name `line_editor`;
/// this crate's test pins it to the terminal's own encoding.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const STDIN_READ_REQUEST: u64 = 10 << 56;

/// **Standard input as a byte stream, with `READ`** (milestone 784 (reuse what the tree wrote),
/// item (f2)'s prerequisite; calef, 2026-10-20 16:53 UTC, on #2461: "a"). Placed when the line gave
/// the program an input, a file the shell streams (`prog file`, `prog < file`) or a pipe's read end
/// (`a | prog`). It carries `byte_sink_contract` frames ending in that contract's EOF message.
/// std's `Stdin::read` prefers it to [`STDIN_SLOT`]: an input the line named wins over the console.
///
/// Thirty because this contract's numbers are 0 to 12 and 22 upward, and 13 to 21 are
/// `grant_plan`'s; the next of this contract's own is the one past [`STDIN_SLOT`].
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/784-mdr-pulldown.
pub const STDIN_STREAM_SLOT: u64 = 30;

/// **This program's own process, with `BIND`** (milestone 812 (`std::thread::spawn` runs real
/// threads in one address space)): what `std::thread::spawn` joins a new thread to. Only if the
/// program may have threads; without it, a spawn answers `Unsupported`.
pub const PROCESS_SLOT: u64 = 10;

/// **This program's own first thread, with `WRITE`** (milestone 812): what the main thread sets its
/// own thread pointer through (`ThreadControlBlock::SET_THREAD_POINTER`), which `x86_64` can do no
/// other way. Given with [`PROCESS_SLOT`].
pub const THREAD_SLOT: u64 = 11;

/// **This program's own address space, with `READ`** (milestone 812): the capability futex `WAIT`
/// and `WAKE` are invoked on, so `std`'s locks sleep rather than spin. Without it they yield.
pub const SPACE_SLOT: u64 = 12;

/// **The first slot above every fixed one**, this contract's and `grant_plan`'s: std makes its own
/// capabilities (threads, keys) at or above it (`RETYPE_OBJ`'s floor). A loader places a std
/// program's capabilities at the fixed slot numbers and nowhere else; the reserved fault slot is
/// last in the table and far above them.
pub const SLOTS: u64 = 31;

/// Where the loader maps the page a std program shares with its file service: one file block,
/// carrying a name out on `OPEN` and file bytes both ways on `READ` and `WRITE`.
pub const FS_PAGE: u64 = 0x1100_0000;
/// Where the loader maps the wall-clock page, read-only (`clock_contract`'s layout).
pub const CLOCK_PAGE: u64 = 0x1200_0000;
/// Where the loader maps the inert-configuration page, read-only (`environment_contract`'s layout).
pub const CONFIG_PAGE: u64 = 0x1300_0000;
/// Where the loader maps the argument page, read-only (`program_argument_contract`'s layout). One page
/// above the configuration page, following its pattern. Provisional, like [`ARGS_SLOT`].
pub const ARGS_PAGE: u64 = 0x1400_0000;

/// Where std maps the page it lends the resolver: the program's own page, minted from the socket
/// frames' budget (slot 3) the first time a name is looked up, so a resolver grant is only usable
/// beside the network. Provisional, like [`RESOLVER_SLOT`].
pub const RESOLVER_PAGE: u64 = 0x1500_0000;

/// **Where the loader maps a copy of the image's package catalog**, read-only, for a program that
/// declares `grant_plan::Manifest::catalog` (milestone 809 (the package client becomes a program)):
/// `package_archive::CATALOG`'s `<stem> <digest>` lines, padded with zero bytes to the page's end.
/// The page is the child's own, carved from its region, so its reap takes it back; its capability
/// is at `grant_plan::CATALOG_SLOT`, above the ten fixed slots, for the program to probe. One page
/// above the resolver's. Provisional.
pub const CATALOG_PAGE: u64 = 0x1600_0000;

/// **Where the loader maps a copy of the image's index sources**, read-only, for a program that
/// holds `grant_plan::Manifest::indexes` (milestone 801 (packages over the internet)): the
/// `package_index::SOURCES_ENTRY` text, padded with zero bytes to the page's end, as
/// [`CATALOG_PAGE`] is. Its capability is at `grant_plan::INDEX_SOURCES_SLOT`. One page above the
/// catalog's. Provisional, 2026-10-12 (UTC).
pub const INDEX_SOURCES_PAGE: u64 = 0x1700_0000;

/// Where the heap starts: the start of the address-space map's heap band, and the same value as
/// `memory_region_heap::DEFAULT_BASE`.
pub const HEAP_BASE: u64 = 0x4000_0000;
/// The heap's growth cap: the heap band's whole width. Only a bound on the address range: the
/// budget in [`MEMORY_REGION_SLOT`] is the real, per-program limit.
pub const HEAP_MAX: u64 = 256 * 1024 * 1024;

/// **Stack pages a loader maps for a std program**, where a native child of the progenitor gets
/// twelve. std's startup, its formatting machinery and its collections use far more stack than a
/// hand-written `no_std` program, and the failure mode of too little is a fault in code nobody
/// here wrote. Thirty-two is what the kernel test harness has mapped under every std program since
/// milestone 27 (Rust `std` on the native ABI), plus the one page its own `run` maps, so it is the number every std program on
/// nife has been proven under, rather than a measurement of any one of them.
///
/// Name: provisional.
pub const STACK_PAGES: u64 = 32;

// The stack band holds 4,080 pages less a guard (`address_space_map::MAX_STACK_PAGES`); the test
// below pins the exact figure. This line keeps a nonsense value out of the PAL, which cannot see
// the map.
const _: () = assert!(STACK_PAGES > 0 && STACK_PAGES <= 4079);

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: u64 = 4096;

    /// The stdin request is the terminal's own encoding of its read, which the PAL cannot import.
    #[test]
    fn the_stdin_request_is_the_terminals_read() {
        use line_editor::proto;
        assert_eq!(
            STDIN_READ_REQUEST,
            proto::req(proto::OPERATION_READ_LINE_BYTES, 0)
        );
    }

    #[test]
    fn every_slot_is_distinct_and_inside_the_fixed_range() {
        let slots = [
            MEMORY_REGION_SLOT,
            STDOUT_SLOT,
            STACK_SLOT,
            NET_MEMORY_REGION_SLOT,
            FS_DIR_SLOT,
            CLOCK_SLOT,
            ENTROPY_SLOT,
            CONFIG_SLOT,
            ARGS_SLOT,
            PROCESS_SLOT,
            THREAD_SLOT,
            SPACE_SLOT,
            RESOLVER_SLOTS[0],
            RESOLVER_SLOTS[1],
            RESOLVER_SLOTS[2],
            RESOLVER_SLOTS[3],
            RESOLVER_SLOTS[4],
            RESOLVER_SLOTS[5],
            RESOLVER_SLOTS[6],
            RESOLVER_SLOTS[7],
            STDIN_SLOT,
            STDIN_STREAM_SLOT,
        ];
        for (i, a) in slots.iter().enumerate() {
            assert!(
                *a < SLOTS,
                "slot {a} is outside the {SLOTS} the contract fixes"
            );
            for b in &slots[i + 1..] {
                assert_ne!(a, b, "two authorities share slot {a}");
            }
        }
        assert_eq!(RESOLVER_SLOTS[0], RESOLVER_SLOT);
        assert_eq!(
            STDIN_STREAM_SLOT + 1,
            SLOTS,
            "standard input's byte stream ends the fixed range"
        );
    }

    /// The shared pages must not land on each other, on the heap, or inside the window the
    /// image and stack occupy: a collision there is not a compile error, it is two authorities
    /// aliasing one page in a child.
    #[test]
    fn the_shared_pages_are_page_aligned_and_clear_of_each_other_and_the_heap() {
        let pages = [
            FS_PAGE,
            CLOCK_PAGE,
            CONFIG_PAGE,
            ARGS_PAGE,
            RESOLVER_PAGE,
            CATALOG_PAGE,
        ];
        for (i, a) in pages.iter().enumerate() {
            assert_eq!(a % PAGE, 0, "{a:#x} is not page aligned");
            assert!(
                address_space_map::RUNTIME_WINDOWS.holds(*a, *a + PAGE),
                "{a:#x} is not in the address-space map's runtime windows"
            );
            for b in &pages[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(HEAP_BASE % PAGE, 0);
        assert_eq!(HEAP_MAX % PAGE, 0);
    }

    /// **The restated numbers are the map's**, which is the whole of what stands between this file
    /// and a PAL that disagrees with the loader. See the crate's `BUGS`.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_heap_and_the_stack_are_the_address_space_maps() {
        assert_eq!(HEAP_BASE, address_space_map::HEAP.start);
        assert_eq!(HEAP_MAX, address_space_map::HEAP.bytes());
        assert!(STACK_PAGES <= address_space_map::MAX_STACK_PAGES);
        assert_eq!(address_space_map::MAX_STACK_PAGES, 4079);
    }
}
