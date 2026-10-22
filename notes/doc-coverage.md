# Rustdoc coverage: the doc-example floor and the `missing_docs` ratchet

Milestone 68 left two halves unfinished with counts attached, and this note is where those counts
live now that they are re-derived and acted on. It covers two different things that are easy to
confuse, so they are separated here on purpose:

- **Doc examples**, which is whether a crate has a worked example at all. FreeBSD's standard, the
  one AGENTS.md sets: *a page without a worked example has not finished explaining itself.*
- Item documentation, which is whether every public item has a doc comment. This is what
  `missing_docs` checks, and it is what blocks adopting that lint.

## The numbers the roadmap block carried were stale, and one of them measured the wrong thing

Measured 2026-08-17, against the block's 2026-08-02 numbers.

| Claim (2026-08-02) | Measured (2026-08-17) |
|---|---|
| 28 host crates with no doc example | **31**, before this pass. Now **0** |
| 23 doctests in the host workspace | **49**, before this pass. Now **116** (109 of them run by `script/test`'s selection; see BUGS) |
| Item coverage 36.4% (`socket_contract`) to 100% | **50.0%** (`intrusive_fifo`) to 100%; `socket_contract` is now 57.6% |

**The deficit grew while work was being done, and both halves of that are worth seeing.** Three of
the four crates the block named as the hard ones left (`dtb`, `nifefs`, `gpt`) got their examples in
the following fortnight, and `machine_discovery`, `manual`, `swish` and `generational_table` gained theirs too. Meanwhile five
crates arrived with none: `ntlm` and `system_initializer` (2026-08-04), `nvme` (2026-08-15), and
`multicast_dns_protocol` and `smb_proto` (2026-08-15). `ntlm` and `smb_proto` were removed from the tree on
2026-08-30 (notes/smb.md), and `multicast_dns_protocol` with `multicast_dns_config` on 2026-09-15
(notes/mdns.md); their rows below are left as the record they always were. A count of
what is missing is a moving target in a tree
adding a crate every few days, which is the argument for a gate rather than a number in a block.

The coverage range is measured by `rustdoc --show-coverage`, and it is not the same measure as
`missing_docs`. This matters because the block used the first to justify deferring the second. Six
crates report 100% documented and still have `missing_docs` hits, because `--show-coverage` does not
count struct fields, type aliases or `macro_rules!` and the lint does. Take a `missing_docs` decision
from `missing_docs` output.

## Doc examples: closed, and how each crate was treated

Every crate under `crates/` now has at least one worked example. Three treatments, and the third is a
recorded limitation rather than a pass:

1. An executing doctest (28 crates). Preferred always: a doctest that runs is a test, and this
   project's whole method is pure logic in host-testable crates. Each example was written to carry
   the crate's own argument rather than to restate a signature. `elf` forges a
   writable-and-executable segment and watches it be refused. `paging` builds real page tables on
   the host and demonstrates that break-before-make is forced. `smb_proto` performed a whole SMB2
   mount, and `network_time_protocol` shows an
   off-path spoof failing the origin check before any of the packet is believed.

2. **`no_run`, with the reason stated in the prose** (`user_mode_runtime`, `virtio`, `system_initializer`).
   These have nothing to assert: every entry point is a syscall from EL0 or returns `!`. `svc` on a
   machine with no nife kernel under it is a fault, not a syscall. The examples are type-checked
   against the real signatures and executed by the QEMU boot and `script/swish-check`.

3. An executing doctest that the gate does not run (`hot_swap_contract`, `supervision_protocol`, and the
   two above that are not `no_run`). See BUGS below; this is the one honest gap.

## BUGS

- **Five crates' doctests are never run by `script/test`.** `user_mode_runtime`, `hot_swap_contract`, `virtio`,
  `supervision_protocol` and `system_initializer` take unconditional `user_mode_runtime` dependencies, so the host
  test selection excludes them (the list is in `xtask/src/suite.rs`, derived and checked by
  `script/lint`). Their examples run under `cargo test --doc -p <crate>` on an aarch64 host and
  are checked by nothing in CI. On an x86_64 host they do not even compile, which is a property of the
  packages and not of the examples. The fix is to split each crate's pure half out from its syscall
  half. That is a lane of its own, and is what would let the arithmetic in `hot_swap_contract::digest`
  and
  the constants in `supervision_protocol` be gate-checked like every other wire contract.

- **`rustdoc --show-coverage` undercounts.** See the section above. It is still the right tool for
  "does this crate have any example at all", which is what the examples half is about.

- A crate can lose the ratchet by deleting one line. The `#![warn(missing_docs)]` opt-in below is
  rung two of AGENTS.md's ladder for the crates that carry it and rung zero for the ones that do
  not. Nothing requires a crate that becomes clean to adopt it, and nothing requires a *new* crate
  to.
  Closing that needs a `script/lint` check, and the cost is a second workspace clippy pass with
  different flags (so a second full build, since the flags are part of cargo's cache key). That is a
  decision about gate runtime rather than about documentation, and it is not taken here.

## `missing_docs`: the ratchet, and the worklist

401 undocumented public items across 32 of the 55 crates, measured 2026-08-17 with one
workspace-wide `cargo clippy --workspace --lib -- -W missing_docs` into a clean target directory.
Measure it that way or not at all: cargo replays cached diagnostics, and a per-package loop reports
other crates' warnings as the selected crate's. A first attempt at this measurement said 647 across
38 crates for exactly that reason.

Adopting the lint tree-wide is a commitment to write those 401 first, so it was not adopted tree-wide
at first. A milestone 68 follow-up lane re-measured on 2026-08-22 (the trap-avoiding way: one
workspace-wide `cargo clippy --workspace --lib -- -W missing_docs` into a clean target directory) and
found the honest count had already drifted to 404 across 31 crates, five days on. That lane closed 169
of those 404, crate by crate, re-measuring the same way after every batch: 235 items remain, across
7 of the 57 crates under `crates/`. The other 50 carry the per-crate opt-in, spelled
`#![warn(missing_docs)]` beside each crate's `#![no_std]`. Under `script/lint`'s `-D warnings` that is
a hard gate, so those crates cannot regress:

`abi`, `address_space_identifier`, `bitmap_font`, `block_roster`, `c_seam`, `calendar`, `memory_corruption_canary_gate`, `capability`,
`clock_contract`, `capability_plan`, `coremark`, `cpu_set`, `cred`, `credential_contract`,
`dma_validator`, `device_tree_blob`. `elf`, `entropy_contract`, `page_frames`,
`filesystem_contract`, `graphics_contract`, `glob`, `inter_process_communication`, `intrusive_fifo`,
`line_editor`,
`manual`, `measured_boot`, `nifefs`, `network_time_protocol`, `non_volatile_memory_express`,
`paging`, `pgrep`. `ps`, `memory_regions`, `byte_sink_contract`, `generational_table`,
`socket_contract`, `work_steal_slot`, `supervision_protocol`,
`hot_swap_contract`, `swish`, `system_initializer`, `timetable`, `user_mode_runtime`, `video_terminal`,
`virtio`, `thread_wake_handshake`.

The worklist, largest first, so the next person can take one crate and turn its line on:

| Crate | Items |
|---|---|
| `machine_discovery` | 54 |
| `smb_proto` (removed 2026-08-30) | 52 |
| `multicast_dns_protocol` (removed 2026-09-15) | 41 |
| `pci` | 24 |
| `globally_unique_identifier_partition_table` | 23 |
| `grant_plan` | 22 |
| `compositor` | 19 |

Every crate that was one item from clean on 2026-08-17, and every crate under about a dozen items, is
now closed. What remains is seven substantially larger crates (19 to 54 items each); none is close to
clean, so there is no more "cheapest item" shortcut left in this table.

Outside `crates/`: `xtask` has 176, `user` has 50, and `kernel` has 2 (both
`#[macro_export]` macros in `console.rs`, documented in the original pass). Unmeasured by this
follow-up lane, since `--lib` (required to avoid the cache-replay trap) does not reach a `[[bin]]`-only
package, and none of xtask/user/kernel were touched. The kernel's near-zero count is not a surprise
once you look: it is a binary, so almost nothing in it is public API. It does not carry the attribute
anyway, because on a binary crate the lint reaches only exported macros, which is a small return for a
line in a file every lane touches.

## The decision this leaves open

Whether `missing_docs` should go in `[workspace.lints.rust]` with an explicit
`#![allow(missing_docs)]` in each of the 7 crates that are not ready, instead of the opt-in above.
That is higher on AGENTS.md's ladder, because the default becomes on and the opt-out list is a
greppable worklist that can only shrink, and it is what would cover a crate created tomorrow. It also
contradicts a rule written in that table's own comment: *"adding a lint to this table is a decision to
fix every existing violation first. Nothing goes in this table to see what it finds."* Inverting that
for one lint is a policy change, so it is an architect's and not a lane's; see the pull request that
closed this worklist down to 7 crates for the six-questions writeup and a recommendation.
