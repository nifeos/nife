//! The suite's half of `user`: the tests, and the services only they spawn.
//!
//! These 66 modules were declared in `kernel/src/user.rs` under `cfg(test)` until milestone 609
//! (the system tests leave the kernel crate) moved them here, byte for byte. Their declarations
//! came with them, doc comments and gates unchanged, and they are below.
//!
//! **Why the glob.** Every file here was written as a child of the kernel's `user` module, so it
//! says `super::spawn` and `crate::user::initrd` for the kernel's helpers. The glob below puts the
//! kernel `user` module's `pub` items in this module too, so those paths resolve to the same items
//! they always did, and a module declared here (`alloc_service`, say) shadows nothing, because the
//! kernel no longer has one. See `kernel/src/lib.rs`'s `system_test_access` for the other half.

// The imports `kernel/src/user.rs` makes for itself. Its children saw them through `use super::*`,
// private or not; a glob across crates sees only `pub` items, so the suite's half repeats them.
#[allow(unused_imports)]
use elf::Elf;
pub use kernel::system_test_access::user::*;
#[allow(unused_imports)]
use page_frames::{FRAME_SIZE, PageFrame};
#[allow(unused_imports)]
use paging::{Flags, Half, MapError, Mapper};

#[allow(unused_imports)]
use crate::arch::exceptions::{TrapFrame, enter_user};
#[allow(unused_imports)]
use crate::arch::mmu::{self, phys_to_ptr};
#[allow(unused_imports)]
use crate::arch::sync_icache;
#[allow(unused_imports)]
use crate::memory;

/// **A fresh page from `region` holding `code`, ready for the instruction fetcher**: retyped,
/// written through the direct map, and made coherent with `sync_icache`. Panics when the region is
/// spent or `code` does not fit one page.
///
/// Nine tests in five files built a child's code page this way by hand, each with its own
/// `// SAFETY:` comment over the same raw-pointer loop and none checking that the stub fit
/// (milestone 139 (drive the unsafe count down), 2026-10-07 UTC; name provisional). The fact they
/// each restated is `retype_page`'s own postcondition, so it is asserted once here, and the copy is
/// a bounds-checked slice copy rather than pointer arithmetic.
#[cfg(test)]
fn code_page(region: u64, code: &[u32]) -> u64 {
    let phys = crate::memory_region::retype_page(region).expect("no code frame");
    let va = mmu::phys_to_virt(phys);
    // SAFETY: `retype_page` hands back a whole frame carved from `region` for this caller alone,
    // and the direct map names every RAM page, so the slice is the frame and nothing aliases it.
    let page = unsafe {
        core::slice::from_raw_parts_mut(va as *mut u32, FRAME_SIZE as usize / size_of::<u32>())
    };
    page[..code.len()].copy_from_slice(code);
    sync_icache(va, size_of_val(code));
    phys
}

/// **Reading a real disk's partition table, and the difference between listing and holding**
/// (milestone 57).
///
/// The first test is the half of the milestone that is not optional: which blocks of a device are a
/// filesystem is written in the partition table and nowhere else, so this reads one off a virtio-blk
/// device. The table was written by `sgdisk`, in C++, by people who never heard of this project;
/// that provenance is what makes the parse worth asserting.
///
/// The second is the negative control the first would be weaker without. The roster is a read-only
/// mapping, so a program that knows exactly where it is still cannot add a device to it or turn an
/// entry into a handle. `lsblk` plus `parted` cannot make that claim.
#[cfg(all(test, initrd))]
mod disk_tests;

/// **Wall-clock time** (milestone 51 lane A, DECISIONS §43).
///
/// Arch-neutral on purpose: one portable binary carrying both RTC drivers, one host-tested
/// contract, and the machine's own device tree choosing between them, so **both ISAs run literally
/// these tests** rather than two copies that can drift (DECISIONS §19, parity is a gate).
#[cfg(all(test, initrd))]
mod clock_tests;

/// **`date`** (milestone 51; DECISIONS §43, notes/date.md).
///
/// The command that makes the wall clock visible to a person, and the first thing in the tree that
/// exercises `crates/calendar` against a clock the machine actually read. Arch-neutral like the
/// service it reads from: one portable binary over one host-tested contract, so **both ISAs run
/// literally these tests** (DECISIONS §19).
///
/// What these prove that nothing else does: the printed text, parsed back, names the same instant
/// the kernel computes independently from the page; and **an unknown clock produces a sentence
/// rather than 1970 or a panic**, which DECISIONS §43 listed as proven by construction only. It is
/// proven in the guest now, on a board whose RTC works, because the page is the thing under test
/// and a frame nobody has published to is an honest unknown clock.
#[cfg(all(test, initrd))]
mod date_tests;

/// **`printenv`** (milestone 47's environment-variable fork, DECISIONS §111, notes/env-config.md).
///
/// `date`'s own proof, one manifest field over: the program that makes the inert-configuration
/// page visible to a person, and the first spawnable, shell-facing program to declare
/// [`grant_plan::Manifest::config`]. Arch-neutral like the page it reads: one portable binary over
/// one host-tested contract (`environment_protocol`), so **both ISAs run literally these tests**
/// (DECISIONS §19).
///
/// What these prove that nothing else does: a real spawned process, given the real capability
/// `crates/system_initializer`'s wiring would grant, reads the three validated keys back
/// unchanged; a page nobody assembled (the zeroed frame the allocator hands out, `date_tests`'s
/// own unpublished-clock shape) reads as no configuration rather than three empty strings; and a
/// process granted no capability at all answers without touching the page, the same
/// without-touching-memory-it-does-not-hold property `date`'s clock probe already proves.
#[cfg(all(test, initrd))]
mod printenv_tests;

/// **`uuid`** (milestone 111, `components/src/uuid.rs`, notes/entropy.md).
///
/// `printenv`'s proof one manifest field over, and the field is
/// [`grant_plan::Manifest::entropy`]: the first spawnable, shell-facing program that needs random
/// bytes, which before this milestone was authority the *system* could grant and a person could
/// not reach. Arch-neutral, so **both ISAs run literally this test** (DECISIONS §19).
///
/// What it proves that nothing else does: a real spawned process holding an **empty** entropy slot
/// prints no identifier at all, says why, and ends normally. That is the direction the milestone
/// rests on, because randomness is the one authority whose use leaves no trace in what a program
/// does; only removing the capability distinguishes a program that drew bytes from one that
/// invented them. The endowed direction is proven at the real prompt by `script/swish-check`,
/// through the real `crates/system_initializer`, for the reason this module's own
/// `spawn_uuid_holding_no_entropy` records: `Spawn::grants` fills a capability table from zero and
/// cannot place one at the slot a manifest names.
#[cfg(all(test, initrd))]
mod uuid_tests;

/// **A confined EL0 process drives a real, non-virtio DMA device** (milestone 261).
///
/// What these prove that nothing else would: that the NVMe queue mechanics work from ring 3 with
/// no authority over the controller's own registers, that the blk contract a client holds names
/// neither the device nor the doorbells, and that the bytes a client reads back are the bytes it
/// wrote, through a controller an IOMMU confined before it was ever enabled.
///
/// `cfg(initrd)`: see `kernel/build.rs::declare_initrd_cfg`; the server is a packed program, so a
/// build without an archive cannot spawn it.
#[cfg(all(test, initrd))]
mod non_volatile_memory_express_tests;

/// **Milestone 30 (the network stack as a confined component)'s DHCP, TCP and UDP gates over the `e1000e` NIC** (milestone 494 (a driver for
/// the network card a PC actually has)): `net_stack` driving QEMU's 82574L at EL0 through two pages
/// of BAR0 and an IOMMU-confined DMA region, on all three architectures. The first NIC the x86_64
/// leg has.
#[cfg(all(test, initrd))]
mod e1000e_tests;

/// **A host name resolves through a confined resolver, and only inside the zone the client was
/// granted** (milestone 384 (in a capability system the resolver is a grant), §248 (the name
/// resolver is its own confined program)): `net_stack` over the `e1000e`, `name_resolver` as its
/// client, and a test client holding one badged capability to the resolver. All three architectures.
#[cfg(all(test, initrd))]
mod name_resolver_tests;

