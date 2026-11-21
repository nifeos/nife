---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 972, 1017
decision_dependencies: 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1078. `boot_ladder`: what an outside maintainer would fix before it leaves

*(The number is the recording lane's. The title and slug are provisional.)*

Raised by a read-only outside review of `boot_ladder` as a stranger to this tree would meet
it. It changes nothing; it lists what the next lanes change before the crate moves to its own
repository and before any tag.

The crate has no review package in `notes/crate-reviews/`, so this review has no settled baseline
to inherit. The one ruling it does inherit is the crate name, `boot_ladder`, ratified in the
crate's own `Name:` block with four refusals, and it does not reopen it. Every other public name
in the crate is unratified (`script/names boot_ladder` and `helpers/name_release.py --draft`
list eight items with no `Name:` block and two marked provisional).

Reuse: the crate template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-review-packet`, `cargo kani`. No new tool.

## Goal

A stranger who finds `boot_ladder` in its own repository can tell in one screen what it holds. It holds the fixed heads of the lines a nife kernel and shell print while booting, and the list of self-test check names. A program reading a serial log, a CI transcript or a photograph of a screen then keys on the same bytes the kernel wrote. The same stranger can build and test it from the README
alone, and the repository's gates are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1078-boot-ladder-outside-review/what-was-checked.md).

## The crate in one paragraph

`src/heads.rs` holds `Heads`, built from a system's name, whose seven accessors (`banner`,
`machine`, `self_test`, `self_test_subject`, `core_runs`, `shell_banner`, `screen_held`) each give
a `Head` that prints through `Display` and matches a line in place. `src/rungs.rs` holds the
verdict's two inner heads (`SELF_TEST_FAILED`, `SELF_TEST_ABSENT`), the fifteen check names
(`SELF_TEST_CHECKS`) and the two older list lengths (`EARLIER_SELF_TEST_CHECK_COUNTS`).
`src/verdict.rs` holds the verdict's grammar: `Verdict` prints it and `ParsedVerdict` reads it.
The pages and slots the kernel, the progenitor and the self-test program agree on are not in this
crate; they are nife's `self_test_contract`, which depends on it. The crate is `no_std`, with no dependencies and no `unsafe`. The kernel
prints its heads, the self-test program prints the verdict, the shell prints its banner, and
`serial_console` and `xtask` read them through the same code. Soundness has nothing to review.
Correctness lives in the agreement between the printer and the readers, and that is where the items
below sit.

## The items

Effort: S is half a day for a lane, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after a release (a public name, a printed wording that logs and profiles already hold, a layout, a license). Everything else is two-way, and the maintainer decides it.

### P0: blocks the move

The P0 items are in [the appendix](1078-boot-ladder-outside-review/blocks-the-move.md).

### P2: later

The P2 items are in [the appendix](1078-boot-ladder-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1078-boot-ladder-outside-review/before-the-tag.md).

## Decisions for the architect

Each states the background first, then the recommendation, prior art and a concrete example.
The architect ruled the heads, the printed check names and the public names, below. The review
clearance that is the go for the repository waits; it is under [Open decisions](#open-decisions).

### The heads are fixed shapes after the caller's system name

Ruled 2026-11-14 (UTC), option A. The crate names no system. It fixes the shape of each head and
takes the system's name from its caller. `Heads::new(name)` yields the seven heads that begin with
the name (`banner`, `machine`, `self_test`, `self_test_subject`, `tour`, `prompt`, `screen_held`).
Each is a `Head` that prints through `Display` and matches a line in place, so neither side
allocates and the kernel can use it. nife passes `nife`, so every byte printed and matched is the one stored
logs, board profiles and gates already hold, and a test in `serial_console` pins the nife heads to
those bytes.

The ruling ratifies the nine shapes as printed. They are `<system> on `, `<system> machine: `,
`<system> self-test: `, `<system> self-test subject: thread `,
`<system>: the capability core runs on `, `<system> capability shell`,
`<system> screen: held for the host; `, and inside the verdict `FAILED: ` and `absent: `. The shell
banner keeps its text and takes no period. The names of the accessors and constants are
ratified in the last ruling below.

Prior art (recalled, not read in this review, unless stated): seL4's `sel4test` ends a green run with one fixed sentence that its CI harness matches. Zephyr's `twister` console harness matches `PROJECT EXECUTION SUCCESSFUL` or `FAILED`. Linux's KTAP (`Documentation/dev-tools/ktap.rst`)
freezes `KTAP version 1` as the head of every report. In each, the first release of the reader freezes the sentence. Where nife differs: the heads live in a crate both sides link rather than
in a document, and the system's name is a parameter of that crate.

Example: a transcript holding
`nife self-test: 12 of 15 passed, 3 absent: block_device clock true_random_number_generator` is
green under the released crate and under every later one, because the verdict's shape and the
grammar behind it never change. Only the list length and the names may change, and the
earlier-lengths list says which totals an older kernel printed.

### The printed check names spell out their words

Ruled 2026-11-14 (UTC). Of the fifteen names the verdict prints, `blockdev` becomes `block_device`
and `trng` becomes `true_random_number_generator`. The ruling ratifies the
other thirteen as printed, `device_interrupt` included. The self-test program's slot and the kernel half's flag that name the
generator follow: `TRUE_RANDOM_NUMBER_GENERATOR` and `TRUE_RANDOM_NUMBER_GENERATOR_ANSWERED`. The
crate keeps no list of old spellings: a reader matches the current names only, and a stored log that
prints an older one is history.

Prior art: Linux kselftest names its suites by directory and abbreviates freely (`bpf`, `kvm`,
`mm`); nife's own naming rule refuses that. seL4test names tests by code (`SCHED0001`), which
needs a decoder. Zephyr names them dotted (`kernel.common`). Where nife differs: a reader takes a check name off a photograph with no decoder at hand, so a word beats a code and a spelled word beats an abbreviation.

Example: `nife self-test: 12 of 15 passed, 3 absent: block_device clock true_random_number_generator`,
and radon's profile forbids `entropy`, `true_random_number_generator` and `clock` absent.

### The public names of the heads and the check lists stand as built

Ruled 2026-11-19 (UTC), option A. Every name stands as built. The accessors on `Heads` are `banner`,
`machine`, `self_test`, `self_test_subject`, `core_runs`, `shell_banner` and `screen_held`. The
constants are `SELF_TEST_FAILED`, `SELF_TEST_ABSENT`, `SELF_TEST_CHECKS` and
`EARLIER_SELF_TEST_CHECK_COUNTS`. The printed bytes stay as ruled 2026-11-14 (UTC), so the shell
banner keeps its text and takes no period. The names in `verdict` are outside this ruling and go
to the crate's release name review; `kernel_half` and `self_test_program` are nife's, in
`self_test_contract`.

Prior art: systemd's readiness protocol names its messages for what they say (`READY=1`,
`STATUS=`), and its reader and writer share `sd_notify`'s definition. U-Boot's `bootstage` names
each stage for its event (`BOOTSTAGE_ID_START_UBOOT_F`) in one header its tools include. Both are
recalled, not read in this review. Where nife differs: a name here is a Rust item a reader
imports, so a misnomer is in their code too.

Example: a reader written against the released crate calls `Heads::new("nife").shell_banner()` and
knows without the `BUGS` entry that it is not waiting for a `$ `.

## Prior art

The comparison with nearby work is in [the appendix](1078-boot-ladder-outside-review/prior-art.md).

## What needs the architect

The review clearance, under [Open decisions](#open-decisions). The architect has ruled on the heads,
the check names and the public names.
Nothing else here is one-way: the manifest, the README, the tests, the `Verdict` type and the
nife-side gate are the maintainer's.

## Done when

The checks are in [the appendix](1078-boot-ladder-outside-review/done-when.md).

## Finding status

- P0-1, manifest stands alone: Done. The license, `rust-version`, description, readme, keywords,
  categories and lints are literal ([#3655](https://github.com/nifeos/nife/pull/3655)), and the
  move wrote the `repository` line.
- P0-2, the files a stranger needs: Open. The README, both licenses, `CHANGELOG.md`,
  `_typos.toml` and `docs/stranger-test.md` exist in nifeos/boot_ladder. No stranger run is on
  record yet, and the README's proof steps are nifeos/boot_ladder#1.
- P0-3, shipped text points into the tree: Done, [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3655](https://github.com/nifeos/nife/pull/3655). Tree references remain only in `Name:` blocks,
  which the move strips.
- P1-4, one verdict code for the writer and the readers: Done,
  [#3655](https://github.com/nifeos/nife/pull/3655). The kernel prints through `Verdict`, and
  `serial_console` and `xtask` read through `ParsedVerdict`.
- P1-5, check list invariants: Done, [#3565](https://github.com/nifeos/nife/pull/3565). A test
  (`the_check_names_are_plain_and_distinct`), not a `const` block, because a `const` block drew five
  mutation survivors no test can kill.
- P1-6, tests and proofs: Done, [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3655](https://github.com/nifeos/nife/pull/3655). Two Kani harnesses in `src/verdict.rs`, each
  with a falsification patch.
- P1-7, head literals in `xtask`: Open. The literals are gone
  ([#3655](https://github.com/nifeos/nife/pull/3655)); the `script/lint` section that refuses a head
  literal outside `boot_ladder`, tests and fixtures is not written, and as a new lint section
  it is a reviewed interface.
- P1-8, the ten public names: Done, [#3589](https://github.com/nifeos/nife/pull/3589),
  [#3655](https://github.com/nifeos/nife/pull/3655). Ratified as built (ruled 2026-11-19 UTC).
- P1-8b, the `self_test_program` names: out of this crate. The module is in nife's
  `self_test_contract`, so its names, and the proposed change of
  `DIRECT_MEMORY_ACCESS_PEEK_VIRTUAL_ADDRESS` to `DIRECT_MEMORY_ACCESS_OBSERVER_VIRTUAL_ADDRESS`,
  are nife's to rule and not this review's.
- P1-9, the boundary claim and the matching rule: Done,
  [#3586](https://github.com/nifeos/nife/pull/3586),
  [#3655](https://github.com/nifeos/nife/pull/3655).
- P2-10, abbreviated check names: Done, [#3586](https://github.com/nifeos/nife/pull/3586). The
  kernel's name column is still ten wide, which the README's `BUGS` records.
- P2-11, a version policy: Done, [#3655](https://github.com/nifeos/nife/pull/3655). The README
  states it; no identity constant exists.
- P2-12, growth already proposed: Open. Milestone 972 (the boot stick's shared names are code) and
  the two hoists of soak markers and job-mix fields wait; each lands in nifeos/boot_ladder and moves
  the pin.
- P2-13, the banner's tail: Done, [#3565](https://github.com/nifeos/nife/pull/3565),
  [#3586](https://github.com/nifeos/nife/pull/3586). The crate states that the tail is free text and
  a reader takes the architecture from the machine line.
- Decision, heads after the caller's system name: Done (ruled 2026-11-14 UTC, option A),
  [#3586](https://github.com/nifeos/nife/pull/3586).
- Decision, the ten public names: Done (ruled 2026-11-19 UTC, option A), built in
  [#3589](https://github.com/nifeos/nife/pull/3589).
- Decision, printed check names spelled out: Done (ruled 2026-11-14 UTC),
  [#3586](https://github.com/nifeos/nife/pull/3586).
- Decision, clear the review: Done (ruled 2026-11-21 UTC, option A). The crate moves with
  `publish = false`, no tag and no publication, and carries only the console lines; `kernel_half`
  and `self_test_program` stay in nife as `self_test_contract`.
- The move: Done. The review package is
  [notes/crate-reviews/boot-ladder.md](../../notes/crate-reviews/boot-ladder.md). The crate lives in
  [nifeos/boot_ladder](https://github.com/nifeos/boot_ladder), and nife depends on it by git and
  `rev`, a commit on its `main`, with `version = "0.1.0"`. It keeps `publish = false`; no tag
  exists, and crates.io holds no release.

## Open decisions

None. The publishing hold stands.

## Follow-on

- **Outstanding.** The Open lines under [Finding status](#finding-status).

## BUGS

- The prior-art rows come from memory and say so. A lane that writes the README should read each source before citing it.
- Item 7's gate is nife's, not the crate's, and is not written yet. After the move the crate cannot know that a consumer
  spelled a head by hand; only the consumer's own lint can.
- The version policy adds no checkable version. A tenth rung that printed one would itself be a
  wording to freeze, and this review found no reader that needs it yet.
- The review did not run the template's name-check helpers over the crate; `script/names` and
  `helpers/name_release.py` in nife cover the same ten names, and the printed check names are
  string literals no helper reads.
- `xtask/src/time_record.rs` line 929 and 1010 (`"nife on riscv64"`, `"nife on x86_64"`) spell the
  banner inside `#[cfg(test)] mod tests`, so item 7 omits them; item 7's gate must exempt test
  modules the same way.

## Index row

An outside review of `boot_ladder` before it leaves found three manifest and file items that block the move. It found six to land before the first tag. They are a shared `Verdict` type so the printer and its three readers agree by construction, `const` invariants on the check list, real tests, eight hand-spelled literals in `xtask`, ten names that are now ratified, and a false boundary claim. It found four items for later. Three rulings hold. The heads are fixed shapes after a system name the caller supplies, ratified as printed. Two check names become `block_device` and `true_random_number_generator`, and the other thirteen stand as printed. The ten public names, three of them renames, stand as built. The crate carries the shared verdict reader, tests, a proof and its README.
