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

The review's accepted baseline is the disposition in force for this crate in the releasable-crates
note (`notes/releasable-crates-*.md`): `line_editor` moves after a cut, because its `component`
module links nife's `capability_plan`, and the cut leaves that module here first. It takes the
crate name (ratified), `Event::Suspend`, `Event::Tab` and the badge and opcode values of `proto` as
settled, together with §232 (the `line_editor` swap contract) and §295 (the line editor's crate
root splits into module files). No review package exists for this crate under
`notes/crate-reviews/`, so the baseline is thinner than other crates'.
Three names #2797 ratified, `LINE_MAXIMUM`, `PROMPT_MAXIMUM` and `RAW_QUEUE_MAXIMUM`, are reopened here. The later unit rule (design/naming.md, "Units": a limit opens `MAXIMUM_` and ends with its unit) contradicts them. The rule in force is that a release review rechecks a name ratified before a later rule.

Reuse: the template `nifeos/crate-workflows` and its helpers (`spelled_out.py`,
`abbreviations.py`, `standalone.py`), cargo-fuzz, Kani, cargo-mutants. No new tool.

## Where this stands

Finished: the cut of decision 1 and the one `QUIESCED` (items 2 and 8, #3587). Items 5 to 7 and 9 to 14 (README, bindings, fuzz targets, Kani harnesses, handoff layout, P2 fixes) landed in #3566.

- Item 3 (public names): `LineEditor` and `HISTORY_LINES` ruled provisionally by the maintainer (#3589), not built, a lane; the three limits, decision 3, a decision for calef.
- Item 4 (gate spellings): two-way, not built; a lane for the engine's fields with item 3, and the contract crates' own worklists for `terminal_contract` and `line_editor_component`.
- Item 1 (manifest, licenses, citations): not built; the move, which waits on milestone 1017's order, the names of item 3 and calef's review clearance for the repository.
- `terminal_contract` interface identity: open; decision for calef.

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
   Change: as decision 1 below rules. `handoff` stays: `save` and `restore` read the engine's
   private fields, and the crate documents the layout as its own.
   Door: one-way for the new crate's name and for the shape of the published `line_editor` (no
   `proto` module); decision 1 below. Effort: M.

### P1: before the first tag

The P1 items are in [the appendix](1098-line-editor-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1098-line-editor-outside-review/later-items.md).

## Follow-on

- **Outstanding.** Items 1, 3 and 4: the move commit's manifest and license work, the public names
  (Decisions 2 and 3) and the gate spellings that follow the cut. The ledger names each with its
  reason.
- **Done.** Item 2, as decision 1 rules: `terminal_contract` and `line_editor_component` carry
  the cut, and `line_editor` depends on nothing. Item 8: a `const` assertion in the `line_editor`
  program holds the two `QUIESCED` values equal.
- **Outstanding.** Whether `terminal_contract` takes an interface identity; decision 1 records it
  as open.

## Decisions for the architect

Each gives the background first, then the recommendation.

### Where the cut falls: `proto` and `component` leave the crate, `handoff` stays

Background. The disposition in force says `line_editor` moves after a cut that leaves `component`
in nife, because that module links `capability_plan`. `component` is not alone: `proto` is the
same kind of thing, the opcode and badge table of nife's terminal contract, and it is what 32 of
the 38 callers come for. `handoff` is different: its two cursor types depend on nothing, and
`save`/`restore` read the engine's private fields, so it is the engine's own serialization. Once
the crate leaves, the modules it carries are its public shape, which others build on.

Recommendation. Cut `proto` and `component` together into one nife wire-contract crate,
provisionally `terminal_contract`, and keep `handoff` in `line_editor`. The engine then has one
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
provisional, and its published name waits on the naming-layers decision. The kernel,
`std_runtime_contract`, `fixtures`, `system_initializer`, `system_tests` and every program that
speaks the protocol depend on it, not on `line_editor`. `component` stays beside the terminal
programs as the nife-only crate `line_editor_component` (name provisional), and `capability_plan`
does not leave. `handoff` stays in the engine. `line_editor` depends on nothing
and is listed in `crates/externalization-list.txt`. The leaf states `QUIESCED`, and the
`line_editor` program holds it equal to `hot_swap_contract::QUIESCED` at compile time (item 8).
Open: `terminal_contract` carries no interface identity (an interface number, a version), and
whether it takes one under milestone 1017's family rule is not decided; none is minted.

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

### The three ratified limits, reopened under the unit rule

Background. #2797 ratified `LINE_MAXIMUM`, `PROMPT_MAXIMUM` and `RAW_QUEUE_MAXIMUM` in the same
sweep that renamed `MAXIMUM_NAME_BYTES` and `MAXIMUM_TERM_BYTES` elsewhere, before anyone wrote the units rule
down as a rule. The rule in force now reads: a limit opens `MAXIMUM_` and ends with its
unit, and the release review rechecks a ratified name. These three are byte lengths.

Recommendation. Rename before the tag to `MAXIMUM_LINE_BYTES`, `MAXIMUM_PROMPT_BYTES` and
`MAXIMUM_RAW_QUEUE_BYTES`, with no alias.

Prior art. POSIX `MAX_CANON` and `MAX_INPUT` are the same two limits with the unit implied;
linenoise's `LINENOISE_MAX_LINE`; the tree's own `MAXIMUM_SURFACE_BYTES` and
`MAXIMUM_UDP_MESSAGE_BYTES`, ratified in the same pull request.

Concrete example. `components/src/swish/prompt.rs` line 321 declares
`[0u8; line_editor::LINE_MAXIMUM]` for a completion buffer; with the unit in the name the reader
sees it is bytes, not columns, which the one-row `BUGS` entry makes a live distinction.

## Done when

The checks are in [the appendix](1098-line-editor-outside-review/done-when.md). [The ledger](1098-line-editor-outside-review/ledger.md) says, item by item, what this block finished and what waits.

## BUGS

- No review package exists for this crate under `notes/crate-reviews/`, so this block had no list
  of accepted names and proposed items to measure itself against; it took the releasable-crates disposition, the two notes and #2797 as the baseline. A recording lane should write the package or say that this block is it.
- The mutation figures come from records, not a re-run, and predate the module split and the unit
  renames; the census that confirms them belongs in the template's window.
- The behavior of `noline`'s initialization comes from `notes/line-discipline.md`, and the review did not
  re-verify it against its source.
- Whether `abbreviations.py` is a gate or a report in the template's `check` job stays
  unverified; item 4 lists its 40 findings either way.

## Index row

An outside review of `line_editor` before it leaves. Two items block the move (the crate does not stand alone, and the contract modules are not yet cut out of it). Six land before the first tag. Six wait for later. Three one-way doors go to the
architect: where the cut falls, the engine's type and history names, and the three limits reopened
under the unit rule.