/// **A client of a network stack reaches the sockets it was handed and no others** (milestone 649
/// (every client of a network stack shares its socket numbers), §255 (each socket is its own
/// capability)): a squatter against a held socket, and a socket handed from one program to another.
/// All three architectures, over the `e1000e`.
#[cfg(all(test, initrd))]
mod net_confinement_tests;

/// **Randomness that an adversary cannot predict** (milestone 56, DECISIONS §44).
///
/// Not arch-gated and not transport-gated: the same binary, the same contract, the same assertions,
/// over virtio-mmio and over PCIe on both ISAs, because a random source that works on one bus is
/// not a random source (§18, §19).
///
/// What these prove that nothing else would: that bytes from a *device* reach a userspace client
/// through a capability that names no device, that consecutive draws are not the same bytes (a
/// stuck source, a re-served buffer, or a driver reading a stale ring all present as repeats), and
/// that the count in a reply is honoured so a caller cannot be handed zeros it mistakes for entropy.
///
/// `cfg(initrd)`: see `kernel/build.rs::declare_initrd_cfg`. Not aarch64/riscv64-only for much
/// longer: `x86_64` picks it up the moment milestone 161 item 4's userspace-compilation hand-off
/// lands (no `x86_64` arm in `declare_initrd_cfg` as of this writing), and every test in this
/// module, including the instruction-backend one below, needs no further change to run there.
/// Confirmed 2026-08-25 by cherry-picking milestone 162's `x86_64` scheduling fix onto that
/// hand-off's branch:
/// `a_client_obtains_unpredictable_bytes_from_rndrrs_with_no_device_at_all` passes under the
/// suite's default `-cpu max` with no code change of its own.
#[cfg(all(test, initrd))]
mod entropy_tests;

/// **What a userspace program is told the counter runs at** (2026-09-21, calef's ruling that a
/// program returns accurate numbers rather than hardcoded ones).
///
/// Its own file rather than a case in `tests.rs`, and arch-neutral on purpose: all three
/// architectures run literally these tests (DECISIONS §19, parity is a gate), even though each
/// learns its rate a different way and two of the three carry it to userspace through a page the
/// third does not need. The question ("does a process time itself against the number the machine
/// stated") is the same on all three, so the assertion is too.
#[cfg(all(test, initrd))]
mod counter_frequency_tests;

/// **A thread's own CPU, read from a page with no syscall** (calef's 2026-09-21 ruling that
/// observing yourself is a page and observing another thread is a selector; that decision's
/// section is on another branch and is named here rather than cited).
///
/// Its own file rather than a case in `tests.rs`, and arch-neutral on purpose: all three
/// architectures run literally these tests (§19 (architectural parity is a tenet)), and here that
/// is more than a convention. The page exists *because* the register that would answer this is
/// x86_64-only, so a suite that proved it on one ISA would prove the wrong thing.
#[cfg(all(test, initrd))]
mod current_cpu_tests;

/// **A secret you can check and cannot read** (milestone 56, the credential half).
///
/// Not arch-gated: the same binaries, the same contract, the same assertions on aarch64 and
/// riscv64, because a credential store that authenticates on one instruction set is not a
/// credential store (§19).
///
/// What these prove that nothing else would:
///
/// - that a **userspace** client with one endpoint and no store gets a correct yes/no over a real
///   Argon2id verification, with the salt drawn from a real virtio-rng;
/// - that the **identical endowment**, used by a program that wants to write the store instead of
///   reading it, cannot;
/// - that the frame a client shares with the service holds **nothing** after the answer, which is
///   the strongest form of "the reply carried no data" that a test can check.
#[cfg(all(test, initrd))]
mod credential_tests;

/// **A login produces a directory and a budget, not a changed identity** (milestone 49).
///
/// What these prove that nothing else would: that a correct identity and secret yield capabilities
/// which actually work (a real `READDIR` through a freshly built `fs_subtree_caretaker`, a real
/// page retyped from a freshly split budget), that a wrong secret is refused and nothing follows
/// the refusal, and that two different identities' channels are independently working and
/// correctly attributed in the service's own audit trail (DECISIONS §109's property, made
/// checkable). See `components/src/login.rs`'s BUGS for what this slice does not attempt: a terminal,
/// per-principal subtree scoping, and wiring into the interactive boot are all named there as
/// follow-on rather than guessed at here.
#[cfg(all(test, initrd))]
mod login_tests;

/// **A principal never exists with a credential and no home, or a home and no credential, for
/// longer than one tool invocation** (milestone 155).
///
/// What these prove that nothing else would: that a fresh identity gets both a working credential
/// (a real `VERIFY` against what was just `PUT`) and a real subtree (a real `MKDIR` that a
/// subsequent `OPENDIR` can descend into), that a duplicate identity's credential half is refused
/// without disturbing an existing subtree, and that re-running the tool against a subtree that
/// already exists (`EEXIST`) is recovery rather than a second failure. See
/// `components/src/identity_provisioner.rs`'s own module docs for the ordering argument these tests hold
/// it to.
#[cfg(all(test, initrd))]
mod identity_provisioning_tests;

/// **An NTP client may propose a time and may not set one** (milestone 51).
///
/// The milestone's demonstrable claim, and the one Unix cannot make: `ntpd` runs as root and may set
/// the clock to anything. Here the network-facing component holds an endpoint the clock service is
/// free to refuse, and holds no mapping of the page the offset lives in. These tests take that
/// apart: the happy path lands as a **proposal**, a reply that fails validation moves nothing, a
/// proposal outside the policy's bounds is refused **by the service**, and a write aimed straight at
/// the clock page kills the process.
///
/// Not arch-gated: three portable binaries, the same assertions on aarch64 and riscv64
/// (DECISIONS §19).
#[cfg(all(test, initrd))]
mod ntp_tests;

/// **The untyped-backed userspace heap** (milestone 27): spawn the `allocator_exerciser` workload, the
/// first program that links `extern crate alloc`, with an untyped budget (slot 0) and a report
/// endpoint (slot 1). The program wires `user_mode_runtime::heap` as its global allocator, churns
/// `Vec`/`String`/`BTreeMap` with frees in arbitrary order, asserts every intermediate result
/// itself (a wrong value faults), and reports a magic word plus how many bytes of heap it
/// committed. Portable: the same test runs the riscv64 ELF on riscv and the aarch64 ELF on
/// aarch64, out of each arch's own initrd.
#[cfg(test)]
pub mod alloc_service;

#[cfg(all(test, initrd))]
mod heap_tests;

/// **The compositor: one screen, several mutually distrusting clients** (milestone 33, rung two of
/// the display ladder).
///
/// Arch-neutral, like rung one and for the same reasons: two portable binaries in both archives, one
/// host-tested contract crate, and an isolation property that is the kernel's own (mappings and
/// capabilities), so **both ISAs run literally these tests**.
///
/// Three of the four tests do not need a GPU at all, and take a **kernel stand-in for the display**
/// instead. That is not a shortcut, it is two things at once: it keeps four device bring-ups down to
/// one, and it makes the flush rectangles *observable*, which is how "a one-window redraw does not
/// cost a whole screen" becomes an assertion instead of a claim. It is also the swappable-component
/// story falling out for free: the compositor cannot tell whether the endpoint it flushes to is a
/// virtio-gpu driver or the kernel.
///
/// **These tests run before `display_tests`** (`compositor_tests` sorts first), which matters for the
/// host-side scanout check: the composed screen goes up first and rung one's pattern last, and
/// `cargo xtask` looks for both in that order. See notes/compositor.md.
#[cfg(all(test, initrd))]
mod compositor_tests;

/// **The display: virtio-gpu, a confined driver, and a client that draws** (milestone 29, rung one
/// of the display ladder).
///
/// Arch-neutral on purpose, unlike most of the device tests here: the driver and the client are
/// portable binaries in both archives, the transport is the same PCIe seam on both boards, and the
/// contract is one host-tested crate, so **both ISAs run literally this test** rather than two
/// copies of it that can drift (DECISIONS §19: parity is a gate).
#[cfg(all(test, initrd))]
mod display_tests;

