---
status: BUILT
raised: 2026-08-22
built: 2026-09-19
---
# 150. Adding a program should not need eight hand-maintained lists

Built 2026-09-19 (PR #968). A program is declared once: its `[[bin]]` block, plus one
`programs!` row in `crates/grant_plan` if the shell can spawn it. Everything else is generated from
those or checked against them. The after count is measured, by adding and removing a scratch
program (`triple`) and booting it at both prompts; the before count is read off the base tree
(8bac3637) and the 2026-08-18 walks rather than re-walked. A plain program went from three hand
edits to one. A spawnable program that answers in a register went from twelve edits across five
files (two of them silent) to six across four, of which only the first two are unprompted.

Minted provisionally on 2026-08-22 by the handoffs lane of milestone 117 (the stranger test); the integrator
should confirm the number at merge. 147, 148 and 149 were already claimed by other lanes' pull
requests (147/148 open, 149 merged into this lane's own base commit) as this was written.

**Amended 2026-08-27** (maintainer/initrd-build-consistency): site 3 below, `initrd_riscv()`'s
`--bin` argument list, is deleted. It predated every program in `user/` compiling for riscv64 and
bought nothing once that became true; it also fell out of step with sites 1/2/4 twice in one night
(`audit_sink`, milestone 49) before it was removed. The count this milestone was minted against is
now seven hand-maintained places, not eight (`mkinitrd()` is also renamed `initrd_aarch64()`, and
riscv64/x86_64 both build the whole `user` package unfiltered, the way aarch64 always did). This
does not close the milestone: sites 1, 2 (now `initrd_aarch64()`), 4 (`initrd_riscv()`'s/`initrd_x86()`'s
shared `entries` table), 5, 6, 7 and 8 are all still hand-maintained, and the design question in the
next section is unchanged. See notes/adding-a-program.md for the corrected count and PR #564.

## What was built

The archives are generated from `Cargo.toml`. `xtask`'s `declared_programs()` reads the
`[[bin]]` blocks of `components/` and `fixtures/` and all three packers (`initrd_aarch64`,
`initrd_riscv`, `initrd_x86`) pack exactly that list. Both hand-written tables are gone. They had
drifted when deleted: `serial_driver` and `jh7110_entropy` were absent from aarch64's and `pmap` from
both, none of it decided, so aarch64's archive gained three programs and the others one. The
reader is strict (an unknown key in a `[[bin]]` block stops the pack), because a lenient one would
silently drop a program. This retires sites 2 and 4 of the list below, and site 3 was already gone.

`Prog` is one `macro_rules!` declaration (`programs!`, provisional), a row of
`Variant { id: N, name: "..." }` per program, generating the enum, `name()`, `id()`, `from_id()`,
`from_name()`, a new `Prog::ALL`, and `PROG_COUNT`. `manifest()` stays a hand-written match because
the compiler already demands its arm. Ids are written, never positional; `PROG_COUNT` is one past the
highest id, so a removed program leaves a hole rather than renumbering; duplicate ids or names fail
the build in a `const` assertion. A host test written before the refactor pins the thirteen shipped
`(name, id)` pairs and refuses both renumbering and reuse. Site 5 goes from seven edits to two.

`swish`'s render match is a wildcard plus a host test that every `OutputSpec::Words` program
renders its answer. Eleven of thirteen arms were empty, so the compile error it used to raise asked
most authors for a keystroke. Site 6 now applies only to the programs it means something for.

The gate on count is three relationship checks rather than a number (item 4 below). A pinned
total was refused because it would be a hand-maintained number failing on every legitimate
addition. Instead: packing refuses a `programs!` row with no `[[bin]]`. A host test refuses a
program the kernel or the progenitor still loads by string literal after its block is deleted (the
removal that used to become a test that `skip!()`s forever). A host test requires every
spawnable program to have a `SWISH_CHECK_SCRIPT` line or a stated reason. That last one found
`memory_grant_depleter` had none, and it gained one. Site 7 is now enforced rather than remembered.

The eighth edit site is closed: `the_arg_line_follows_the_manifest_for_every_program` types
every operand a manifest asks for, so an argument-plus-input program needs no edit in `crates/swish`.
Whether such a program is *wanted* (item 3) is an architect's call and is written up as
[a-program-that-takes-an-argument-and-an-input.md](0498-a-program-that-takes-an-argument-and-an-input.md),
recommending the status quo. The neighboring file-plus-input refusal moved from a comment into a
host test.

**notes/adding-a-program.md shrank** to what survives (site 8), gained the removal section run 4
asked for (item 5), and carries the mechanism reasoning with what lost under "Why it works this
way", for the integrator to mint a `design/decisions/` section from.

### BUGS and honest caveats

- The removal gate is textual: it reads `program("...")` and `.read("...")` literals in
  `kernel/src` and `crates/system_initializer/src`. A name built at runtime is invisible to it. It
  counts what it matched and fails below fifty, so it cannot pass on nothing.
- **A stale `SWISH_CHECK_SCRIPT` line is not caught on the host.** Removing a spawnable program
  leaves its transcript line, which `script/swish-check` then fails at the cost of a boot. The walk
  measured this: it was the one edit nothing named. Recorded in notes/adding-a-program.md's `BUGS`.
- The wire-id pin covers ids shipped before 2026-09-19. Reuse of a later program's id after that
  program is removed is not gated; the written id makes a renumbering visible in review only.
- The `[[bin]]` reader knows four keys. The first program to need `required-features` must teach
  it, by design.
- `write_outcome`'s wildcard is rung two where rung one stood. It is the one place this milestone
  went down the ladder, deliberately, and the test that replaced it is recorded beside the match.
- `cargo xtask build` still packs only aarch64. That no longer hides a packing mistake, since
  all three archives pack one list, but it is not a build of the other two.

### Provisional names

`programs!`, `Prog::ALL`, `declared_programs`, `declared_program_blobs`, `bin_names`,
`check_declared_programs`, `PROGRAM_PACKAGES`, and the test names added in `grant_plan`, `swish` and
`xtask`. `PROG_COUNT` keeps its name with a changed meaning (one past the highest id, not a count),
which a reader of the name alone would get wrong once an id is retired; renaming it is an
architect's call.

## What this is

**Nominated by three successive strangers.** Milestone 117's stranger-test runs 3, 4 and 5 each
independently walked `notes/adding-a-program.md` by adding and removing a scratch program. Each
one named the same defect as the highest-value thing a newcomer could fix. Adding a program to this
tree means editing the same fact into eight hand-maintained places, by hand, with no gate that
catches every way to get one of them wrong.

The eight, as `notes/adding-a-program.md` currently enumerates them:

1. the `[[bin]]` block in `user/Cargo.toml`;
2. `mkinitrd()`'s `entries` table (aarch64 archive, `(archive_name, bin_name)` pairs);
3. `initrd_riscv()`'s `--bin` argument list (what cargo actually builds for riscv64);
4. `initrd_riscv()`'s own `entries` table (what the archive actually packs);
5. the `Prog` variant in `crates/grant_plan/src/lib.rs`, itself seven edits per program
   (`PROG_COUNT`, `from_id()`, `from_name()`, `name()`, `id()`, `manifest()`, and the variant);
6. the exhaustive `match` arm in `crates/swish/src/lib.rs` that renders the shell's answer;
7. `SWISH_CHECK_SCRIPT` in `xtask/src/main.rs`, if the shell spawns the program;
8. `notes/adding-a-program.md` itself, which describes the other seven and has gone stale twice in
   the four days between runs 2 and 3 alone.

Run 5 added an eighth *edit site within site 5*. A program that both requires an argument and takes
an input fails `the_arg_line_follows_the_manifest_for_every_program` in `crates/swish`, a test in a
crate the person adding the program has no reason to open. That host test hard-codes
`Holdings::default()` and a fixed argument line rather than reading the rest of the manifest it
claims to check generally.

## Why this is a design question, not a documentation fix

CLAUDE.md's ladder ranks "make the wrong state unrepresentable" above "a gate that fails loudly"
above "a written record at the thing itself" above "a note." Right now the eight-list problem is
answered entirely at rung four (the note you are reading a summary of), and the note itself is the
evidence that rung four cannot hold this fact. It has been rewritten by four successive strangers,
correctly, and gone stale again within days each time, because the fact it describes lives in seven
other files it does not control.

**What the compiler already catches, and what it does not**, measured rather than assumed (run 5,
2026-08-18, by widening `PROG_COUNT` last and watching what broke):

| edit skipped | consequence |
|---|---|
| `name()`, `id()`, `manifest()` on the `Prog` variant | compile error, `E0004`, in `grant_plan` itself |
| the `swish` render arm | compile error, `E0004`, in a crate the author did not otherwise touch |
| `from_id()` or `from_name()` | a host test fails, **but only if `PROG_COUNT` was also widened** |
| `PROG_COUNT` | **nothing.** The tree compiles, every host test passes, and the program cannot be spawned from the prompt until somebody notices by hand |
| the riscv `--bin` list (site 3) without the riscv `entries` table (site 4), or vice versa | either a build failure naming a missing ELF, or a program present in `mkinitrd()` and silently absent from the riscv archive, uncaught by `cargo xtask build` or `script/lint`, first caught (if at all) by `script/swish-check` or `script/test`, both of which cost an emulated boot |

`PROG_COUNT` is the keystone and forgetting it hides the other two. The sweep in
`prog_id_round_trips` only walks up to that constant, so a variant added past it is invisible to the
one test that would have caught the missing `from_id`/`from_name` arms. **And nothing counts
programs at all**: the kernel test suite reports the identical total, 1312, both before and after a
program is added or removed. A program's presence in this tree is proven only by a transcript line
somebody remembered to type into `SWISH_CHECK_SCRIPT`.

## What this milestone would decide

**The shape run 3 nominated and runs 4 and 5 did not dispute**: a `Prog` variant should carry its
archive name and its manifest as data. So that both initrd tables (sites 2 and 4), the `--bin` list
(site 3), and as much of the `grant_plan` bookkeeping (site 5) as possible are *generated* from the
one place a program is declared. None of it is hand-copied into four more. Concretely, this milestone
would need to answer:

1. **Where the single declaration lives.** A `const` table in `grant_plan` that both `xtask` (for
   the initrd lists) and the shell (for dispatch) can read is the shape the current seven-edit list
   already gestures at with `Prog`. Whether `xtask` can depend on `grant_plan` without pulling
   `no_std` machinery into a host binary is the first thing to check, not assume.
2. Whether `PROG_COUNT` can be replaced by something the compiler derives, rather than a
   number a human widens. Rust has no built-in way to count an enum's variants without a derive
   macro, and this tree has deliberately not taken one (DECISIONS §46: write it if it's on the
   verification path). Whether a hand-rolled `const fn` count, a macro this tree writes itself, or
   accepting the manual count with a stronger gate is the right trade is exactly the kind of
   question that belongs in `design/decisions/`, not assumed by a lane.
3. Whether the argument-plus-input manifest gap (run 5's eighth edit site) is closed by the same
   mechanism or needs its own decision. `notes/adding-a-program.md`'s `BUGS` section already
   records this as open: whether that combination is wanted at all, or should be refused the way
   file-plus-input already is refused with a comment at `plan_against_with`. This milestone's
   generalization would likely force that question rather than let it stay implicit.
4. What a gate on program *count* would look like, since nothing today notices a program was
   added or removed. A `PROG_COUNT`-vs-archive-entry-count assertion in a host test is the cheapest
   candidate and should be weighed against the cost of computing "how many programs are actually in
   this archive" outside a QEMU boot.
5. **Removal**, which run 4 pointed out is the same eight places in reverse and has no page at all
   today. Whatever generation mechanism this milestone builds should make removal the deletion of
   one declaration rather than a checklist nobody has written down.

## What this does NOT include

- Rewriting `notes/adding-a-program.md` a fifth time as prose. That is the move this milestone
  exists to stop making. If any hand-maintained step survives this milestone, it gets documented,
  but the target is fewer steps, not a better description of the current eight.
- Deciding the generation mechanism up front. The three candidate shapes above are starting
  points for the design fork, not a recommendation; this block scopes the question.
- **Touching the riscv/aarch64 archive-content asymmetry** (that `mkinitrd()` packs `hello` as
  `init` and `initrd_riscv()` packs `builder`). That is a separate, already-recorded finding
  (`notes/adding-a-program.md`'s `BUGS`) about which binary boots as `init` on each ISA, not about
  how a program's declaration is spread across files.

## Prior art in this tree

- `xtask`'s measurement manifest is already generated rather than hand-maintained. It hashes every
  entry of the archive it just packed and writes the manifest from that (`write_measure_manifest`),
  so a program's presence there follows the archive by construction with no eighth place to edit.
  That is the shape sites 2 through 4 (and as much of 5 as practical) should end up in.
- The boot tour's `println!("... skipped (no 'X' program in the initrd)")` pattern already treats
  "this program may or may not be present" as ordinary data to check against, rather than an
  assumption baked into a hand-written list; the same posture, one level up, is what a generated
  `Prog` table would give the initrd builders.

## Where the finding is recorded today

`notes/adding-a-program.md`'s `BUGS` section carries the full history (runs 3, 4 and 5, each citing
the file and line the previous stranger read). It should link to this milestone once the integrator
confirms its number, replacing the current circular pointer at "the tracked home for the mechanism is
milestone 117's handoff."

## Follow-on

- **Milestone 498.** milestone 498 (whether a program may take an argument), `design/roadmap/0498-a-program-that-takes-an-argument-and-an-input.md`: item 3,
  whether a manifest may declare an argument and an input together. The mechanical half is done
  (the `crates/swish` sweep supplies both); whether the shape is wanted is an architect's call, and
  the proposal recommends the status quo.
- **Decision.** The mechanism (the `[[bin]]` block as the archive list, `programs!` for `Prog`, and
  three relationship checks in place of a pinned count) is reasoned with what lost in
  `notes/adding-a-program/why-it-works-this-way.md`. The reasoning moved there from
  `notes/adding-a-program.md`'s "Why it works this way" on 2026-10-11, for the integrator to mint a
  `design/decisions/` section at merge. No section exists yet; this lane does not mint one.
  Minted by the maintainer on 2026-09-19 as
  [DECISIONS §158](../decisions/0158-a-program-is-declared-once.md).
- **Recorded.** The stale `SWISH_CHECK_SCRIPT` line a removal leaves, the textual removal gate, the
  wire-id pin's cut-off date, and the four-key `[[bin]]` reader, in `notes/adding-a-program.md`'s
  `BUGS`.
- **Recorded.** `PROGRAM_COUNT` means one past the highest wire id, not a count of programs, stated
  in its doc comment in `crates/program_table/src/program/catalog.rs`. A rename is a naming
  decision and is an architect's.
- **Done.** `memory_grant_depleter` had no `SWISH_CHECK_SCRIPT` line; the new coverage test found
  it and it has one.

## Index row

Built 2026-09-19: a program is its `[[bin]]` block (packed into all three archives from `Cargo.toml`)
plus, if the shell spawns it, one `programs!` row generating `Prog`'s bookkeeping; host tests catch
the rest, including a removal something still loads by name. Minted provisionally by milestone
117's handoffs lane on 2026-08-22, nominated independently by
three successive stranger-test runs (3, 4, 5), when a program's declaration was spread across
eight hand-maintained places and the compiler silently accepted a variant missing its `PROG_COUNT`
bump.
