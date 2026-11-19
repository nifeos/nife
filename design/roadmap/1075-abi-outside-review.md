---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017, 1034
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1075. `abi`: what an outside maintainer would fix before the move

Raised by a read-only review of `crates/abi` as a stranger would meet it in a repository of its own.
It changes nothing; it lists what the next lanes change. The crate is the last of step 2 of
milestone 1017 (the contract crates leave in this order). No review package exists for it under
`notes/crate-reviews/`, so this block has no settled baseline to defer to: the crate's ruled name,
`moho_abi` (§323), and the sixteen ratified item names stand, and everything else below is open.

Reuse: `script/crate-move --dry-run`, the template `nifeos/crate-workflows` and its helpers,
`helpers/lint_externalized_crates.py`, `helpers/format_values_check.py`, `helpers/name_release.py`,
cargo-mutants' recorded census, the crates.io API. No new tool.

## Goal

A stranger who finds this crate can tell in one screen what it defines.
The stranger can build and test it from the README alone. Every method's contract reads without a path into another repository. A changed number cannot ship without the version changing with it. The repository's gates are green
on arrival for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1075-abi-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1075-abi-outside-review/prior-art.md).

## The items

Effort: S is under half a day for a lane, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after the release (a published name, a wire number, a layout, a
dependency); everything else is two-way and the maintainer decides it.

### P0: red on arrival

1. `cargo clippy -- -D warnings` fails in the staged repository.
   Finding: `src/survey.rs` line 39 writes `VisionFive 2's` without backticks, which `clippy::doc_markdown`
   (a lint the manifest turns on) reports. nife passes it only because the root `clippy.toml` lists
   `VisionFive` under `doc-valid-idents`; the template's scaffold carries no `clippy.toml`, so the
   `check / clippy` job is red on the first push. Verified on stable 1.99.1 and on the pinned nightly.
   Fix: write `` `VisionFive 2` `` (one edit, no new file). The alternative, shipping a
   `clippy.toml`, adds a file every future lint difference will grow.
   Effort: S. Door: two-way.

2. The crate cannot publish under its name.
   Finding: crates.io already has a crate named `abi` (version 0.1.4, 251 downloads, unrelated). The
   repository can be `nifeos/abi` and consumers can pin it by `rev`, but the publication step of the
   runbook (`notes/crate-move.md`, steps 16 to 19) and the README's badge row have no name to use.
   Fix: decision A below: the crate is `moho_abi` in code and as the package name, so the
   manifest and every `use abi::` change together.
   Effort: S. Door: one-way (a published name).

### P1: before the tag