/// **Rust `std` on the native ABI** (milestone 27): spawn the `std_exerciser` demo, an ordinary Rust
/// program (no `no_std`, no attributes) built for the `*-unknown-nife` custom target with std's
/// PAL implemented directly over the capability ABI. It gets the same two grants as `allocator_exerciser`,
/// an untyped budget (slot 0, which the std `GlobalAlloc` draws the heap from) and an endpoint
/// (slot 1, which `println!` SENDs to). Its stdout is a fixed, deterministic transcript the test
/// reassembles from the endpoint and checks byte for byte. Portable: the aarch64 ELF runs on
/// aarch64 and the riscv64 ELF on riscv, out of each arch's own initrd.
///
/// **Since milestone 51 it is also granted a wall clock**: a clock service is started first, and
/// the program gets that service's page as a `PageFrame` capability with `READ` in slot 5 plus a
/// read-only mapping of it. That is the whole of a std program's wall-clock authority, and it is
/// what turns `SystemTime::now()` from "1970 plus uptime" into a real answer (DECISIONS §43).
#[cfg(test)]
pub mod std_service;

#[cfg(all(test, initrd))]
mod std_tests;

/// **Unmodified `ripgrep` from crates.io** (milestone 121), which skips unless somebody ran
/// `helpers/build-ripgrep.sh`. Every ISA the `std` port ships on, per DECISIONS §19, which is all
/// three since milestone 184 built `x86_64-unknown-nife`.
#[cfg(all(test, initrd))]
mod ripgrep_tests;

/// **Somebody else's C, unmodified, on nife's C library**: SQLite's `speedtest1` and ioping
/// (milestone 835 (a C library, stage 1: files, clock and memory)). Each skips unless somebody ran
/// its `helpers/build-<name>.sh`. Every ISA, per DECISIONS §19.
#[cfg(all(test, initrd))]
mod c_program_tests;

/// **A TLS crypto provider's primitives against published test vectors**, for milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets), on
/// `ripgrep`'s exact terms: present only when `helpers/build-cryptography-exerciser.sh` has been
/// run, because the crates under it are a dependency decision DECISIONS §46 (thin primitives or whole subsystems; we write everything in between) makes calef's. Every
/// ISA, per DECISIONS §19 (architectural parity is a tenet; the targets are aarch64, riscv64 and x86_64),
/// and here the x86_64 leg is the one that matters most: it is the only
/// target whose build forces the portable implementations.
#[cfg(all(test, initrd))]
mod cryptography_tests;

/// **A TLS 1.3 handshake against a peer that is not ours, trusting one pinned root**, for milestone
/// 501 (a TLS client that speaks to one pinned peer), on `cryptography_tests`' terms: present only
/// when `helpers/build-pinned-tls-exerciser.sh` has been run. Every ISA, over the `e1000e` each
/// runner attaches, per DECISIONS §19 (architectural parity is a tenet; the targets are aarch64,
/// riscv64 and x86_64).
#[cfg(all(test, initrd))]
mod pinned_tls_tests;

/// **A package fetched through the distribution's index, by host name, over pinned TLS**, for
/// milestone 801 (packages over the internet), on `pinned_tls_tests`' terms: present only when
/// `helpers/build-pinned-tls-exerciser.sh` has been run. Every ISA, over the `e1000e`.
#[cfg(all(test, initrd))]
mod package_index_tests;

/// **Capability delegation: authority moves between processes at runtime.**
///
/// Every other capability in nife is minted by the kernel and handed to a process at spawn.
/// That made the kernel a central authority-granting oracle, which is the ambient-authority shape
/// §10 argued against, just relocated. A capability system's defining move is that a process can
/// pass authority it holds to another process, narrowing it on the way, and only if it was trusted
/// to (`GRANT`). This wires the smallest scenario that exercises all three: a *granter* delegates a
/// resource capability to a *receiver* over a channel, narrowed to `WRITE` (no `GRANT`); the
/// receiver uses it and then cannot pass it on. See fixtures/src/hello.rs `granter()/receiver()`.
/// **`PageFrame` capabilities: shared memory a process holds, maps, and delegates.**
///
/// The payoff of delegation applied to memory. A *producer* retypes a page out of its own untyped
/// into a `PageFrame` capability, maps it, writes into it, and delegates a READ-only view to a
/// *consumer*, which maps the same physical page and reads what the producer wrote. The kernel
/// copies nothing and pre-arranges nothing: the two processes compose the sharing themselves, and
/// the read-only narrowing means the consumer can look but not write. See fixtures/src/hello.rs
/// `page_frame_producer()/page_frame_consumer()`.
// Test scaffolding: the `tests` module below is the only caller, and it runs on both ISAs now
// (milestone 19's user-test port). This wiring was already portable; it was compiled out on riscv64
// only because its consumer was.
#[cfg(test)]
pub mod page_frame_service;

// Test scaffolding: the `tests` module below is the only caller, and it runs on both ISAs now
// (milestone 19's user-test port). This wiring was already portable; it was compiled out on riscv64
// only because its consumer was.
#[cfg(test)]
pub mod delegation_service;

/// **Milestone 19a: a process mints an endpoint from its own memory, at EL0.** The maker holds
/// an untyped budget and a channel; the peer holds the channel and a report line. Everything
/// else, the endpoint itself included, is created at runtime by the maker out of its own pages
/// and delegated. See fixtures/src/hello.rs `ep_maker()/ep_user()`.
// Test scaffolding: the `tests` module below is the only caller, and it runs on both ISAs now
// (milestone 19's user-test port). This wiring was already portable; it was compiled out on riscv64
// only because its consumer was.
#[cfg(test)]
pub mod retype_ep_service;

/// **A process composed from two capabilities, at EL0** (milestone 19b (run a real workload),
/// extended by §185 (what carries the claim that userspace composes a process from an authority you
/// can count on one hand)). A memory region and a report line, and the archive to read a child out
/// of; everything else it constructs, the child included. See
/// `fixtures/src/process_composition_witness.rs`, which milestone 291 (thirty-one programs wearing
/// one name) split out of the `hello` multiplexer.
// `cfg(initrd)`: the witness reads its child out of the archive. See `kernel/build.rs::declare_initrd_cfg`.
#[cfg(all(test, initrd))]
pub mod process_composition_service;

/// **Milestone 12: Call/Reply, at EL0.** One request endpoint, a server that answers a caller it was
/// never wired to, and the one-shot reply capability proven across the boundary. See
/// fixtures/src/hello.rs `call_server()/call_client()`.
// Test scaffolding: the `tests` module below is the only caller, and it runs on both ISAs now
// (milestone 19's user-test port). This wiring was already portable; it was compiled out on riscv64
// only because its consumer was.
#[cfg(test)]
pub mod call_service;

/// **A reply that carries a capability, at EL0** (§255 (each socket is its own capability),
/// milestone 649 (every client of a network stack shares its socket numbers)). See fixtures/src/carried_capability_server.rs.
#[cfg(test)]
pub mod carried_capability_service;

/// **Milestone 13: revoke a frame, at EL0.** One process with an untyped budget retypes a frame,
/// maps it, revokes it, and reports whether the revoke deleted its own capability. See
/// fixtures/src/hello.rs `revoke_demo()`.
// Test scaffolding: the `tests` module below is the only caller, and it runs on both ISAs now
// (milestone 19's user-test port). This wiring was already portable; it was compiled out on riscv64
// only because its consumer was.
#[cfg(test)]
pub mod revoke_service;

