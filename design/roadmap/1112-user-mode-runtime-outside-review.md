---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1112. `user_mode_runtime`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/user_mode_runtime` at the head of `main`, read the way a
maintainer meeting it for the first time would read it. It changes nothing; it lists what the next
lanes change.

The crate has no review package in `notes/crate-reviews/`, so there is no settled baseline to
inherit. The review treats as settled only what a ruling elsewhere already fixes. That is the crate name,
`VERSION` and `INTERFACE_ID` (the names every contract crate carries), `INTERFACE_ID = 20` for the
entry registers and link layout, and the ratified function names the `Name:` blocks record. It is also the
order of extraction in milestone 1017 (the contract crates leave in this order). That order places this
crate in step 3, after `abi`, `address_space_map`, `current_cpu_contract` and the five contract
crates it links. Nothing here reopens those. Where a later rule contradicts a recorded ruling, the
item says so.

Two pull requests in flight touch `src/socket_frame.rs`: #3517 (a send's length rides in the `CALL`
argument word) and the ruling that `ACCEPT`'s second reply word carries the peer. Both are read
against the items below; neither changes a line of code in this crate, only documentation and one
example.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-move --dry-run`, the template's name and prose gates, cargo-llvm-cov, cargo-mutants.
No new tool.

## Goal

A stranger who finds `user_mode_runtime` in its own repository can tell in one screen what it is
for. It holds the one syscall wrapper, the entry macro, the panic handler and the typed windows over
memory the kernel handed a process. It covers three architectures. The same stranger can build it for all three
targets and run its host tests from the README alone. The stranger can also trust that every safe
function is safe to call with any argument. A wrong argument panics or meets a refusal from the
kernel, and it never reads or writes memory the type did not promise. The repository's required checks are green for the
crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1112-user-mode-runtime-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1112-user-mode-runtime-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two, L is a week. A door is
one-way when somebody outside the repository acts on it after the release (a public name, a
published behavior, a layout, a wire value, a dependency); everything else is two-way and the
maintainer decides it. The crate has no release, so every public name is two-way until the first tag
and one-way after it.

### P0: blocks the move or any publish

The P0 items are in [the appendix](1112-user-mode-runtime-outside-review/blocks-the-move.md).

### P2: later

The P2 items are in [the appendix](1112-user-mode-runtime-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1112-user-mode-runtime-outside-review/before-the-tag.md).

## Decisions for the architect

Three decisions, each with the background first, and each decided provisionally below.

### Decision 1: the names a stranger meets

Background. The move rule asks for ratified names before a crate leaves, and 114 of this crate's
names lack ratification. The template's abbreviation gate will run in the new repository with a ceiling, and
today it reports 53 components. The naming rules this review applied are the ones in
`design/naming.md`: abbreviations spelled out, a quantity ending in its unit, the standard
library's spelling and meaning where it has one, and no generic word. A ratified name keeps its
spelling until its own review. The record shows none of the names below as ratified except where
noted.

Recommendation, one row per name, the stricter spelling first:

| today | recommended | why |
|---|---|---|
| `va` (three parameters) | `virtual_address` | the `[spelled_out]` list says so; the sibling contract crates already renamed theirs |
| `aspace`, `aspace_slot` | `address_space_slot` | abbreviation; `address_space_map_into` already spells it |
| `tcb_slot` | `thread_control_block_slot` | the function names already spell it |
| `user_sp` | `user_stack_pointer` | abbreviation |
| `nr` | `number` | abbreviation |
| `val` (three) | `value` | the list says so |
| `num` | `descriptors` | a count of things names the things |
| `x0`, `x1`, `x2` (parameters of `thread_control_block_start`) | `argument0`, `argument1`, `argument2` | register names of one architecture in a three-architecture signature |
| `copy_within(src, dst, len)` | `copy_within(source, destination, bytes)` | the list spells both; the standard library's `copy_within` takes a range and `dest`, so this is not its signature anyway |
| `set_dst`, `dst_ip`, `dst_port` | `set_destination`, `destination_address`, `destination_port` | the list spells `dst`; `ip` has no entry |
| `mapped_window::PAGE` | `PAGE_BYTES` | a quantity carries its unit; the page-crate rule spells it this way |
| `cntfrq`, `cntfrq_checked` | `counter_hertz`, `counter_hertz_checked` | a rate ends in `_hertz` (`tick_hertz` is the in-crate precedent); the ARM mnemonic names nothing on riscv64 or `x86_64` |
| `MappedWindow::sub` | `MappedWindow::window` | `sub` is the standard library's subtraction |
| `list` | `address_space_list` | a generic word; its twin is `survey`, which names its object |
| `r8` ... `w64` | keep, or `read_u8` ... `write_u64` | the architect's call; Linux spells `readb`/`writeq`, OpenBSD `bus_space_read_1`; `read<T>`/`write<T>` already exist as the long form |
| `virtio` | accepted word | a product name, like `pci` |

