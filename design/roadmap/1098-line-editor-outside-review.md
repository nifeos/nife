---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: 232, 295
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1098. `line_editor`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/line_editor`, read the way a maintainer meeting it
for the first time would. It changes nothing; it lists what the next lanes change, and it puts the
one-way doors to the architect.

The review's accepted baseline is the disposition in the releasable-crates note
(`notes/releasable-crates-*.md`) when the review ran: `line_editor` moved after a cut, because its
`component` module linked nife's `capability_plan`. The note now gives the cut: `line_editor`
moves, `terminal_contract` joins the contract crates and `line_editor_component` stays. It takes the
crate name (ratified), `Event::Suspend`, `Event::Tab` and the badge and opcode values of `proto` as
settled, together with §232 (the `line_editor` swap contract) and §295 (the line editor's crate
root splits into module files). No review package exists for this crate under
`notes/crate-reviews/`, so the baseline is thinner than other crates'.
Three names #2797 ratified, `LINE_MAXIMUM`, `PROMPT_MAXIMUM` and `RAW_QUEUE_MAXIMUM`, are reopened here. The later unit rule (design/naming.md, "Units": a limit opens `MAXIMUM_` and ends with its unit) contradicts them. The rule in force is that a release review rechecks a name ratified before a later rule.

Reuse: the template `nifeos/crate-workflows` and its helpers (`spelled_out.py`,
`abbreviations.py`, `standalone.py`), cargo-fuzz, Kani, cargo-mutants. No new tool.

## Finding status

Every finding not Done blocks the first crates.io publish, which waits until every outside-review
milestone reaches delivery.

- Item 1 (P0), the crate does not stand alone: Open. `Cargo.toml` still inherits `license` and
  carries the allowlist comment; the move commit writes the manifest rows and the license files and
  rewrites the citations.
- Item 2 (P0), the cut: Done, [#3587](https://github.com/nifeos/nife/pull/3587).
- Item 3 (P1), public names the naming rules refuse: Open. Decisions 2 and 3 rule the engine's
  names and the three limits, and neither is built: `struct LineDisc` (`src/line_disc.rs` line 142)
  and `HIST` (line 23) keep their names. `pop8` (`src/raw_queue.rs` line 65) is two-way before the
  tag, so the maintainer rules its new name provisionally and a lane renames it with decision 2.
- Item 4 (P1), names the template's gates refuse: Open. The engine's fields (`buf`, `cur`, `hist`,
  `esc`, `params`, `src/line_disc.rs` lines 143 to 159) rename with item 3; `terminal_contract` and
  `line_editor_component` keep their own worklists, and `proto` joins the template's word list.
- Item 5 (P1), bindings and limits a stranger can read: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 6 (P1), `forbid(unsafe_code)`, fuzz targets and Kani harnesses: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 7 (P1), the handoff layout's version policy: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 8 (P1), one `QUIESCED`: Done, [#3587](https://github.com/nifeos/nife/pull/3587).
- Item 9 (P2), stack copies: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 10 (P2), the `csi_move` bound: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 11 (P2), three repaints with one body: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 12 (P2), the history depth: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 13 (P2), the test terminal's row: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 14 (P2), `Writer::field` width: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Decision 1, where the cut falls: Done, [#3587](https://github.com/nifeos/nife/pull/3587), ruled 2026-11-14 (UTC).
- Decision 2, `LineEditor` and `HISTORY_LINES`: Open. Ruled by the maintainer, provisional,
  2026-11-14 (UTC), in [#3589](https://github.com/nifeos/nife/pull/3589); the rename lane builds it.
- Decision 3, the three limits take their unit: Open. Ruled by the maintainer, provisional,
  2026-11-19 (UTC); the rename lane builds it with decision 2.
- Decision 4, an interface identity for `terminal_contract`: Waiting on the interface number the
  architect mints when freezing the crate's identity. Ruled 2026-11-19 (UTC), A: an interface
  number and version 1, with no magic.
- Decision 5, the name `terminal_contract`: Open. Ruled 2026-11-19 (UTC), A, in [#3796](https://github.com/nifeos/nife/pull/3796); the
  crate's `Name:` block still reads provisional.
- Decision 6, review clearance and the repositories: Waiting on a ruling, the architect's
  clearance of `notes/crate-reviews/line-editor.md` and `notes/crate-reviews/terminal-contract.md`.

## Open decisions

None. The rename lane carries item 3's limits, and the review package carries the go for both crates.

## Goal

A stranger who finds `line_editor` in its own repository can tell in one screen that it is a
sans-IO, allocation-free line editor for a serial terminal. The stranger can read every key binding and every
limit without opening another repository. The stranger can also trust that any byte sequence leaves the editor
in a consistent state. Nothing in the repository names nife, a milestone, a decision section or a
person. The template's gates are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1098-line-editor-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1098-line-editor-outside-review/prior-art.md).

## The items

Effort: S is half a day for a lane, M a day or two, L longer. A door is one-way when somebody
outside the repository acts on it after the release (a public name, a published behavior, a layout
or identity, a dependency); everything else is two-way and the maintainer decides it.

### P0: blocks the move

1. The crate does not stand alone, and its text quotes and dates a person.
   Finding: `standalone.py` lists what the template's `standalone` job will refuse. The list is: no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, `license.workspace = true`, `lints.workspace = true`, no `rust-version`, `publish = false`, 48 milestone citations, 16 paths into the nife tree, 41
   provenance lines, 8 lines naming the architect and 16 section citations. The manifest also
   carries no `description`, `repository`, `readme`, `keywords` or `categories`. Four doc comments
   quote a ruling by name and date in their body, outside any `Name:` block, which the move strips
   and the record rules refuse regardless of the move.
   Evidence: `Cargo.toml` lines 5 to 14; `src/lib.rs` lines 8 to 9 and 23; `src/line_disc.rs`
   lines 157 to 160 (names `kernel/src/user.rs` and `components/src/line_editor.rs`);
   `src/proto.rs` lines 78, 99, 202 and 235; `src/component.rs` lines 110, 150, 171, 226 and 234.
   Change: inline `license = "MIT OR Apache-2.0"` and the two lints the template requires. Add `rust-version`, `description`, `repository`, `readme`. Add the two license files and a README (item 5 gives its content). Rewrite each citation as a statement of the rule in force or a link inside the repository. Delete the quoted rulings and keep the decision. Most of this falls
   away with item 2, since `component.rs` and `proto.rs` hold 60 of the 129 findings.
   Door: two-way. Effort: M.

2. The cut the disposition requires is not made: `component`, `proto` and the `capability_plan`
   dependency are nife's terminal contract, not a line editor.
   Finding: `component` declares slot orders, virtual addresses, page budgets and a supervisor
   protocol for one nife program, and pulls `capability_plan` (which pulls `abi`) into a crate
   whose engine needs neither. `proto` is the IPC framing of the terminal contract: opcodes,
   badges, a quiesce handshake and a swap protocol that only nife programs speak. 32 of the 38
   caller files reach only these two modules; the kernel links the crate for `proto` alone. The "nothing that leaves references back" rule of milestone 1017 (the contract crates leave in this order) cannot hold while `component` names `terminal_supervisor`, `system_initializer`
   and `display_terminal`.
   Evidence: `Cargo.toml` line 14; `src/lib.rs` lines 38 to 68; `src/component.rs` whole;
   `src/proto.rs` lines 63 to 215; `crates/hot_swap_contract/src/lib.rs` line 49 (the duplicate
   `QUIESCED`).
   Change: as decision 1 below rules, built in #3587. `handoff` stays: `save` and `restore` read
   the engine's private fields, and the crate documents the layout as its own.
   Door: one-way for the new crate's name and for the shape of the published `line_editor` (no
   `proto` module); decision 1 below. Effort: M.

### P1: before the first tag

The P1 items are in [the appendix](1098-line-editor-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1098-line-editor-outside-review/later-items.md).

## Follow-on

- **Done.** Item 2, as decision 1 rules: `terminal_contract` and `line_editor_component` carry
  the cut, and `line_editor` depends on nothing. Item 8: a `const` assertion in the `line_editor`
  program holds the two `QUIESCED` values equal.
- **Outstanding.** Every finding marked Open or Waiting on under "Finding status", each checked
  against the crates' sources on main.
- **Outstanding.** The externalization list: `crates/externalization-list.txt` names
  `line_editor` and not `terminal_contract`, checked in that file on main.

## Decisions for the architect

Decisions 1 to 6 are the rules in force.

### Where the cut falls: `proto` and `component` leave the crate, `handoff` stays

Background. The disposition in force says `line_editor` moves after a cut that leaves `component`
in nife, because that module links `capability_plan`. `component` is not alone: `proto` is the
same kind of thing, the opcode and badge table of nife's terminal contract, and it is what 32 of
the 38 callers come for. `handoff` is different: its two cursor types depend on nothing, and
`save`/`restore` read the engine's private fields, so it is the engine's own serialization. Once
the crate leaves, the modules it carries are its public shape, which others build on.

Recommendation. Cut `proto` and `component` together into one nife wire-contract crate,
named `terminal_contract` (decision 5), and keep `handoff` in `line_editor`. The engine then has one
dependency (none) and one job; the contract crate joins milestone 1017's family and takes its
identity rule there. The alternative, cutting only `component`, leaves a published editor whose
largest module is an IPC table for a system the reader does not have.

Prior art. Linux keeps the tty line discipline (`n_tty.c`) apart from the `termios` ABI in
`uapi`; the ABI is what every program includes and the discipline is replaceable. In Rust, `vte`
is the parser and `alacritty_terminal` is the program that speaks to the OS; `noline` keeps its
sans-IO core apart from its sync and async fronts. In this tree, `supervision_contract` and
`hot_swap_contract` already hold protocols apart from the programs that serve them, and
`video_terminal` carries a `component.rs` facing the same cut.

Concrete example. `kernel/Cargo.toml` links `line_editor` to read `proto::req` and
`proto::OPERATION_WRITE` in `kernel/src/user/terminal_print.rs`; after the cut it links
`terminal_contract` and the kernel no longer depends on an editor.

Ruled 2026-11-14 (UTC): the cut falls in three places. `proto`, the terminal protocol's opcodes and
badges, is the wire-only leaf crate `terminal_contract`, with no dependencies; its name is
ratified (decision 5). The kernel,
`std_runtime_contract`, `fixtures`, `system_initializer`, `system_tests` and every program that
speaks the protocol depend on it, not on `line_editor`. `component` stays beside the terminal
programs as the nife-only crate `line_editor_component` (name provisional), and `capability_plan`
does not leave. `handoff` stays in the engine. `line_editor` depends on nothing
and is listed in `crates/externalization-list.txt`. The leaf states `QUIESCED`, and the
`line_editor` program holds it equal to `hot_swap_contract::QUIESCED` at compile time (item 8).
`terminal_contract` takes an interface identity (decision 4).

### The engine's public type and constant names at release: `LineEditor` and `HISTORY_LINES`

Background. `LineDisc` is an abbreviation of "line discipline", a word the crate name ruling
refused as an overclaim; `HIST` is not a word and carries no `Name:` block. After the tag a rename is a breaking change to every user.

Recommendation. `LineEditor` for the type, matching the crate (a crate that holds a component's
logic takes the component's name) and the family (`rustyline::Editor`, `noline::Editor`);
`HISTORY_LINES` for the constant (a count of things names the things). Keep `Event::Eof`, the
standard library's spelling. No alias for the old spellings before the first release.

Prior art. `rustyline` and `noline` call the type `Editor`; linenoise calls its state
`linenoiseState` and its history size `history_max_len`; `reedline` uses the crate name.

Concrete example. `components/src/swish/prompt.rs` holds `static mut EDITOR: line_editor::LineDisc`;
after the rename the declaration reads `line_editor::LineEditor`.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The recommendation is the rule. Item 3 is the work.

### The three limits take their unit

Background. `LINE_MAXIMUM` (256), `PROMPT_MAXIMUM` (64) and `RAW_QUEUE_MAXIMUM` (64) are byte
lengths, and #2797 ratified them. The units rule came later: a limit opens `MAXIMUM_` and ends with
its unit, and a release review rechecks a name ratified before a later rule. A completion buffer in
the shell declares `[0u8; line_editor::LINE_MAXIMUM]`, and a reader cannot tell bytes from columns.

- A. Rename the three as the units rule spells them.
- B. Keep the ratified names and state the unit in each doc comment.

Prior art. POSIX's `MAX_CANON` and `MAX_INPUT` are the same two limits with the unit implied, and
linenoise has `LINENOISE_MAX_LINE`. The tree's own `MAXIMUM_SURFACE_BYTES` and
`MAXIMUM_UDP_MESSAGE_BYTES` carry it. Where nife differs: the unit is always in the name. Example:
`[0u8; line_editor::MAXIMUM_LINE_BYTES]`.

Decided by the maintainer on 2026-11-19 (UTC), provisional: a two-way door until the first tag.
Rule A: `MAXIMUM_LINE_BYTES`, `MAXIMUM_PROMPT_BYTES` and `MAXIMUM_RAW_QUEUE_BYTES`, with no alias.
A rename lane builds it with item 3, not this milestone's pull request.

### An interface identity for `terminal_contract`

Background. `terminal_contract` holds the terminal protocol's opcodes, badges and quiesce value,
which the kernel and every program that speaks the protocol share. Other contract crates in
milestone 1017's family (the contract crates leave in this order) carry an interface number and a
version, which a program declares in its interfaces note and a server checks at spawn.
`terminal_contract` carried neither, so a program built against an older table passed unchecked.

- A. An interface number and a version, with no magic.
- B. No identity: the protocol is register-only, and the badges already gate the operations.
- C. An identity and a magic word, as the page contracts carry.

Prior art. In this tree `entropy_contract` (12, version 1) and `resolver_contract` (11, version 1)
took their identities before they moved. Linux, which has no such check, added `termios2` and
`TCGETS2` beside the old layout rather than change it. Where nife differs: the check runs at spawn,
not at the first call. Example: under A, the shell's note carries
`(terminal_contract::INTERFACE_ID, 1)`.

Ruled 2026-11-19 (UTC): A. The crate takes an interface number and version 1, with no magic. The
architect mints the number when freezing the crate's identity, as for the other contract crates;
it is not chosen yet.

### The name `terminal_contract`

Background. The crate holds the terminal protocol. §323 (the binary interface, and the names of the
project's layers) keeps crate names descriptive and gives the page-contract crates no common prefix,
so nothing else decides it. The repository takes the name at creation.

- A. `terminal_contract`.
- B. `terminal_protocol`, since it holds opcodes and no page layout.

Prior art. The tree's `supervision_contract` and `hot_swap_contract` name a protocol the same
way, and Linux's `uapi` headers name the interface by subsystem (`termios.h`). Where nife differs:
the suffix says the crate is the agreement both sides compile against. Example:
`use terminal_contract::OPERATION_READLINE;`.

Ruled 2026-11-19 (UTC): A. The architect ratifies the name `terminal_contract`, and the repository takes it at
creation.

### Review clearance and the repositories

Background. Clearing a crate review is the go to create its repository under nifeos. `line_editor`
and `terminal_contract` leave; `line_editor_component` stays in nife. Neither leaving crate has a
review package in `notes/crate-reviews/`.

- A. Clear both once the review package exists.
- B. Clear `line_editor` alone first, since it depends on nothing.
- C. Hold until a dry run also stages the move's own work (item 1).

Prior art. In this tree `generational_table`, `block_roster` and `file_allocation_table` each moved
on a cleared review with no tag. Where nife differs: nothing outside nife links either crate.
Example: under A, the shell pins `nifeos/line_editor` by revision.

Decided by the maintainer on 2026-11-19 (UTC), provisional: a one-way door at repository creation,
and the clearance itself stays the architect's. Rule A: one review package covers `line_editor` and
`terminal_contract`, a lane writes it after this pull request, and the architect gives the go for
both from the package, with no tag and no publication.

## Done when

The checks are in [the appendix](1098-line-editor-outside-review/done-when.md).

## BUGS

- No review package exists for this crate under `notes/crate-reviews/`, so this block had no list
  of accepted names and proposed items to measure itself against; it took the releasable-crates
  disposition, the two notes and #2797 as the baseline.
- The mutation figures come from records, not a re-run, and predate the module split and the unit
  renames; the census that confirms them belongs in the template's window.
- The behavior of `noline`'s initialization comes from `notes/line-discipline.md`, and the review did not
  re-verify it against its source.
- Whether `abbreviations.py` is a gate or a report in the template's `check` job stays
  unverified; item 4 lists its 40 findings either way.

## Index row

An outside review of `line_editor` before it leaves. The cut has landed: `terminal_contract` and
`line_editor_component` hold the contract, and the engine depends on nothing. Twelve findings have
landed. The engine's renames and the move's own work remain. The limits' rename, the interface
number for `terminal_contract` and the architect's clearance of the review package remain; the
rulings cover the names and the clearance route.