/// **The in-kernel userspace suite, on both instruction sets** (milestone 19's user-test port).
///
/// It was aarch64-only for most of this project's life, and the module comment used to say the
/// reason was the tests: "every test drives a hand-written aarch64 program through `exec` and reads
/// aarch64 fault registers". That was true, and it was the wrong thing to fix. The tests were fine;
/// their *scaffolding* was aarch64. Three things moved and the tests came along unchanged:
///
/// 1. The hand-assembled programs became real ELFs the toolchain builds for both targets (the
///    `outlaw` binary and the `interrupt_ignorer` that already existed). See the note above
///    `OUTLAW_ROUND_TRIP`.
/// 2. `ESR`/`FAR` became `arch::UserFault`, the same fact in words RISC-V can say, which is what
///    keeps "a PERMISSION fault at exactly this address" assertable rather than softened to "a fault
///    happened".
/// 3. `hello`, which carries the milestone 7-19 role catalogue, was found to build for RISC-V once
///    six syscalls it had hand-rolled in aarch64 `asm!` were routed through `user_mode_runtime`, which already
///    had portable versions of all six.
///
/// **What is still gated, and why, is written at each test rather than here**, because a blanket
/// module comment is how the old claim survived past the point of being true. Two kinds of gate
/// appear below: a property that has no RISC-V analogue at all (`el1_runs_on_sp_el1`), and a
/// property whose RISC-V twin lives in `riscv_virtio_tests` and would be duplicated rather than
/// gained. See notes/riscv-parity-scope.md.
#[cfg(all(test, initrd))]
mod tests;

/// **Forcible teardown: `DESTROY` tears a runaway down** (DECISIONS §16 amendment, §24's second-`^C`
/// tier). A child spinning at EL0, never yielding and never checking an endpoint, cannot be waited
/// out; its region's owner must be able to reclaim it anyway. This is the one cross-ISA test in this
/// file, because the mechanism it proves is pure portable scheduler logic: the only per-architecture
/// part is the single spin instruction (`b .` / `j .`), and the whole capability dance around it is
/// the same code both ISAs run. It is separate from the aarch64 module above precisely so it can run
/// on both, which the parity gate (DECISIONS §19) asks of every kernel capability.
#[cfg(test)]
mod force_kill_tests;

/// **A first process that gives its authority away, and a supervision tree that outlives it** (milestone 22
/// phase B.2).
///
/// Cross-ISA, because every piece is portable: the whole tree is four ordinary user programs
/// (`root_supervisor`, `spawner`, `sub_server_supervisor`, `flaky`) built out of the capability verbs, and the kernel's only
/// part is the fault endpoint phase A already built.
///
/// The kernel spawns `root_supervisor` the way it spawns the progenitor: the archive mapped read-only, one untyped
/// budget, one report endpoint. `root_supervisor` then builds a construction sub-server and a supervisor, hands
/// each exactly what it needs, and **deletes its own budget**. From then on the tree runs without it:
/// the sub-server crashes, its supervisor hears about it, reaps it through the spawner, and asks for a
/// replacement, which runs and exits cleanly. The progenitor could not have done any of that, and that is what
/// these two tests prove.
#[cfg(all(test, initrd))]
mod authority_tests;

/// **The interactive boot's half of the same idea: a job's memory comes home** (milestone 22, the
/// increment that migrated the hand-validated boot path).
///
/// The tree above proves a first process that can hand its construction authority away entirely. The
/// interactive progenitor cannot: it stays the shell's spawn service, so it must keep *some* budget. What
/// it can do instead is keep a **bounded** one and make it renewable, which is what these two tests
/// are about. Every job the prompt spawns is built in a region split off that pool and born
/// supervised, and `job_undertaker` (one endpoint capability, no memory at all) collects the corpse
/// through `Rendezvous::REAP`, which returns the region to **The progenitor's** pool under §13 region ownership.
///
/// The pair is a control and a claim, in that order: three jobs exhaust the pool when nothing
/// collects, and twelve go through the same pool when `job_undertaker` does. Neither is a timing
/// argument; the assertion in both is which budget the pages are in.
///
/// Cross-ISA, because every piece is portable: `job_undertaker` is an ordinary program in both archives
/// and the reap authorization reads two TCB fields.
#[cfg(all(test, initrd))]
mod job_undertaker_tests;

/// **A memory-unsafe C component, confined** (milestone 36, DECISIONS §31).
///
/// The thesis (§14) is a verified core that confines unverified workloads, and C is the most
/// unverified workload available: no bounds checks, no borrow checker, nothing between a bad index
/// and a store. So this is not a dilution of the claim, it is the sharpest available test of it. The
/// contrast is concrete rather than rhetorical: in a monolith, C filesystem or driver code with this
/// bug is a kernel memory corruption; here it is a page fault in an unprivileged process, and its
/// supervisor restarts it.
///
/// **What is under test is the seam, not the C.** `fixtures/c/c_seam.c` is deliberately throwaway: 150
/// lines, one honest function and two one-line bugs. What the milestone de-risks is everything around
/// it, before a real foreign component (libghostty-vt, milestone 29's later rung) depends on it: a
/// bare-metal clang in the build for both ISAs, a Rust `user_mode_runtime` shell that holds every capability so
/// the C can hold none, and five libc symbols shimmed rather than a libc ported.
///
/// **The four claims, and how each is proven rather than assumed.** All four are asserted from
/// outside the faulting address space, by `c_confiner`, after the component is dead:
///
/// 1. *It faults*, rather than silently corrupting and continuing. Proven by the death message
///    existing at all, with `EVENT_FAULT` and a non-zero kernel-stamped tid.
/// 2. *The fault is the bug we planted.* The kernel's reported fault address equals the address the C
///    code computed, so the crash is not something unrelated on the way there, which would make the
///    rest of the assertions vacuous.
/// 3. *Nothing outside the grant changed.* Two witness pages, both position-derived patterns
///    checked byte by byte through the confiner's own mappings. `WITNESS_RO` is the **same physical
///    frame** the component holds read-only, so an unchanged page is not "the store landed
///    elsewhere"; the page was reachable and the store did not happen. `WITNESS_FAR` is a
///    **different frame at the same virtual address**, which is the statement that a virtual
///    address means nothing outside the address space that owns it.
/// 4. *The supervisor restarts it and the restart works.* Not "an instance ran": the replacement's
///    output is read out of the shared grant and checked against an independent Rust computation of
///    the same checksum, so a restart that produced a process which merely reported for duty fails.
///
/// The in-grant marker byte is the control for all of it. Each misbehaving C function stores inside
/// its grant first, and that store must be visible; a process whose stores never worked would satisfy
/// every witness check while proving nothing.
///
/// Both ISAs, because a fault that only manifests on one would be a finding, not a pass. The two
/// bugs take *different* fault paths on each (a permission fault on the read-only page, a translation
/// fault on the unmapped one), which is more of each architecture's fault machinery than any previous
/// test has exercised from userspace.
#[cfg(all(test, initrd))]
mod c_seam_tests;

/// **A running component replaced under a talking client** (milestone 23, DECISIONS §41).
///
/// The flagship the roadmap points at, and the thing to notice about it is what the kernel does not
/// contain. There is no component object, no swap syscall, no naming service, and no
/// lifecycle-aware anything: `swapper` is an unprivileged process with a budget, one device
/// capability and four endpoints, and the swap is the composition of mechanisms that already
/// existed for their own reasons. What milestone 23 needed the kernel to grow is exactly one thing:
/// `PageFrame::REVOKE` now answers on a `DeviceFrame`, with take-back semantics (§41).
///
/// **The claim is not that a swap completes. It is that a client does not notice.** So the shape is
/// the one milestones 29, 33 and 36 used: two witnesses in two address spaces, an attacker with
/// real authority, and a control that must fail.
///
/// 1. **The client's witness**, computed inside `chatty` from the replies it received. It holds one
///    capability to one endpoint for its whole life, calls sixty-four times in a plain loop, and
///    checks every answer against its own independent computation of the digest. It has no code
///    path for "the server went away" because there is no such event to have one for.
/// 2. **The operator's witness**, a shared page in `swapper`'s address space that each instance
///    stamps with its own version per request. Read after every writer is dead, it says that no
///    request went unserved (nothing was lost in the down window) and that the version never goes
///    backwards (**there were never two owners of the device at once**, which is the whole reason
///    step 2 revokes).
/// 3. **The control that must fail**: the outgoing instance is told to read one UART register
///    *after* the operator revoked it. It faults, and the kernel's fault message carries the
///    device's own virtual address. Before the revoke the same read succeeded, which is what makes
///    this a receipt rather than a coincidence.
/// 4. **The attacker**, `chatty` in its usurper role, endowed with exactly the honest client's
///    capabilities including a real working capability to the stable endpoint. It tries to park
///    itself in `RECEIVE_CAP` and become the server. `NotPermitted`: its capability carries `WRITE`
///    and not `READ`, so endpoint-only naming does not mean "whoever holds the endpoint is the
///    server".
///
/// **The replacement is written in C** (`fixtures/c/c_swappable.c`, over the seam DECISIONS §31 built),
/// and that is the strongest form of the claim: what held across the swap was the contract, not a
/// recompile of the same source.
///
/// The second test covers the latency ladder's opt-in rung, `broker`. Both ISAs, because a swap
/// that only worked on one would be a finding, not a pass.
#[cfg(all(test, initrd))]
mod live_swap_tests;