Prior art: `rustix` spells every parameter out. Its one exception, `fd`, is a name the kernel owns.
`cortex-m` spells `dmb`, `dsb`, `isb` because ARM owns them. This tree applies the same rule to
`cntfrq` on aarch64 and cannot apply it on the other two. Concrete example: a reader on
`x86_64` who sees `cntfrq()` searches for a register that does not exist; one who sees
`counter_hertz()` does not.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may
revisit. These are names in an unreleased crate, and a crate
tag is a pre-release. The table above is the rule as written, with `counter_hertz` matching the
ratified `counter_frequency_contract` method `hertz`. The open row takes the spelled-out form:
`r8` to `w64` (`src/mapped_window.rs` lines 100 to 130, 147 call sites) become `read_u8` to
`write_u64`. Removing them in favor of the existing `read::<T>` (line 84) is the stricter option,
and it stays open for the architect at ratification. The premises hold at `src/clock.rs` lines 45,
72 and 128 and `src/mapped_window.rs` lines 37, 66 and 145. Item 13 is the work. Ratifying the
names stays the architect's.

### Decision 2: how a program outside nife links against the layout

Background. `link.ld` is interface 20, the entry registers and link layout. Today nine build
scripts in nife reach it by a relative path into this crate's directory, and that path stops
existing at the move. A consumer of the released crate needs a documented way to link with the script. The consumer is a program built outside nife, as described by the SDK archive of milestone 607 (nife is built and updated from packages, and its source divides by purpose). That way is a
published behavior from the first release on.

Recommendation: the embedded Rust mechanism. This crate's `build.rs` copies `link.ld` into
`OUT_DIR` and emits `cargo:rustc-link-search`; a program links with `-C link-arg=-Tlink.ld` and
no path. The script's base stays `0x60000000`, and the test that checks it against
`address_space_map::IMAGE_BASE` moves here. The alternative, each consumer vendoring a copy, is
what milestone 73 (name the aarch64 files aarch64, before x86_64 makes it worse) refused ("two copies would be two places to be wrong about the base") and the
move would make it nine copies.

Prior art: `cortex-m-rt` (`link.x`, 10.6 million downloads) and `riscv-rt` (`link.x`) do exactly
this, and every embedded Rust binary's `.cargo/config.toml` carries `-Tlink.x`. Concrete example:
`jig/build.rs` becomes one `println!("cargo::rustc-link-arg=-Tlink.ld")`.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door until `disk-tools`
leaves (milestone 1017 step 4), which the architect may revisit before then. No repository outside
nife links against the layout yet. Once `disk-tools` leaves, the mechanism is a published
convention, so this ruling must stand or change before that step. The crate delivers `link.ld`
the way `cortex-m-rt` and `riscv-rt` deliver `link.x`, as recommended above, and no consumer
vendors a copy. Item 5 is the work.

### Decision 3: the receive surface before the first tag

Background. Ten `receive_*` functions project the same five kernel words ten ways, in three
return shapes. Every one is public, and after the first tag removing one is a breaking change.
The crate's own `Request` and `Delivered` types already carry everything a receive returns except
the sixth word (the death label) and the bound notification's word.

Recommendation: before the first tag, add one `Message` record (`w0`, `w1`, `w2`, `badge`, `label`,
`delivered: Delivered`) and one `Received` enum (`Message(Message)` or `Notification(u64)`). Build
both from one `receive_message(slot)` over `invoke6` and one `receive_capability_message(slot)`.
Keep `receive`, `call` and `receive_request` as the thin hot-path wrappers callers write most, and
mark the other seven for removal in the changelog. The stricter surface is the smaller one; loosening
later is compatible and tightening is not. If the architect prefers to ship the ten, the block
records that the surface is provisional in the README and the first tag is a pre-release, which
the crate-tag ruling already makes it.

Prior art: seL4's `seL4_Recv` returns the badge through one out-parameter on every receive and
has no `_badged` twin; Zircon's `zx_channel_read` returns bytes and handles from one call.
Concrete example: `receive_capability_bound` has no caller in the tree today and would still be a
published function after the tag.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which is a pre-release, and the architect may revisit it. The smaller surface, as recommended
above: a wrapper added back later is compatible, and one removed is not. The ten functions are at
`src/inter_process_communication.rs` lines 101 to 498, and `receive_capability_bound` (line 224)
has no caller outside the crate. Item 15 is the work.

## Done when

The checks are in [the appendix](1112-user-mode-runtime-outside-review/done-when.md).

## BUGS

- Item 6's premise, firmware that leaves `CNTFRQ_EL0` at zero, is from reading, not from a board.
  The fix costs one compare and is worth landing without one.
- Item 14 depends on a kernel property (every object type refuses an unknown method with
  `BadMethod`) that this review did not verify for every type and that no host test can.
- Item 4 says what the coverage and mutation jobs will do with a crate that has no test target
  from reading the template; the review did not run them.
- Two of the dry run's findings are against the template, not the crate: its scaffold files
  mention nife on ten lines, and its pull request template names the architect. Until the
  template fixes them, every move's dry run reports them, and the move lane for this crate cannot
  clear them from inside the crate.
- The template's spelled-out-word and unit-symbol gates are not in release v7, which the dry run
  pins. Item 13's counts come from running the helpers at the template's head; the ceiling they
  enforce arrives with the release that carries them.
- The review ran no mutation census (outside the window) and found none recorded; the crate has
  no row in the mutation baseline.

## Index row

An outside review of `user_mode_runtime` before it leaves. It finds two soundness fixes (a generic
window read safe code can misuse, two barriers declared `nomem`). It finds three move blockers (the
manifest and files, a test target the template's checks need, the link script's delivery). It adds
six items before the first tag and six for later. Three decisions go to the architect: the names,
the link mechanism, and the receive surface.
