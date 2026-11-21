---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1112. `user_mode_runtime`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/user_mode_runtime` at the head of `main`, read the way a
maintainer meeting it for the first time would read it. It changes nothing; it lists what the next
lanes change.

The review began before the crate had a package in `notes/crate-reviews/`, so it had no settled baseline to
inherit. It treated as settled only what a ruling elsewhere already fixes. That is the crate name,
`VERSION` and `INTERFACE_ID` (the names every contract crate carries), `INTERFACE_ID = 20` for the
entry registers and link layout, and the ratified function names the `Name:` blocks record. It is also the
order of extraction in milestone 1017 (the contract crates leave in this order). That order places this
crate in step 3, after `abi`, `address_space_layout`, `current_cpu_contract` and the five contract
crates it links. Nothing here reopens those. Where a later rule contradicts a recorded ruling, the
item says so.

Two rulings touch `src/socket_frame.rs`: a send's length rides in the `CALL` argument word
(#3517), and `ACCEPT`'s second reply word carries the peer. The items below hold under both, which
change only documentation and one example in this crate.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-move --dry-run`, the template's name and prose gates, cargo-llvm-cov, cargo-mutants.
No new tool.

## Goal

A stranger who finds `user_mode_runtime` in its own repository can tell in one screen what it is
for. It holds the one system call wrapper, the entry macro, the panic handler and the typed windows over
memory the kernel handed a process. It covers three architectures. The same stranger can build it for all three
targets and run its host tests from the README alone. The stranger can also trust that every safe
function is safe to call with any argument. A wrong argument panics or meets a refusal from the
kernel, and it never reads or writes memory the type did not promise. The repository's required checks are green for the
crate's own reasons.

## Finding status