/// **Measured boot: the kernel refuses to enter a first process it was not built for** (milestone 22 phase
/// B.1, DECISIONS §22).
///
/// Cross-ISA, because the check is portable: one hash implementation (`crates/measured_boot`), one trust
/// root generated into the kernel image by `build.rs`, called from the boot path on every
/// architecture (`boot_progenitor`; the riscv milestone-20 demo `riscv_initrd_demo` measures too).
///
/// **What these two prove, and why the boot path itself cannot be tested directly.** A real refusal
/// halts the machine, so a test cannot take that branch and live. What *can* be proven, and is what
/// actually matters, is the decision: the same function the boot path consults says Ok for the bytes
/// in the initrd QEMU loaded (which proves the whole build composition end to end: userspace built,
/// archive packed, digest written, kernel compiled with it, and the digest in the running image
/// matches the archive in RAM), and says Err for bytes off by one bit. The boot path's only response
/// to Err is `arch::halt()`, which is three lines up from here in `trust::require` and is the sort of
/// thing a reader can check by looking.
#[cfg(all(test, initrd))]
mod measured_boot_tests;

/// **The fault endpoint: a supervisor watches a child die and reap it** (milestone 22, DECISIONS
/// §26). These are the cross-ISA tests, because the mechanism is portable: a supervised child that
/// faults (or exits) turns into a five-word message on its supervision endpoint, its corpse persists
/// until the supervisor reaps it with §16 revocation, and a fresh child runs in its place. The only
/// per-architecture parts are the two tiny code stubs (a null load that faults, and a `SEND` + exit),
/// and even those are the same shape both ISAs already use elsewhere in this file. The kernel is the
/// only sender on the fault endpoint, so the tid the supervisor reads is trustworthy without a badge.
#[cfg(test)]
mod supervision_tests;

/// The two load-bearing tests of the x86 port-range capability (milestone 299): a non-holder faults
/// on `out` (and a holder's grant does not leak across the switch to it), and a revoked holder faults
/// on its next `out`. `x86_64` only, because the mechanism is the TSS I/O permission bitmap, which
/// the other two architectures have no counterpart to.
#[cfg(all(test, target_arch = "x86_64"))]
mod x86_port_tests;

/// **A supervisor may collect a corpse without being able to build one** (DECISIONS §32,
/// `rendezvous::REAP`). Cross-ISA, because the authorization check is architecture-neutral: it reads
/// two fields of a TCB and compares two generational names, so a divergence here would mean
/// something is wrong under `arch/`, not in this feature.
///
/// **What shape these tests are, and why.** Every reap goes through the real syscall dispatcher
/// (`syscall::invoke`), from a thread whose capability table holds **endpoint capabilities and
/// nothing else**: that is what a supervisor's authority actually is, and calling `sched` directly
/// would prove the helper rather than the boundary. The *building* is done with kernel-internal
/// calls, which is deliberate: it keeps the builder's authority out of the supervisor's capability table, so
/// "structurally unable to build" is a fact about the table these tests audit rather than a promise.
///
/// The accounting proof is the one that makes §32 worth having. A test that only showed the corpse
/// gone would be satisfied by a reap that quietly handed the pages to the reaper. So the builder's
/// region is one the test still owns and can measure, and the assertion is that its watermark comes
/// back down and it can spend those pages again, while the supervisor's capability table does not grow.
#[cfg(test)]
mod reap_tests;

/// **Notification objects** (milestone 151 (notification objects), DECISIONS §101 (notification objects)): the kernel half of the binding, the
/// syscall layer's rights and registers, teardown, and a program binding a notification to itself
/// through the real boundary. Cross-ISA: nothing here is architecture-specific except the register
/// each `TrapFrame::arg` names, which is exactly what the `Irq::WAIT` and program tests read.
#[cfg(test)]
mod notification_tests;

/// **Timers** (milestone 106 (a wait that ends on either the interrupt or the deadline), DECISIONS
/// §147 (a timer a userspace service cannot hold)): the tick reaching the expiry walk, a wait ending
/// on a signal or on the deadline (in `WAIT` and in a bound `RECEIVE`), a replaced or cancelled deadline
/// never firing, and the syscall layer's rights. Cross-ISA: the counter each test reads is the one
/// the walk compares, on every architecture.
#[cfg(test)]
mod timer_tests;

/// **A process listing is a capability, not a fact about the machine** (milestone 126,
/// `rendezvous::SURVEY`, notes/process-view.md). Cross-ISA for the same reason `reap_tests` is: the
/// scope decision reads one field of a TCB and compares two generational names, so a divergence
/// here would mean something is wrong under `arch/` rather than in this feature.
///
/// **The shape, and why it is this shape.** Every survey goes through the real syscall dispatcher
/// (`syscall::invoke`), and the walk is driven by `ps::collect`, which is the loop `components/src/ps.rs`
/// really runs: a bug in the cursor protocol therefore cannot hide in the gap between the kernel's
/// half and the program's. The tests build real supervised children out of a real region, so the
/// domain under test is one the kernel built rather than one a helper described.
///
/// The negative control is the one that matters, and it keeps milestone 108's shape: a viewer run
/// against a domain it was not granted is **refused loudly** rather than shown an empty list, and
/// an empty domain answers rather than refusing. Both are asserted in the same test, because
/// neither claim means anything without the other.
///
/// `pgrep`'s filter is driven here too, and this is the only place in the tree that can be: the
/// selector arrives in a register, and the prompt cannot spell one (`crates/pgrep`'s `BUGS`). The
/// negative control gains a fourth answer with it, which is a selector that **matched nothing** in a
/// domain that really has members: distinct from an empty domain and from a refusal, where upstream
/// `pgrep` collapses all three into printing nothing.
#[cfg(test)]
mod survey_tests;

/// **The other axis of a survey: which per-thread fact it asks for** (calef's 2026-09-21 selector
/// ruling, `abi::survey::record`).
///
/// `survey_tests` above proves the walk, meaning what a domain contains and who may look at it.
/// This proves the selector, meaning which record a walk returns. Separate files because the two
/// properties are independent and their failures read nothing alike: a broken walk reports the
/// wrong threads, where a broken selector reports the wrong fact about the right threads, with
/// every tid still looking correct.
///
/// Cross-ISA, and here that is a claim rather than a habit (DECISIONS §19). The one record this
/// ships with is placement, which is `sched`'s: `pick_spawn_target` samples two online cpus and
/// `place_on` enqueues onto the winner, with no line of either under `arch/`. All three
/// architectures therefore run literally these assertions, and a divergence would mean the
/// scheduler is wrong rather than an ISA.
#[cfg(test)]
mod survey_record_tests;

/// **An endpoint that carries an interrupt refuses every send** (milestone 603 (an interrupt's
/// endpoint refuses every send), DECISIONS §101 (notification objects) ruling B). Driven through
/// the real dispatcher (`syscall::invoke`) on the sending side and through `Irq::WAIT` on the
/// driver's, because the ruling is about what a program can do at the boundary.
///
/// Cross-ISA, and it has to be: the refusal is `Rendezvous::send`'s and the error mapping is
/// `syscall`'s, neither under `arch/`. The interrupt is delivered with `sched::irq_notify`, the
/// function every ISA's handler calls, which keeps the test off each machine's interrupt wiring.
#[cfg(test)]
mod irq_send_refusal_tests;

