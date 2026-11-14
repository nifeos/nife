---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1062, 1069
decision_dependencies: 46, 23
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1113. `virtio`: what an outside maintainer would fix before it leaves this tree, if it leaves

Raised by a read-only outside review of `crates/virtio`, read as a stranger to this tree would meet
it. It changes nothing; it lists what the next lanes change, and it puts one question to the
architect first, because every other item depends on its answer.

There is no review package for this crate in `notes/crate-reviews/`, no row in
`notes/crate-reviews.md`, and no step for it in journey 5. So this review has no settled baseline
to take. The one ruling it takes as settled is the crate name: `virtio` is ratified as a proper
name (the specification's own) and this review does not reopen it. Everything else below is open.

Reuse: `script/crate-review-packet`, `script/crate-standalone`, `script/names`,
`helpers/name_release.py`, the template `nifeos/crate-workflows`, and the crates.io API for the
prior-art figures. No new tool.

## Goal

A reader who opens `crates/virtio` can tell in one screen what it is. Today it is the body of one
program (`block_driver`). It has seven entry points that each drive a device and never return. Six
of them are test roles the QEMU suite spawns, and one is the block server RedoxFS runs on. Whatever
the architect decides about where that code lives, the code that stays is sound on every path a
device or a client can reach. Its layout constants are the kernel's and not a copy of them. Its pure
logic is testable on the host, and its names pass `design/naming.md`.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1113-virtio-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1113-virtio-outside-review/prior-art.md).

## The items

Each item states what is wrong, where, why it matters, the fix, and the effort (S under a day, M a
few days, L a lane of its own). Each also says whether it is a one-way door (a published name, wire
format, identity, layout or dependency) or two-way.

### P0

**1. The crate cannot leave as it stands, and by the standing rules it should not leave as a
crate at all.** `Cargo.toml` lines 12 to 18 names four path dependencies. One of them is `user_mode_runtime`,
the SDK. The contract-crate ordering (milestone 1017 (the contract crates leave in this order)) has the SDK leaving last, because a crate that
leaves must reference nothing back into this tree. The crate's seven public items
(`src/lib.rs` lines 198, 246, 297, 321, 352, 872 and 933) are `fn(u64) -> !` entry points: five test roles
the QEMU suite spawns, one abandon-and-die role, and one server. They hardcode this tree's
conventions. These include capability slot numbers (`:42-45`, `:910`, `:923`), the DMA window
address (`:39`), and the kernel image address (`:326`, `:357`). They also include the test disk's
file names `motd` and `scratch` (`:213`, `:256`). Finally, they hardcode a private report protocol
with the kernel test (`0xDEAD_...`, `0xE4` through `0xEA`, `0xE2xx`, `CRKWRIT1`). It has one consumer, `components/src/block_driver.rs`. The
ruling that a crate gets its own repository only when it would publish on its merits or when more
than one repository consumes it fails on both counts. Why it matters: a repository made for this
crate would be a repository nobody outside could use and that pins the SDK back into nife. The
fix: see Decision 1. Effort: S to fold the roles into the program, L to re-cut a library. One-way:
a repository and a published name.

**2. No test can run anywhere but under QEMU, and no proof exists.** The crate has zero unit
tests, no Kani harness, and one `no_run` doctest (`:192-197`). Its pure logic is `find_file`
(`:399-421`), `pattern_byte` (`:232-239`), `ip_checksum` (`:606-617`), `build_dhcp_discover`
(`:723-796`) and `parse_dhcp_offer` (`:821-865`). Each reads or writes the fixed `DMA_VA` through
volatile pointers, so none of it can run on the host. `notes/mutation-testing.md` records that a
census of this crate measures nothing. Why it matters: every behavior below that this review
found by reading would have been a one-line host test. The fix: route the DMA accesses through a
window value (`user_mode_runtime::mapped_window::MappedWindow`, which `virtio_net_transport`
already uses, or a `&mut [u8]` for the parsers), so the parsers and builders take a buffer and a
host test can hand them one. Effort: M. Two-way.

### P2: later

The P2 items are in [the appendix](1113-virtio-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1113-virtio-outside-review/before-the-tag.md).

## Decisions for the architect

Two decisions, each with the background first, then the recommendation, prior art and a
concrete example. The maintainer decides Decision 2, provisionally, below.

### Decision 1. Whether `crates/virtio` leaves this tree, and as what

Background. The crate is the shared body of one program, `block_driver`: five test roles the DMA
confinement and parity suites spawn (`run`, `run_write`, `run_write_abandon`, `run_attack`,
`run_attack_indirect`), one DHCP exerciser (`run_net`), and the block server RedoxFS runs on
(`run_blk_server`). It became a crate when a second binary compiled the same file (the aarch64
`hello` roles, per the comment at `components/Cargo.toml` lines 89 to 91); that second consumer is gone,
and `block_driver.rs` is the only caller. It depends on the SDK (`user_mode_runtime`), which the
contract-crate ordering has leaving last. Two standing rules bear on it. First, a crate gets its own
repository only when it would publish on its merits or when more than one repository consumes it.
Second, a program package leaves as one repository holding its programs and any crate only that
package uses. Two ruled milestones are about to change what a nife virtio driver is. One is a direct
grant for IOMMU-confined virtio-pci functions (1062 step 7), under which the driver sets its own
queues up through the device registers. The other is a startup page that replaces the fixed `DMA_VA`
(1069). Meanwhile the tree holds six copies of the virtio-mmio handshake and ring constants, and
none of them is in the crate named `virtio` as a shared thing.