3. The crate names nife and nife programs.
   Finding: `helpers/lint_externalized_crates.py` counts six hits. The dry run's own
   nife-word check fails on `README.md` ("written for the nife kernel"), `src/lib.rs` line 270 (a comment
   about "nife's standard library" generator), `src/rendezvous.rs` line 30 ("makes nife a capability
   system"), and `src/virtio.rs` line 33 to 35. These lines name `entropy`, `net_stack`, `gpu_driver`,
   `keyboard_driver` and `system_initializer` with their ring offsets.
   Fix: say "a capability kernel" and "the drivers that map the region"; the ring offsets belong
   in the drivers that own them, not in the constant that reserves the last eight bytes.
   Effort: S. Door: two-way.

4. Twenty-eight references point into nife's internals, which a stranger cannot follow.
   Finding: after the move strips the `Name:` blocks, the doc comments still cite
   `cap::CAPABILITY_TABLE_SLOTS` (`src/lib.rs` line 95), `capability_table::Rights::ALL`, `::BIND` and
   `::ENUMERATE` (`src/lib.rs` line 210, `src/rights.rs` line 9, 14, 19). They cite `user_mode_runtime::cycle_reading`,
   `::receive_request` and `::reply` (`src/lib.rs` line 226, `src/rendezvous.rs` line 108),
   `std_runtime_contract::SLOTS` (`src/memory_region.rs` line 26). They cite `current_cpu_contract::THREAD_PAGE_SLOTS` (`src/thread_control_block.rs` line 8), `filesystem_contract` (`src/rights.rs` line 9, `src/reboot.rs` line 7) and `crates/byte_sink_contract` (`src/rendezvous.rs` line 10). They also cite `entropy_contract` and `credential_contract`
   (`src/lib.rs` line 384), `inter_process_communication::timer` (`src/timer.rs` line 41), `sched::last_cpus`
   (`src/survey.rs` line 50), `console::configure_from_dtb` (`src/uart.rs` line 6), "AGENTS.md rule 7"
   (`src/lib.rs` line 227), "claim 11" (`src/page_frame.rs` line 6, `src/address_space.rs` line 8), "(19c)"
   (`src/lib.rs` line 184). The programs `pmap` and `ps` appear in `src/memory_region.rs` line 52, `src/address_space.rs` line 28 and `src/survey.rs` line 13. A raw HTML link into the kernel's rustdoc, `../user/struct.Spawn.html` (`src/fault.rs` line 3), does not resolve in the crate's own docs, and rustdoc does not check it. `src/address_space.rs` line 4 cites `frame::MAP`, a module that does not
   exist (it is `page_frame`).
   Fix: restate each as a fact about the boundary ("the kernel asserts its own table size equal to
   this at compile time"; "a runtime wrapper tests `x4`, never `x1`"); delete the HTML link; name
   `page_frame::MAP`. The W^X claim gets its one sentence here instead of a claim number.
   Effort: M. Door: two-way.

5. The virtio contract documents an error that does not exist.
   Finding: `src/virtio.rs` line 19 says `SETUP_QUEUE` answers `BadQueue`/`WrongObject`. `Error` has no
   `BadQueue`; the kernel maps every transport refusal but a DMA escape to `WrongObject`
   (`kernel/src/syscall/object_methods.rs` line 234). A reader who matches on `BadQueue` finds nothing to
   match.
   Fix: "`WrongObject` when `queue` is out of range, the block does not fit the region, or the
   queue was never set up".
   Effort: S. Door: two-way.

6. The crate root understates the register convention.
   Finding: `src/lib.rs` line 6 to 7 say `x0` carries the slot, `x1` the method, `x2` to `x4` the
   arguments and `x0` the result. The runtime's trap moves six words in both directions
   (`x0` to `x5`, `a0` to `a5`). `RECEIVE` writes `x3` and `x4`. A death delivers a sixth word
   (`src/fault.rs` line 26). `CONFIGURE` reads "argument register 6" (`src/thread_control_block.rs` line 9),
   a register the root never admits. The names are aarch64's throughout; the README says "or the
   architecture's equivalent" and the crate never says what that is beyond `START` (`src/thread_control_block.rs` line 29)
   and `LIST` (`src/address_space.rs` line 31). The x86_64 names of words 3 to 5 are not verified here.
   Fix: one table at the crate root: word, aarch64, riscv64, x86_64, meaning on entry, meaning on
   return; every module doc then says "word 3" or keeps `x3` with the table to read it by.
   Effort: S. Door: two-way (documentation of a wire that does not change).

7. Three statements contradict the crate.
   Finding: (a) `src/lib.rs` line 184 says of an address space that "nothing can run in it until TCBs
   arrive", while `thread_control_block::CONFIGURE` binds one and the same paragraph says a thread can take a binding. (b) `src/lib.rs` line 104 to 109 keep `console::WRITE` "so the ABI's history is
   legible", a method of an object no kernel serves, with no user in the tree; the tree's rule is
   that git holds history. (c) `README.md` says "BUGS: None known" while the crate carries five
   `# BUGS` sections (`src/rendezvous.rs` line 20, 46, 71; `src/memory_region.rs` line 56;
   `src/lib.rs` line 247). Two of them are real wire warts (a message word that reads as an error; a death
   read through `RECEIVE_CAPABILITY` loses its `pc`).
   Fix: (a) delete the sentence; (b) keep the `console` module as reserved (ruled, decision B; done); (c) the README's BUGS section lists the five in one line each.
   Effort: S. Door: (b) is decision B; the rest two-way.

8. Dates and rulings stand in doc comments as history.
   Finding: 23 dated phrases remain in `src/` after the move strips the `Name:` blocks, such as
   `src/lib.rs` line 101 ("the size was set on"), `:171` ("confirmed on"), `:307` ("ruled on") and `:357` ("were ratified on"). Others sit at `src/memory_region.rs` line 11 and `:54`, `src/rendezvous.rs` line 82 and `:109`, `src/survey.rs` line 19, `src/notification.rs` line 33, `src/reboot.rs` line 5, `src/object_type.rs` line 29, `src/cycle_counter.rs` line 31, `src/timer.rs` line 39, `src/reply.rs` line 7 and `src/thread_control_block.rs` line 43. There are also four "Index exception (date)" comments in `src/lib.rs`. Section §267 (A comment states the constraint as it is now) and §298 (A document states what is in force now, and its history is in git) put these in commit
   messages. The template's `history phrases` gate reads only markdown today, so nothing refuses
   them. A stranger reads them as noise.
   Fix: state each rule in the present tense; where the sentence is provenance, move it into the
   item's `Name:` block before the move strips it.
   Effort: S. Door: two-way.

9. The README fails the template's prose gates, and has no badge row.
   Finding: `prose caps` reports a 57-word sentence (the "Methods" bullet) and 28 bold spans per 1,000 words against a cap of 4. `passive voice` reports 6 passives in 459 words against 2 per 1,000. `readme_badges` wants the scaffold's row under the title, with the repository's name.
   The section order passes.
   Fix: split the bullet into a table of modules; unbold; rewrite the six sentences; add the row at
   the move. Reading as a stranger, the README also needs the one sentence item 6's table gives
   (which register holds what on each architecture) and the line `cargo test` already has.
   Effort: S. Door: two-way.

10. The manifest's publish comment points out of the repository.
    Finding: `Cargo.toml` line 10 says "the allowlist lint holds the exception (crates/publish-allowlist.txt)",
    a file the new repository will not have. The same finding as milestone 1048 (what an outside maintainer would fix before 0.1.0)'s first item.
    Fix: "`false` until the release that publishes; that release deletes the line".
    Effort: S. Door: two-way.

11. The version gate stays behind.
    Finding: `helpers/format_values_check.py` (`ABI_CRATE = 'crates/abi/'`) is the check that a
    changed or removed public number arrives with an `ABI_VERSION` bump. It reads nife's diff and
    will see nothing once the crate has left. The crate's own tests pin every value, and the
    test's doc claims the list is exhaustive, but nothing counts: 99 `pub const` `u64` items today
    against 99 pinned lines, by hand. The template's `semver` job watches API shape, not values.
    Fix: carry the rule into the crate. A test reads `src/**/*.rs`, counts every `pub const NAME:
    u64` and `pub const NAME: u32`, and fails unless each appears in the pinned lists (the lists
    then cannot drift). The version half becomes a `script/` check in the new repository, or a
    CHANGELOG rule the pull request template states: a pinned line changes only with `ABI_VERSION`.
    Effort: M. Door: two-way.

12. Two function names break rules a stranger reads as inconsistency.
    Finding: `futex::admitted(flags) -> bool` (`src/futex.rs` line 16) is a bare participle. `design/naming.md` ("Functions that answer yes or no") wants `is_admitted`, and the template's
    `question_shape.py` refuses it at ceiling 0. `Error::from_ret` (`src/lib.rs` line 387) abbreviates
    "return"; the standard library's spelling of the same conversion is `TryFrom<i64>`, which also
    gives `?` and `.try_into()` for free. Both names are provisional; `admitted` has one kernel
    caller, `from_ret` six.
    Fix: `is_admitted`; `impl TryFrom<i64> for Error` with the current body, and `from_ret` deleted
    or kept one release as a thin alias (the no-alias rule for old spellings applies before the
    first release, so delete).
    Effort: S. Door: two-way.

13. The names are not ready for the runbook's first step, and no package exists.
    Finding: `helpers/name_release.py --draft abi` lists 148 names: 17 ratified, 24 provisional,
    106 with no `Name:` block at all (every method constant of `address_space`, `memory_region`,
    `notification`, `page_frame`, `timer`, `usage`, `rights` and more). `notes/crate-move.md` step 1 needs the architect's ratification before the move. Also, `notes/crate-reviews/` has no package
    for this crate to start from. This is the largest gap between the crate and the move.
    Fix: a lane writes the package (`script/crate-review-packet abi` plus the naming table, in
    the shape of `notes/crate-reviews/socket-contract.md`), and marks every unmarked public item
    `provisional` so the worklist can see it. The rulings are the architect's, and this review does not list them one by one. The package asks them.
    Effort: M for the package. Door: each ratified name is one-way once published.

### P2: later

The P2 items are in [the appendix](1075-abi-outside-review/later-items.md).

## Decisions for the architect

The architect ruled decision A, and decision B is the maintainer's provisional ruling. The decisions still
open are under "Open decisions" below.

### A. The published name (ruled)

Rule in force, ruled 2026-11-14 (UTC) as §323 ruling 2: the crate is `moho_abi`, in code and as
the package name. Moho names the capability binary interface, the boundary programs target, and
`nife` stays the name of the capability microkernel. The name was free on crates.io when ruled,
where `abi` belongs to another author's crate (0.1.4). Prior art: Redox publishes its system call
numbers as `redox_syscall`, and Hubris keeps a crate named `abi` that it never publishes. Example:
the rename turns `abi::SYSCALL_INVOKE` in `kernel/src/syscall.rs` into `moho_abi::SYSCALL_INVOKE`.

### B. `console::WRITE` stays reserved (ruled)

Rule in force, ruled 2026-11-14 (UTC) by the maintainer as a provisional two-way door; the architect
may revisit it before the first publish. Nobody outside the project uses nife, so the door stays
two-way until the crate publishes (as `moho_abi`) and has outside users.

`console::WRITE` is a method number of a kernel object that no kernel serves and no program invokes.
The number stays in the crate as reserved: the module and its pinned line stay, `ABI_VERSION` does
not change, and the number is never reused for another method. Removing a number that no kernel
serves is not done, and changing the meaning of a number a kernel serves owes the bump. The gate
(`helpers/format_values_check.py`) refuses a changed or removed public number of the crate without a
changed `ABI_VERSION`; it is unchanged. A test (`reserved_numbers_are_never_reused`) holds the
reservation, and the crate's docs state the rule beside `ABI_VERSION` and in the README.

## Done when

The checks are in [the appendix](1075-abi-outside-review/done-when.md).

## Finding status

- Item 1, clippy red on `VisionFive 2`: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 2 and decision A, the published name: Ruled (§323 ruling 2, 2026-11-14 UTC). Not built:
  `crates/abi/Cargo.toml` still names the package `abi`, and every `use abi::` waits for the
  rename to `moho_abi`, which §323 lists as a follow-on for a rename lane after its plan.
- Item 3, the crate names nife and nife programs: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 4, twenty-eight references into nife: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 5, the virtio error that does not exist: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 6, the register convention: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 7, three self-contradictions: Done, (a) and (c) in
  [#3565](https://github.com/nifeos/nife/pull/3565), (b) in
  [#3596](https://github.com/nifeos/nife/pull/3596).
- Item 8, dates and rulings as history: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
  The six `// Index exception (<date>)` comments in `src/lib.rs` keep their dates, which
  `helpers/parent_index.py` requires.
- Item 9, README prose gates and badge row: Open.
  [#3565](https://github.com/nifeos/nife/pull/3565) fixed the prose; the badge row arrives with the
  move.
- Item 10, the manifest's publish comment: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 11, the version gate: Open.
  [#3565](https://github.com/nifeos/nife/pull/3565) added the counting test
  `every_public_number_is_pinned`; the rule that a pinned number changes only
  with `ABI_VERSION` moves into the new repository with the crate.
- Item 12, `futex::admitted` and `Error::from_ret`: Open. Two-way until the first publish, so the
  maintainer rules it provisionally; the recommendation is `is_admitted` and `impl TryFrom<i64> for
  Error` with `from_ret` deleted, and calef ratifies the names in the crate's review package.
- Item 13, names not ready and no package: Open. A lane writes the review package and marks every
  unmarked public item provisional; the ratification of its names follows in that package.
- Item 14, `Error` lacks `Display`, `Error` and `Hash`: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 15, `CapabilitySlot` has no user: Open. Two-way until the first publish, so the maintainer
  rules it provisionally (use the alias in `invoke`'s signature or delete it), and the review
  package carries the result.
- Item 16, `RegisterShape::new`'s parameters: Open. Two-way until the first publish, so the
  maintainer rules it provisionally (`register_shift` and `access_width_bytes` with the binding's
  names as `doc(alias)`), and the review package carries the result.
- Item 17, `CycleMeaning` is not a syscall contract: Open. Two-way until the first publish, so the
  maintainer rules where it lives provisionally, and the review package carries the result.
- Item 18, doc nits: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 19, `Name:` blocks quote the decider: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 20, six survivors with no record: Open. One `mutants-exempts` line per survivor, or a test,
  after the new repository's first weekly run.
- Item 21, no machine checks: Open. The bounded `counter_ticks_for` harness attempt and the proofs
  job's behavior on zero harnesses, after the new repository's first runs.
- Decision B, `console::WRITE` stays reserved: Done, [#3596](https://github.com/nifeos/nife/pull/3596).
- The move: Open. It waits for the review package and its clearance.
- Publishing: Open. No first publish happens until the last outside-review milestone closes
  (ruled 2026-11-18 UTC).

## Open decisions

### Clearing the review package

Background. Before a crate leaves this tree, calef reads its code and its review package and
clears it; the clearing is also the approval to create its repository. This crate is the binary
interface every program and the kernel agree on: system call numbers, method numbers, rights bits
and error codes. It has no review package yet. Item 13 asks a lane to write one, in the shape of
the socket contract's, listing all 148 public names with their state. This decision is the
clearing of that package once it exists, under the crate's ruled name `moho_abi`.

Recommendation: A.

- A. Clear the review once the package lands and a lane builds the `moho_abi` rename, which approves
  creating the repository and moving the crate.
- B. Clear the review before the rename, and let the move carry it.
- C. Send the package back, naming what to change first.

Door: one-way. A public repository is an outward step, and every number in this crate becomes what
a stranger's program compiles against.

Prior art. Linux keeps its system call numbers in `arch/*/include/uapi`, reviewed on the kernel's
own mailing lists, and never renumbers one once a release carries it. Redox publishes its numbers
as the `redox_syscall` crate (recalled, not read here). nife differs in that the numbers leave the
kernel's tree as a crate of their own, with a counting test that refuses a changed number without
a changed `ABI_VERSION`. Example: `block_roster_contract` had its clearance on 2026-11-09 (UTC) and
now lives in a repository of its own, which nife pins at the commit of a pre-release tag.

Blocks: the move, and through it the tag and the first publish.

## Follow-on

- **Done.** Decision B: `console::WRITE` stays as a reserved operation (#3596).
- **Outstanding.** Decision A's rename to `moho_abi`, ruled by §323 (the binary interface and the
  names of the layers) and not yet built; checked against `crates/abi/Cargo.toml`.
- **Outstanding.** The package of item 13 with the provisional rulings of items 12 and 15 to 17,
  then items 9, 11, 20 and 21 at the move. The finding status above says what each needs.

## BUGS

- Item 6's x86_64 column for the fourth and fifth words comes from reading the runtime's aarch64 and riscv64 traps and the x86_64 `START` doc. The lane that writes the table reads the
  x86_64 trap and the kernel's entry before it writes a register name.
- Item 20 names three survivors of six; the other three are what the first weekly run reports, and
  this block does not guess them.
- Item 21 does not know whether the proofs job is green with no harness; the lane that stages the
  move finds out on the first push.
- The coverage figure was not measured; every function in the crate has a test that calls it, so the 85 floor should pass, unverified.

## Index row

An outside review of `abi` before its move found these problems. One lint is red on arrival. Another crate holds the crates.io name. Six nife references and twenty-eight internal citations need a rewrite. The docs name an error the kernel never answers. The root states the register convention too narrowly. A version gate stays behind in nife. Two function names break the rules. 130 names lack a ratified block, and no review package exists. The architect ruled the published name `moho_abi`. A dead method's number stays reserved.