/// **What the CPU-time record's number means** (milestone 282 (a thread's CPU time, and the `top` it makes possible), DECISIONS §150 (how does a thread's CPU time reach userspace?)).
///
/// `survey_record_tests` proves that a record can be asked for and that an unknown one is refused,
/// which a record returning a constant zero would satisfy. This proves the figure: a runaway is
/// charged for the CPU it took, a thread blocked in a send is charged for nothing, and a corpse
/// keeps what it earned. The first of those is the assertion the wall-clock age §150 refused would
/// fail, since two threads of the same age read identically under it.
#[cfg(test)]
mod cpu_time_tests;

/// **`pmap`'s split, one object type over `survey_tests`** (milestone 126, `address_space::LIST`,
/// DECISIONS §114). Cross-ISA for `survey_tests`'s reason: the method reads `Flags` through
/// `arch::mmu::translate_at`, so a divergence here means something is wrong under `arch/`.
///
/// Every listing goes through the real syscall dispatcher, driven by `pmap::collect`, the loop
/// `components/src/pmap.rs` really runs, `survey_tests`'s discipline verbatim. The negative control is
/// the one that matters: a capability holding `ENUMERATE` alone can list every mapping and is
/// refused `MAP_INTO`, and a capability holding `WRITE` alone can map and is refused `LIST`, so
/// the split is proved in both directions rather than asserted in prose.
#[cfg(test)]
mod pmap_tests;

/// **`AddressSpace::UNMAP`** (milestone 95 (an unmap primitive), DECISIONS §162 (whether a
/// holder can give up a mapping), option A): one page out of the tables
/// and out of the mapping record, a viewer refused, a `va` with nothing mapped refused, and the
/// record half proved by a revoke that must not reach the frame mapped at that address since.
/// Cross-ISA: the method is portable kernel code over `arch::mmu::unmap_user_at` (DECISIONS §19).
#[cfg(test)]
mod unmap_tests;

/// **A running address space stays nameable** (§249 (a running address space stays nameable)):
/// `UNMAP` through a capability made before `CONFIGURE` faults the running thread, from another
/// core too; a second bind is refused; a space dies with its thread and not with its capabilities;
/// a corpse does not keep a space its region gave back; and milestone 95 (an unmap primitive)'s
/// negative control, a builder faulting on a page it gave its child. Cross-ISA (DECISIONS §19).
#[cfg(test)]
mod running_space_tests;

/// **`free`, `vmstat` and `slabtop`'s two sources** (milestone 126 (the `procps` package),
/// DECISIONS §225 (`free` sees the machine and your share)): `MemoryRegion::USAGE` under
/// `ENUMERATE` alone, refused to a spender and answering a viewer, and the machine statistics page
/// recognized and moving. Arch-neutral, so every ISA runs it.
#[cfg(test)]
mod machine_statistics_tests;

/// **Scheduled execution, where every entry is a grant** (milestone 129, notes/scheduled-execution.md).
///
/// One module for both ISAs, like `dir_capability_tests`: nothing in it is architecture-specific, so
/// the parity gate (DECISIONS §19) is met by literally the same test running twice.
///
/// The claim is Unix cron's inversion. A crontab line runs as a user and can do whatever that user
/// can do, and there is nothing to print and nothing to check; here an entry is a grant expression
/// checked at registration by the same `grant_plan::plan` the prompt uses, so what a scheduled child
/// will hold is printable before the first tick. The test reads that plan off the real program
/// running the real `components/timetable.conf`, then watches what fires.
///
/// The negative control is what makes it worth having: the shipped document contains entries a Unix
/// cron would simply have run (`date` wants a clock, `ps` wants a process view), and the timetable
/// holds neither, so both are refused **in writing, before anything fires** and neither ever runs.
#[cfg(all(test, initrd))]
mod timetable_tests;

/// **The directory capability, attacked** (milestone 47, notes/dir-capability.md).
///
/// One module for both ISAs rather than an aarch64 test with a riscv twin, which the FS tests above
/// have. Nothing here is architecture-specific: it wires three portable programs and asserts on a
/// bitmap, so the only difference between the legs is which binary carries the block-server role,
/// and that is one `cfg` in [`blk_server_image`] rather than a second copy of every assertion. The
/// parity gate (DECISIONS §19) is met by literally the same test running twice.
#[cfg(all(test, initrd))]
mod dir_capability_tests;

/// **One process, two directory capabilities** (milestone 154,
/// design/roadmap/0154-multi-directory-namespace.md).
///
/// One module for both ISAs, for [`dir_capability_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by literally the same test
/// running twice. It wires the same three portable programs [`dir_capability_tests`] does, twice
/// (a second `fs_subtree_caretaker`, a second capability table slot) for one confined program, and proves
/// the deliverable both milestone 47's `bind` and milestone 64's `File::open` fork were blocked
/// on: `/a/x` and `/b/y` both resolve, `/a/../b` is refused, and neither caretaker can see the
/// other's tree.
#[cfg(all(test, initrd))]
mod multi_dir_namespace_tests;

/// **The navigation builtins, and the property that two shells cannot name each other's files**
/// (milestone 47's commands; notes/shell-navigation.md).
///
/// One module for both ISAs, for [`dir_capability_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running twice.
///
/// What is wired is the **real shell binary**, in a role that reads a script instead of a keyboard,
/// holding a `fs_subtree_caretaker`'s narrowed endpoint where the interactive one holds a terminal.
/// So the builtins under test are the builtins at the prompt rather than a reimplementation of
/// them, and the thing being confined is a shell.
#[cfg(all(test, initrd))]
mod shell_navigation_tests;

/// **`rm` as a program, and a recursive removal bounded by the capability it was handed**
/// (milestone 47's `rm -r`; notes/rm.md).
///
/// One module for both ISAs, for [`dir_capability_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running twice.
///
/// What is wired is the **real `rm` binary** (`components/src/rm.rs`) behind a real
/// `fs_subtree_caretaker`, started the way the shell would start it: the name in a grant's two
/// argument words and the options in the spec word, in `grant_plan::rmopt`'s bit order, so the numbers
/// here come from the manifest the prompt checks against rather than from a second copy of an
/// ordering.
///
/// The thing being demonstrated is not that a loop can delete a tree. It is that **the walk stops
/// exactly where the capabilities stop**: the same command line against the same tree does the
/// whole job through one grant and cannot begin through a narrower one, and no branch in the
/// program decides which.
#[cfg(all(test, initrd))]
mod rm_program_tests;

/// **Globbing: the expansion you see is the grant** (milestone 47's globbing lane;
/// notes/glob-grant.md).
///
/// One module for both ISAs, for [`dir_capability_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running twice.
///
/// What is wired is the **real shell binary** (expanding one pattern two ways over a real
/// `READDIR`) and then the **real `rm` binary** behind a real `fs_nameset_caretaker`. The argument
/// the two halves make together is the one Unix cannot make: the names a command displays are
/// literally the authority it would transfer, and nothing else in the directory moves.
#[cfg(all(test, initrd))]
mod glob_grant_tests;

/// **The shared-frame witness** (milestone 599 (a frame per filesystem client channel),
/// provisional): two live clients on one file service, one rewriting the other's name mid-request.
///
/// One module for both ISAs, for [`dir_capability_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running on
/// every architecture `script/test` boots. It builds finding 1 of `notes/shared-page-audit.md` (the
/// escape the set grant at the prompt would make live) and asserts the per-client windows close it.
#[cfg(all(test, initrd))]
mod fs_shared_page_tests;

/// Parity C: the virtio-blk driver, its two attackers, and the DMA confinement, on RISC-V.
///
/// These are the riscv twins of the three disk tests in the aarch64 module above, separate
/// because that module leans on aarch64-only scaffolding (the hand-written 7a user programs and
/// the PL011-wired `hello` roles), while these need only the ELF loader and the initrd archive.
/// The driver is the SAME `virtio` module the aarch64 roles compile, packed as the dedicated
/// `block_driver` binary (`components/src/block_driver.rs`); the kernel-side wiring (`virtio_service`) is
/// the same code,
/// unconditionally. What these prove that aarch64's runs do not: userspace device drivers with
/// DMA, and the kernel's DMA confinement, on the second ISA.
#[cfg(all(test, target_arch = "riscv64"))]
mod riscv_virtio_tests;