Recommendation. The crate does not leave as a crate. Its contents fold into the program that owns
them, `components/src/block_driver.rs` (or a module beside it), and leave with the drivers
package. The ratified name `virtio` stays for a library, cut later, that holds what the six copies
share. That means the register vocabulary at the transport seam, the status and feature bits, the
handshake, the descriptor and ring accessors at the validator's fixed layout, the blk request chain
and the net header. It has no test role and no slot convention in it. Whether to cut that
library, and when, is a second question that waits for 1062 step 7, because the direct grant
changes what the shared handshake does. Recommending the fold first is about the rules, not the
effort: it is also the smaller change, and that is not why it is right.

Prior art. Linux (`drivers/virtio/virtio_ring.c` shared by every class driver), OpenBSD
(`sys/dev/pv/virtio.c` under `vioblk` and `vio`) and Redox (`virtio-core` under `virtio-blkd`,
`virtio-netd`, `virtio-gpud`) all put the shared half in one library and the device drivers beside
it. None of them ships test roles as a library. The reuse record already names Redox's
split as the shape nife chose when it refused `virtio-drivers`.

Concrete example. `crates/virtio/src/lib.rs` lines 47 to 105 and `components/src/virtio_net_transport.rs` lines 54 to 87
hold the same thirty constants. Folding the roles into `block_driver` changes neither line and
removes a crate boundary that guards nothing; cutting the library later removes one of the two
copies and the four others with it.

### Decision 2. Where the kernel-to-driver ring layout lives

Background. `SETUP_QUEUE` places a queue's rings at fixed offsets inside the queue's ring block of
the driver's DMA region. The offsets and the queue size are the ones `direct_memory_access_validator`
proves (`LAYOUT_QSIZE`, `DESC_OFF`, `AVAIL_OFF`, `USED_OFF`, `RING_BLOCK`). The kernel aliases
the validator's constants so the layout it runs is the proved one. Every userspace driver spells
the same numbers as literals. §46 (thin primitives or whole subsystems; we write everything in between) priced the edge from `components` to the validator, and the
tree did not take it (milestone 410 (six copies of the shared-frame accessors), per `notes/net/the-driver-and-dhcp.md` lines 130 to 137). The
validator has no dependencies. `abi::virtio` already carries one DMA-layout fact for drivers, the
word offset at which the kernel writes the region's physical base.

Recommendation. The five layout constants move to `abi::virtio`, which is the contract the driver
already speaks (`SETUP_QUEUE`'s documentation there already describes "the fixed offsets of that
queue's ring block"). The validator and the kernel alias them from there, so the proof still runs on
the layout the kernel runs. Every driver then imports what it already depends on through
`user_mode_runtime`, and no new edge is taken. The alternative, drivers importing the validator
directly, is the smaller edit and a zero-dependency edge. This review sets it aside only because it
makes a proof crate a contract crate. If neither is wanted, the fallback is a `BUGS` entry at every
copy, which is what `virtio_net_transport` has today and this crate lacks.

Prior art. Linux puts the ring layout in a UAPI header (`include/uapi/linux/virtio_ring.h`) that
both the kernel's ring code and every userspace consumer (QEMU, rust-vmm's `virtio-bindings`)
compile against; the layout is a contract, and the contract has one home. Nife's own `abi` crate
is the same thing for the syscall boundary.

Concrete example. `kernel/src/virtio.rs` lines 426 to 442 aliases the validator; `crates/virtio/src/lib.rs` line 72
and `:99-101` and `components/src/virtio_net_transport.rs` lines 73 to 87 spell `8`, `0x000`, `0x080`,
`0x100`, `0x200`. After the move, each of those is `abi::virtio::RING_...` and a change to the
proved layout is a compile error in every driver.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may
revisit. The layout values are already the agreement between kernel and driver and do not change;
only the home of their names moves, inside unreleased crates. They stay relevant after the virtio
default ruling of 2026-11-11, because every mediated grant still uses `SETUP_QUEUE` at the
validator's layout. The five constants (`crates/direct_memory_access_validator/src/lib.rs` lines
294 to 304) move to `abi::virtio`, beside the DMA-layout fact already there
(`crates/abi/src/virtio.rs` line 38), with `RING_END`, which the validator derives from them and
the kernel also aliases. The validator and the kernel alias them from there. The present names fail
the spelled-out rule (`QSIZE`, `DESC`, `OFF`), so the lane that moves them proposes spelled-out
names in `abi`, provisional, and they join milestone 1075's (the `abi` outside review) pinned-value list and version gate
(1075 item 11). Item 3 is the work.

## Done when

The checks are in [the appendix](1113-virtio-outside-review/done-when.md).

## BUGS

- The review did not boot the QEMU suite. The review took the roles' behavior under test from the
  entry points and from `notes/virtio.md`, and did not observe it.
- Items 6 and 13 cite specification requirements from memory; nobody has verified the section
  numbers,   and the recording lane should confirm them against virtio 1.2 before the text lands.
- Item 14 states a performance shape and no figure. A figure needs `script/bench` on a machine
  with the storage rows, which the review did not run.
- The crates.io figures are one API call each at the time of the review; the `virtio` name's
  availability on crates.io rests on a single not-found response.
- The six-copy count of the handshake constants comes from a `grep` for the magic value and
  `S_FEATURES_OK`; it did not compare every constant in every copy.

## Index row

An outside review of `crates/virtio` before any move. The crate is one program's body and test roles
with a path dependency on the SDK, so the first question is whether it leaves at all. Two decisions
go to the architect (fold or re-cut; where the ring layout lives). The review has two P0 items and
five P2 items. Its nine P1 items cover a client-reachable panic in the block server, an unchecked
`unsafe` precondition, two skipped handshake steps, a length-blind frame parser, copied layout
constants, duplicated loops and the names.