- P0-1, `MappedWindow::read<T>` and `write<T>` take any type at any offset: Done, [#3580](https://github.com/nifeos/nife/pull/3580). They
  take the sealed `WindowInteger`, and a misaligned address panics.
- P0-2, two barriers declared `nomem`: Done, [#3527](https://github.com/nifeos/nife/pull/3527).
- P0-3, the manifest and the files the standalone check refuses: Done,
  [#3815](https://github.com/nifeos/nife/pull/3815) (maintainer, provisional, 2026-11-19 UTC). The
  manifest writes `license`, `lints`, `rust-version`, a description, keywords, categories, `readme`
  and a publish comment as literals; the crate has a README for three targets and both license
  files; `jig`'s dependency carries a version. The `repository` key waits for the move.
- P0-4, no test target: Done, [#3611](https://github.com/nifeos/nife/pull/3611). Two doctests stay `ignore` with a stated reason
  (`src/process.rs` lines 124 and 170).
- P0-5, build scripts reach `link.ld` by a relative path: Done, [#3611](https://github.com/nifeos/nife/pull/3611).
- P1-6, a zero `CNTFRQ_EL0` divides by zero: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-7, the aarch64 child stub hard-codes two `abi` constants: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-8, text that speaks to a reader inside nife: Open. Nine source files cite a section, about 64
  lines cite a milestone, and 29 mention `notes/`. `notes/user-mode-runtime.md` and its directory
  move into the crate's `docs/`, and the externalized-crates ceiling falls to zero.
- P1-9, twenty-four acronyms not expanded at first use: Done,
  [#3815](https://github.com/nifeos/nife/pull/3815) (maintainer, provisional, 2026-11-19 UTC). The
  `prose_acronyms` and `readme_acronyms` rows left the template baseline.
- P1-10, `Frame::payload_read` trusts a reply length: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-11, `INITIAL_RAMDISK_VIRTUAL_ADDRESS` is a literal: Done, [#3611](https://github.com/nifeos/nife/pull/3611).
- P2-12, an unused dependency on aarch64: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-13, names the abbreviation gate flags: Done, [#3611](https://github.com/nifeos/nife/pull/3611).
- P2-14, two spellings of the empty-slot probe: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-15, ten receive functions in three shapes: Done, [#3611](https://github.com/nifeos/nife/pull/3611).
- P2-16, a stale reason for `socket_frame` living here: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-17, no wrapper for `SYSTEM_CALL_EXIT_THREAD`: Done, [#3629](https://github.com/nifeos/nife/pull/3629) (maintainer, provisional,
  2026-11-15 UTC); the docs of `exit` say why none is needed.
- Decision 1, the rename table: Done, [#3611](https://github.com/nifeos/nife/pull/3611) (maintainer, provisional, 2026-11-14 UTC).
- Decision 1, the sized accessors: Done, [#3815](https://github.com/nifeos/nife/pull/3815)
  (maintainer, recommendation B, provisional, 2026-11-19 UTC). the maintainer removed `read_u8` to `write_u64`
  and every call site uses `read::<T>` and `write::<T>`; a two-way door until the first tag.
- Decision 1, ratifying the names: Done, ruled 2026-11-20 UTC. The names stand as built with the short
  forms spelled out (`header_length`, `fill_random_bytes`, `read_volatile`, `write_volatile`,
  `reap_thread`, `counter_ticks`, and spelled-out parameters), the old spellings kept as `doc(alias)`,
  and every `Name:` block in the crate ratified.
- Decision 2, `link.ld` on the link search path: Done, [#3611](https://github.com/nifeos/nife/pull/3611) (maintainer, provisional,
  2026-11-14 UTC). A two-way door until `disk-tools` leaves (milestone 1017 step 4), so not calef's
  until then.
- Decision 3, one `Message` and one `Received`: Done, [#3611](https://github.com/nifeos/nife/pull/3611) (maintainer, provisional,
  2026-11-14 UTC).
- Review package: Done, [#3815](https://github.com/nifeos/nife/pull/3815): the package is
  `notes/crate-reviews/user-mode-runtime.md`, with its index row.
- Clearing the review package, which approves the repository and the move: Done, ruled 2026-11-21 UTC,
  option A. The repository `nifeos/user_mode_runtime` is approved as `publish = false`, with no tag and no
  publish. The move waits until `abi`, `address_space_layout`, `current_cpu_contract` and `socket_contract`
  have left the tree and are git pins (milestone 1017 step 3). The thin machine checks (13 unit tests, 9
  doctests mostly `no_run`, no Kani harness, no fuzz target, no Miri or mutation baseline, most of the
  public functions covered only by QEMU boot tests) are a recorded limitation; a first tag needs its own
  ruling.

## Open decisions

### Ratify the names of `user_mode_runtime`

Background. This crate is what every nife program links: the system call wrapper, the entry macro, the
panic handler and typed windows over memory the kernel hands a process. It leaves nife for its own
repository, and a crate leaves with ratified names. The maintainer ruled the renames on 2026-11-14
(UTC), provisionally, and #3611 built them; each renamed name carried a provisional `Name:` block until the ruling below.
The maintainer ruled on 2026-11-19 UTC, provisionally, to remove the sized window accessors
`read_u8` to `write_u64`, and #3815 built it, so a window has one spelling, `read::<T>` and
`write::<T>` (now `read_volatile` and `write_volatile`). After the first tag a name is a published surface. The package
(`notes/crate-reviews/user-mode-runtime.md`) lists every public name with its status and a
proposal.

Ruled 2026-11-20 UTC: A, with the short forms spelled out (see the status list).

Recommendation: A. Ratify the names as built, from the package's tables.

- A. Ratify the names as built.
- B. Ratify them except the rows the architect names, which keep their provisional blocks.

Door: one-way at the first tag. Before it every name is two-way; after it, a rename or a removal is
a breaking release for every program that links the crate.

Prior art. Linux spells one function per width (`readb`, `readl`, `readq`), and OpenBSD does the
same (`bus_space_read_1` to `_8`), because C has no generic. `tock-registers` and the `volatile`
crate take one generic over an integer trait (`UIntLike`, from memory), as `WindowInteger` does
here. nife differs from Linux and OpenBSD in having the generic already, so the sized forms are a second
spelling of the same call. Concrete example: `window.read_u32(0x70)` and `window.read::<u32>(0x70)`
read the same register before the ruling; now only the second compiles.

What it blocks: the move, and with it the tag.

### Clear the review package

Background. A crate leaves nife only after calef clears its review package, and clearing approves
its repository (the rule of 2026-11-01 UTC). The package exists
(`notes/crate-reviews/user-mode-runtime.md`). Every nife program and `disk-tools` depend on this crate, so it leaves at step 3 of milestone
1017, after the contract crates it links.

Ruled 2026-11-21 UTC: A. The move waits for the four path dependencies above to leave the tree.

Recommendation: A, clear the package, now that it and P0-3 have landed; milestone 1017 step 3 times the move.

- A. Clear when the package and the manifest work land.
- B. Clear only after P1-8 and P1-9 land as well, so the repository opens with no nife-facing text.
- C. Keep the crate in nife. Refused: `disk-tools` and every program outside the kernel use it, and
  a crate used by more than one repository leaves.

Door: one-way. Clearing creates a public repository, and nife, basalt and `disk-tools` then pin it.

Prior art. `cortex-m-rt` lives in its own repository and every embedded binary pins it, which is
the shape Decision 2 copied for `link.ld`. nife cleared its xHCI, e1000e and FAT packages one
approval each. Concrete example: after clearing, `script/crate-move` creates
`nifeos/user_mode_runtime`, and each program's manifest pins it by a full commit.

What it blocks: the move, and with it `disk-tools` leaving (milestone 1017 step 4).

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
| `aspace`, `aspace_slot` | `address_space_slot` | abbreviation; `address_space_layout_into` already spells it |
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
| `r8` ... `w64` | removed; `read::<T>` and `write::<T>` stay | the maintainer's ruling of 2026-11-19 UTC; Linux spells `readb`/`writeq`, OpenBSD `bus_space_read_1`; `read<T>`/`write<T>` already exist as the long form |
| `virtio` | accepted word | a product name, like `pci` |

Prior art: `rustix` spells every parameter out. Its one exception, `fd`, is a name the kernel owns.
`cortex-m` spells `dmb`, `dsb`, `isb` because ARM owns them. This tree applies the same rule to
`cntfrq` on aarch64 and cannot apply it on the other two. Concrete example: a reader on
`x86_64` who sees `cntfrq()` searches for a register that does not exist; one who sees
`counter_hertz()` does not.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may
revisit. These are names in an unreleased crate, and a crate
tag is a pre-release. The table above is the rule as written, with `counter_hertz` matching the
ratified `counter_frequency_contract` method `hertz`. The sized accessors `r8` to `w64`
were renamed `read_u8` to `write_u64` and then, by the maintainer's ruling of 2026-11-19 UTC
(provisional), removed in favor of `read::<T>` and `write::<T>`. The renamed items are at
`src/clock.rs` lines 48, 82 and 142 and `src/mapped_window.rs` lines 39, 94 and 203. Item 13 is
the work. Ratifying the names stays the architect's.
The abbreviation gate reads a bare `address` as ambiguous, so three rows land qualified:
`virtual_address_space_slot`, `virtual_address_space_list` and `destination_network_address`.

### Decision 2: how a program outside nife links against the layout

Background. `link.ld` is interface 20, the entry registers and link layout. Today nine build
scripts in nife reach it by a relative path into this crate's directory, and that path stops
existing at the move. A consumer of the released crate needs a documented way to link with the script. The consumer is a program built outside nife, as described by the SDK archive of milestone 607 (nife is built and updated from packages, and its source divides by purpose). That way is a
published behavior from the first release on.

Recommendation: the embedded Rust mechanism. This crate's `build.rs` copies `link.ld` into
`OUT_DIR` and emits `cargo:rustc-link-search`; a program links with `-C link-arg=-Tlink.ld` and
no path. The script's base stays `0x60000000`, and the test that checks it against
`address_space_layout::IMAGE_BASE` moves here. The alternative, each consumer vendoring a copy, is
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

## Follow-on

- **Outstanding.** The findings "Finding status" marks Open, each line read against
  `crates/user_mode_runtime`.

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