/// **The operators, end to end: `|` is two processes and an endpoint** (milestone 50,
/// notes/pipes.md).
///
/// One module for both ISAs, for [`shell_navigation_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running twice.
///
/// What is wired is the **real shell binary**, in a role that reads a script instead of a keyboard,
/// with the interactive endowment: a terminal, a spawn channel, a result channel, and a budget. The
/// kernel plays the two parties on the other ends.
///
/// - **The terminal.** The test itself serves `line_editor::proto::OPERATION_WRITE` and collects every byte
///   the shell prints. So the assertion is made against *what a person would see*, which is the
///   strongest form this can take: a pipeline that ran but printed the wrong thing fails here.
/// - **The progenitor.** A second thread serves `grant_plan::spawnproto`, receiving the delegated sink and source
///   capabilities and building each stage with them. It is deliberately the same protocol
///   `user/src/system_initializer.rs` serves, because the shell cannot tell the difference and neither should
///   this test; what it is not is the same *code*, and that gap is named in notes/pipes.md's BUGS.
#[cfg(test)]
pub mod pipeline_service;

/// **`>`, `<` and `|` at a real prompt** (milestone 50, notes/pipes.md).
///
/// The claim under test is one sentence: **a program holds an endpoint for its output and cannot
/// tell what is on the other end.** So the assertions are all of the form "the same binary, two
/// destinations, the same bytes", never "the pipeline printed something".
#[cfg(all(test, initrd))]
mod pipeline_tests;

/// **`>` and `<` at a prompt that holds a filesystem** (milestone 50, notes/pipes.md).
///
/// [`pipeline_tests`]'s shell with one more capability: a directory at slot 4, narrowed by a
/// `fs_subtree_caretaker` to one subtree of the real RedoxFS image. Everything else is identical,
/// which is the point of running both. The refusal in
/// `pipeline_tests::a_redirection_a_shell_cannot_back_is_refused_rather_than_dropped` and the file
/// written here are the same binary, and the only difference between them is one capability table slot.
///
/// The assertions are all of the "same producer, two destinations, the same bytes" shape, because
/// that is the only shape that can distinguish a redirection that worked from one that wrote
/// something plausible: a `>` that dropped every second byte would still produce a file, and a `wc`
/// that agreed with it would still print three numbers.
///
/// One module for both ISAs, for [`shell_navigation_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running twice.
#[cfg(all(test, initrd))]
mod redirection_tests;

/// **`time <command>` at a real prompt** (milestone 86, notes/time-command.md).
///
/// [`pipeline_tests`]'s shell with at most one more capability: a read-only clock page. The claim
/// under test is that **the timed command needs no authority to be timed**, so the timing is the
/// shell's own reading and the child is spawned with exactly the endowment its command line names.
///
/// The three clock states are three capability tables rather than three branches, which is the shape
/// [`redirection_tests`] uses for the directory: a published page, a page nobody published to, and
/// no capability at all. Two of those refusals are `date`'s sentences one milestone later, and the
/// only reason they are reachable is that the wiring changed.
///
/// One module for both ISAs, for [`shell_navigation_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running twice.
#[cfg(all(test, initrd))]
mod time_tests;

/// **Quoting, sequencing and `$?` at a real prompt** (milestone 67, notes/swish-language.md).
///
/// The **same run** of the same script [`redirection_tests`] asserts about, whose tail milestone 67
/// added: one shell, once. A seventh scripted shell would have been a seventh live process whose
/// frames nothing reclaims, and wiring one put [`time_tests`] over the frame pool intermittently
/// (`refused to load a user program: Unmappable(OutOfPageFrames)`). The wiring these lines need is
/// [`redirection_tests`]'s exactly, so a second copy bought nothing but the failure.
///
/// It is still its own module, because what it claims is its own: the redirection tests are about
/// where bytes go, and these are about what a word *is* and what a status means.
///
/// The assertions are pairs, which is [`redirection_tests`]'s shape and for the same reason. `echo
/// "*.txt"` against `echo *.txt` is one line quoted and one not; `least_authority_demo 3 && echo
/// yes` against `least_authority_demo && echo yes` is one connector against a refused left-hand
/// side. A single line proving "it printed something" would pass on a shell that ignored quoting
/// entirely.
///
/// One module for both ISAs, for [`shell_navigation_tests`]'s reason: nothing here is
/// architecture-specific, so the parity gate (DECISIONS §19) is met by the same test running twice.
#[cfg(all(test, initrd))]
mod language_tests;

/// **The sink contract, and the one behaviour it changed** (milestone 50, notes/sink-protocol.md).
///
/// Two claims, one per test, and they need each other. The first is that a program cannot tell what
/// its output slot holds; the second is that when what it held is destroyed, the program finds out.
/// Without the second, "indifferent" would mean "unable to notice anything", which is a much
/// cheaper property and the wrong one.
///
/// Both run on both ISAs (§19), because the claim is about a contract and not about an instruction
/// set.
#[cfg(all(test, initrd))]
mod sink_tests;

/// **The system log service** (milestone 613 (a system log service: the in-memory half), for §242
/// (a system log)): two differently badged writers and two readers, and the attribution each reader
/// sees comes from the badge, never from the writer.
///
/// Runs on all three ISAs (§19 (architectural parity is a tenet)): the service is one portable
/// binary, and the claim is about badges and a ring, not about an instruction set.
#[cfg(all(test, initrd))]
mod system_log_tests;

/// **`OPERATION_RAWMODE` and `OPERATION_READRAW`, proved against a real `line_editor`** (milestone 169 (`kilo`, the smallest real text editor, as the forcing function for raw terminal input)): echo
/// suppression, literal (uninterpreted) delivery of what the line discipline would otherwise
/// consume as an editing command, the two input models refusing each other, and a read parked
/// before data arrives still being answered once it does. See the module's own doc for why the
/// echo-suppression check is proven both ways rather than only the direction that matters.
#[cfg(all(test, initrd))]
mod raw_mode_tests;
/// `OPERATION_QUIESCE` and `FLAG_RETRY` (milestone 23 (a capability-routed component OS with live
/// replacement)): a terminal quiesced for replacement hands its parked reader back rather than
/// stranding it, and resumes a half-typed line without repainting it.
#[cfg(all(test, initrd))]
mod terminal_quiesce_tests;
/// `terminal_supervisor` replaces a live `line_editor` under a reader and a typist, carrying the
/// half-typed line and the history across (milestone 23 (a capability-routed component OS with live
/// replacement)).
#[cfg(all(test, initrd))]
mod terminal_swap_tests;

/// **`rmle` itself**: open a file, move a cursor, insert and delete characters, save. Driven with
/// real keystrokes over the raw-keystroke primitive, and the saved file verified independently of
/// `rmle`'s own report. See the module's own doc for why that independence matters.
#[cfg(all(test, initrd))]
mod rmle_tests;

/// **No test may leak a runnable thread** (the regression proxy for the test-thread starvation that
/// made the RedoxFS mount overrun the hang watchdog under the net boot). A one-shot driver that
/// spins forever instead of exiting stays `Ready`/`Running` for the rest of the boot; enough of them
/// crammed onto core 0 (the scheduler places every spawn and wake on the current core, DECISIONS
/// "Open design ideas": the SMP placement gap) starve a later heavy test past the 60 s watchdog.
///
/// It quiesces first (yielding lets a just-finished thread be reaped by the next context switch),
/// then asserts nothing but the idle threads and this probe is still runnable. A leak fails here with
/// the offending thread in the dump, on the test that leaked's own turf, rather than as a mysterious
/// watchdog trip three tests later.
///
/// **The name is what makes this run last, not its position in the file**, and getting that wrong is
/// how the module spent many milestones never policing the one place that needed it. Tests run in
/// link order, which is alphabetical by module path, so being the last thing in the file bought
/// nothing: as `no_leaked_threads` it sorted before `tests`, and `kernel::user::tests` is precisely
/// the module whose whole subject is user threads. Measured on 2026-08-02, the probe ran 158 test
/// lines before the last test it was supposed to police.
///
/// So it is named to sort after `tests`, and the tree's own word for it (`notes/riscv-parity-scope.md`
/// calls this the leak police) is the name.
///
/// # BUGS
///
/// The ordering is still only alphabetical. A future `kernel::user` module sorting after
/// `thread_leak_police` would run after the probe and could leak unpoliced, silently, exactly as
/// `tests` did. Nothing enforces this; there is no "run me last" attribute in
/// `custom_test_frameworks`. If that happens, the symptom will again be a starvation watchdog
/// somewhere unrelated rather than a failure here.
#[cfg(test)]
mod thread_leak_police;

/// **Revocation against a capability that is in flight** (risk 7's adversarial pass, 2026-09-21).
///
/// Every revocation sweep in this kernel walks capability tables. A capability handed to a
/// rendezvous nobody is receiving on yet sits in `Thread::outgoing_cap` instead, which no sweep but
/// `sched::delete_reply_caps_naming` reads. The module's own header has the reasoning and the
/// `BUGS`; it is here rather than in [`tests`] because that file is this tree's worst merge hotspot.
///
/// Cross-ISA: `outgoing_cap`, the sweeps and the rendezvous are portable scheduler code, so the
/// parity gate (DECISIONS §19, architectural parity is a tenet) is met by the same test running on
/// each architecture.
#[cfg(test)]
mod receive_cap_attack_tests;
// Each module carries its own `cfg(test)`: milestone 634 (a plain SEND received by RECEIVE_CAP never
// hands the receiver a sender-chosen slot) inserted the line above between the attribute and
// `revocation_in_flight_tests`, and the attribute silently moved with it. The
// "tests the suite cannot see" check in `script/lint` now refuses a bare `mod` here.
#[cfg(test)]
mod revocation_in_flight_tests;

/// **A revocation sweep that lands inside a delegation leaves no copy behind** (the
/// revocation-race lane, 2026-10-04 UTC, provisional). Milestone 761 (capability lookup off the global lock)'s `BUGS` recorded the gap
/// between a delegating syscall's read of its source and its filing of the copy; these tests hold a
/// thread in that gap with `delegation_pause` and run the sweep. Its own header has the reasoning.
///
/// Cross-ISA: the delegations, the sweeps and the seam are portable kernel code (DECISIONS §19).
#[cfg(test)]
mod revocation_window_tests;

/// **A revocation sweep that lands inside a `MAP` leaves no mapping behind** (the
/// map-revocation-window lane, 2026-10-04 UTC, provisional). The use-side sibling of
/// [`revocation_window_tests`]: `PageFrame::MAP` and `AddressSpace::MAP_INTO` held after their
/// frame read, under `PageFrame::REVOKE` and `MemoryRegion::DESTROY`. Its own header has the reasoning.
///
/// Cross-ISA: the map paths, the sweeps and the seam are portable kernel code (DECISIONS §19).
#[cfg(test)]
mod map_revocation_window_tests;

/// **A seeded syscall driver with a shadow model** (milestone 752 (a seeded syscall driver with a
/// shadow model), provisional). Random capability operations from a seed, every answer and every table
/// predicted by a model and compared. Its own header has the oracle, the replay and the `BUGS`;
/// a module of its own for [`tests`]' merge-hotspot reason, named to sort before
/// [`thread_leak_police`].
///
/// Cross-ISA: one portable body through the portable syscall layer, run on every architecture
/// (DECISIONS §19).
#[cfg(test)]
mod syscall_fuzzer_tests;

/// **A confined EL0 process fuzzes the syscall surface under a conductor** (milestone 779 (fuzz
/// the surface a confined process can reach), part (b), provisional). The sibling of
/// [`syscall_fuzzer_tests`] one level down: real EL0 programs through the real trap entry, one
/// random call at a time, judged between calls by an escape oracle rather than a shadow model. Its
/// own header has the conductor, the oracles and the `BUGS`; a module of its own for [`tests`']
/// merge-hotspot reason, named to sort before [`thread_leak_police`]. It spawns the
/// `confined_syscall_fuzzer` fixture, so it carries the initrd gate.
///
/// Cross-ISA: the trap entries differ by ISA, which is the point, and the conductor is portable
/// kernel code, run on every architecture (DECISIONS §19).
#[cfg(all(test, initrd))]
mod confined_fuzzer_tests;

/// **`login` gives back everything when any retype of a login fails** (milestone 757 (a test kernel
/// fails a process on its Nth retype), provisional). The kernel refuses `login`'s Nth retype, for
/// every N a login reaches, and `login`'s capability table and region usage must come back each
/// time. Its own header has what it reaches and its `BUGS`; a module of its own for [`tests`]'
/// merge-hotspot reason, named to sort before [`thread_leak_police`]. It uses `login_tests`' one
/// `login`, so it carries that module's gate.
///
/// Cross-ISA: one portable body over portable kernel code (DECISIONS §19).
#[cfg(all(test, initrd))]
mod nth_retype_tests;

/// **A userspace builder keeps building past its scratch window** (milestone 604 (provisional),
/// the builder's scratch cursor is bounded).
///
/// `supervision_protocol`'s loader wraps its scratch cursor and probes for pages the kernel took
/// back when a child's region was destroyed; this builds 40 `ripgrep`-sized children through it on
/// a table budget the old, climbing cursor ran out of at the thirty-seventh (measured, with the
/// wrap taken out). Its own header has the arithmetic. A module of its own rather than a test in
/// [`tests`], for that file's merge-hotspot reason, and named to sort before
/// [`thread_leak_police`].
///
/// Cross-ISA: one portable test body, run on every architecture (DECISIONS §19).
#[cfg(all(test, initrd))]
mod scratch_window_tests;

/// **Revocation against a mapping the kernel wired** (the `map_physical` mapping record,
/// 2026-09-21).
///
/// Every unmap sweep in `crate::revoke` is driven by the mapping log, and `AddressSpace::map_physical`
/// filed nothing in it, so a `Spawn::maps` entry survived a revoke of its own frame. The module's
/// own header has the reasoning, the reachable boot path and the `BUGS`; it is here rather than in
/// [`tests`] because that file is this tree's worst merge hotspot, and it is named to sort before
/// [`thread_leak_police`] for that module's own reason.
///
/// Cross-ISA: the mapping log, the sweeps and `map_physical` are portable kernel code, so the
/// parity gate (DECISIONS §19, architectural parity is a tenet) is met by the same test running on
/// each architecture.
#[cfg(test)]
mod spawn_mapping_revocation_tests;

/// **A destroyed region takes no live address space's page tables with it** (the
/// page-tables-outlive-destroy lane, 2026-10-05 UTC, provisional). `PageFrame::MAP` and
/// `MemoryRegion::MAP` build tables out of a region the caller names, and `DESTROY` handed those
/// back while the space still linked them. Its own header has the reasoning; a module of its own
/// for [`tests`]' merge-hotspot reason, named to sort before [`thread_leak_police`].
///
/// Cross-ISA: the map paths, the log and the cut are portable kernel code (DECISIONS §19).
#[cfg(test)]
mod page_table_region_tests;

/// **A client holding one right on a rendezvous cannot reach the operations another right gates**
/// (milestone 633 (an outside agent attacks the confinement claim), fatal risk 7's confinement
/// claim, third outsider pass). The kernel half of claim 26 ("a client cannot become its server")
/// and claim 2 ("userspace cannot forge a right out of a syscall register"), driven through the real
/// dispatcher. Its own header has the reasoning and why it sits beside `live_swap_tests` rather than
/// extending it. A module of its own for `tests`' merge-hotspot reason, named to sort near the other
/// adversarial modules.
///
/// Cross-ISA: `invoke`, the rights check and the rendezvous are portable kernel code (DECISIONS §19).
#[cfg(test)]
mod confinement_attack_tests;
