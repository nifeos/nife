# Adding a user program

Task-oriented: what to do, in order. A program is declared in one place, and in four if the shell
can spawn it (five if it answers in a register). Only the first two of those are places you have to
remember: the build or a host test sends you to each of the rest.

A program is a `[[bin]]` in one of two packages, running at EL0, linked against `user_mode_runtime`.

## Which package: `components/` or `fixtures/`?

Decide this first, because it picks the directory and the `Cargo.toml` every later step edits.
Milestone 175 split `user/` on 2026-09-13, and the question it answers is *would a distribution ship
this because somebody wants its function?*

- `components/` if yes: a service, a driver, the progenitor and supervision spine, or a tool a person
  invokes at the prompt. `net_stack`, `gpu_driver`, `progenitor`, `wc`, `rm`, `date`.
- `fixtures/` if no: the program exists to exercise or measure the system. Test clients,
  attackers that share the honest path, stand-in servers, workloads, benchmarks. `chatty`,
  `kernel_test_subject`, `coremark`, `soaker`, `interrupt_ignorer`.

"Who calls it" is not the test, and getting that wrong is the easy mistake: nearly everything
here is reached only from a kernel test, `disk_partitioner` and `timetable` included, because this
system has one user. What separates them is what the program *is*.

## One program does one thing

A behavior is a program, not a role. The tempting shortcut, when a fixture already exists that
is nearly what you want, is to add an arm to its `match` on `x0` and select the new behavior with
a number. Do not. calef, 2026-09-14: *"Part of the beauty of Unix that I think we want to retain is
small programs with specific functions"*, and, on the binary that had accumulated thirty-one arms,
*"31 role binary is not the right shape."*

The cost is not aesthetic. A role is dispatched on a word the kernel puts in `x0`, so it is a value
the kernel's wiring and the program agree on that nothing checks. Its name lives in a `const` two
files apart rather than in the archive. The binary links everything every role needs, so a fixture
spawned to prove one syscall drags in an ELF parser. And the program's name has to describe all of
them at once, which it stops doing at about the third.

Milestones 291 and 405 are the worked example. One fixture, `hello`, reached thirty-one roles one
convenient arm at a time over two months. Seven of them were an exact duplicate of
`components/src/block_driver.rs`, kept alive only because one archive table did not pack it; the
other twenty-four became programs. See [291](../design/roadmap/0291-one-program-one-job.md) and
[405](../design/roadmap/0405-nine-init-roles-and-the-entry-the-kernel-picks.md).

The exception is allowed and has to say so (AGENTS.md's ladder, rung four's rule). If two
behaviors genuinely are one program, write down in the module doc why, where a reader meets it.

## The steps

### 1. The source

`components/src/<name>.rs` or `fixtures/src/<name>.rs`, `snake_case` (DECISIONS §39, and the
convention table in [naming.md](../design/naming.md)). `no_std`, against `user_mode_runtime`.

### 2. A provenance block in its module doc

```rust
//! Name: unrecorded. Introduced 2026-08-14 for <what it does>.
```

`script/lint` fails without one, via `script/names --check`. Three states: `ratified` (calef
ruled, with the date and what was refused), `recorded` (the tree argues the name somewhere, with a
citation), `unrecorded` (nothing outside this block says why). The gate checks presence, never
`ratified`, so an unratified name never blocks a build and `unrecorded` is a truthful answer.

The name is an architect's (AGENTS.md, "calef names the crates, the programs, and the shared
modules"). Ship a provisional one, say so in your report, expect it to change.

Write `provisional` when you expect the name to change, which is AGENTS.md's word and, since
§89 (2026-08-16), the gate's too. Four states:

```
Name: ratified 2026-08-04 (calef, milestone 63). Refused `x` (why).
Name: recorded (milestone 46). <what the tree already argues, and where>
Name: provisional. <what you called it and why you expect it to change>
Name: unrecorded. <what the history does and does not say>
```

`provisional` is a claim about intent (you expect this to change); the other three are claims
about the record. A settled name can be `unrecorded` (nobody wrote down why `hello` is called
`hello`, and nobody needs to), so the two are not the same word for the same thing.

`script/names --provisional` lists them and they sort first in `--unratified`, because a name its
own author called wrong is the shortest conversation an architect can have. This page told newcomers
the opposite until §89 (provisional): run 2 of the stranger
test wrote the word AGENTS.md asked for and got a red gate, raising the decision.

### 3. A `[[bin]]` block in that package's `Cargo.toml`

```toml
[[bin]]
name = "your_program"
path = "src/your_program.rs"
test = false
bench = false
```

`test` and `bench` off are mandatory, not tidiness: the default libtest harness needs
`extern crate test`, which does not exist for a bare-metal target.

That block is the whole of step 3, and of packing. `xtask` reads the `[[bin]]` blocks of both
packages (`declared_programs()`) and packs every one into all three archives, aarch64, riscv64 and
`x86_64`, under its own name. There is no packing table and no per-architecture filter;
[why-it-works-this-way.md](adding-a-program/why-it-works-this-way.md) says why. You do not touch
the measurement table either: `xtask` hashes every entry of the archive it just packed and writes the table the progenitor measures
against from that (`write_measure_manifest`).

The reader is strict on purpose. A key it does not know inside a `[[bin]]` block
(`required-features`, say) stops the pack with the key named, rather than packing an archive with
that program silently missing.

A new program or crate also needs an entry in a `packages/*.package.toml` file, provisional like a
name, or `script/lint` fails ([package boundaries](package-boundaries.md)).

### 4. Keep the name under 32 bytes

`nifefs` caps `NAME_LEN` at 32, raised from 24 so `os_primitives_benchmarker` would fit. Raising
it again costs directory entries per block, so do not let a name spend it.

There is also a ceiling on how many programs an archive holds, `nifefs::MAX_FILES`, 178 at
`DIR_BLOCKS = 14`. The kernel suite's x86_64 archive holds 128 where `cargo xtask basalt-fetch`
ran, so the headroom is about fifty. It fails loudly: `write_image`
returns `TooManyFiles` and each `initial_ramdisk_*` function prints the error, the file count and the size.

### 5. If the shell should be able to spawn it

Three edits, and after the first the machine names each of the others.

1. A row in `programs!` in `crates/grant_plan/src/program.rs`, with its doc comment:

   ```rust
   /// Triples its integer argument (milestone N, `components/src/triple.rs`).
   Triple { id: 13, name: "triple" },
   ```

   `name` is the `[[bin]]` name. `id` is the stable wire id: the shell sends it and the
   progenitor decodes it, so it is a thing two programs agree on and changing it later is a flag
   day. Take the next unused number; never reuse a removed program's id (the build refuses a
   duplicate, and `the_wire_ids_already_shipped_never_move` refuses reuse of any id shipped before
   milestone 150). The enum, `name()`, `id()`, `from_id()`, `from_name()`, `Program::ALL` and
   `PROGRAM_COUNT` are all generated from the row.
2. Its `manifest()` arm, which the compiler asks for (`E0004` in `grant_plan`). This carries all
   of the actual meaning: see "What you declare" below.
3. A row in the swish-check script: in the topic file under `xtask/src/swish_check/script/` whose
   subject the line belongs to (each file's module doc says what it holds), a `line(...)` or
   `keys(...)` element of that file's `LINES`. The topic slices concatenate, in table order, into
   `SWISH_CHECK_SCRIPT` in `script.rs`. The host test
   `every_spawnable_program_has_a_swish_check_line` fails until the row exists, and names the one
   exception it allows (a program a transcript cannot drive, listed with the reason).

And a fourth only if your manifest says `output: OutputDeclaration::Words`, meaning the answer is a
number in a register rather than a stream: an arm in `write_outcome` in `crates/swish/src/sentences.rs`
that renders it. `every_program_that_answers_in_words_renders_its_answer` fails without one. A
byte-stream program needs nothing there.

`xtask` refuses to pack an archive when a `programs!` row names a program no `[[bin]]` builds, so
getting the name wrong in step 1 is a build failure rather than a program that cannot be spawned.

## What you declare: the manifest

The manifest is the program's endowment, and the shell checks it at the prompt, before a child
exists. A mismatch is a legible refusal on the line you typed rather than a hang deep inside a
program that assumed a slot was full. See [grant-expression.md](grant-expression.md) and
[program-manifest.md](program-manifest.md).

**The manifest declares the direction; the command line designates the file.** Whether a program
writes is a fixed, publishable property of it. Which file it touches is the caller's business. So
`wc report.txt` reads and `tee report.txt` writes, and nobody types a mode.

**A manifest is as much about refusal as need.** `date`'s row is `Forbidden` throughout, so a memory
grant aimed at a clock reader stops at the prompt.

## Removing a program

Delete its source file and its `[[bin]]` block. It leaves all three archives at once.

If the shell could spawn it, also delete its `programs!` row. The compiler then points at its
`manifest()` arm and any `write_outcome` arm that named it; delete those, and its
`SWISH_CHECK_SCRIPT` lines, which nothing on the host flags (see `BUGS`) and which
`script/swish-check` answers with `no such program`. **Leave its id unused.** The table's holes are expected: `PROGRAM_COUNT` is
one past the highest id, not the number of programs.

**What catches a removal you did not mean to make**: `xtask`'s host test
`every_program_the_tree_loads_by_name_is_declared` fails when the kernel or the progenitor still
looks the program up by a string literal (`program("name")`, `.read("name")`), naming the file that
does. Before milestone 150 such a test would `skip!()` with "no such program in this archive" and
nobody would hear about it.

## Check your work

```sh
script/lint          # the name block, the conventions, and every host test named above
script/swish-check   # if the shell spawns it: both ISAs, and much faster than the suite
script/test          # all three architectures
```

`cargo xtask build` packs the aarch64 archive only, whatever the name suggests; `initial_ramdisk_riscv()` and
`initial_ramdisk_x86()` are called by `test` and `swish-check`. That no longer hides a packing mistake,
because all three archives pack one list, but it is still not a check of the other two builds.

**Run swish-check once with a deliberately wrong expectation.** A green harness only proves the
harness did not complain; a red one proves your program was really loaded from the archive,
measured, granted its endpoint and run at EL0. Verbatim from a run of this page on 2026-08-18:

```
$ triple 21
  a process at EL0 computed 21*3 = 63
--- swish-check (aarch64) FAILED ---
  `triple 21` answered "a process at EL0 computed 21*3 = 63", wanted "21*3 = 64"
```

## Why it works this way

Milestone 150 (2026-09-19) made the `[[bin]]` blocks the archive list and the shell's program table
one `macro_rules!` declaration. It replaced a pinned program count with three host tests, and
turned `write_outcome`'s exhaustive match into a wildcard. The block is
[design/roadmap/0150-program-declaration-data.md](../design/roadmap/0150-program-declaration-data.md),
and [DECISIONS §158 (a program is declared once)](../design/decisions/0158-a-program-is-declared-once.md)
is the record. What was considered and refused for each choice is in
[why-it-works-this-way.md](adding-a-program/why-it-works-this-way.md).

## BUGS

- The `[[bin]]` reader knows four keys (`name`, `path`, `test`, `bench`), because those are all
  this tree writes. Anything else in a `[[bin]]` block stops every pack with the key named. That is
  the intended failure, not an accident, but it means the first program to want
  `required-features` has to teach `bin_names` in `xtask` what the key means for the archives.
- The removal gate is textual. `every_program_the_tree_loads_by_name_is_declared` reads
  `program("...")` and `.read("...")` literals in `kernel/src` and
  `crates/system_initializer/src`. A name built at runtime, or looked up through some other
  spelling, is invisible to it, and a kernel test that `skip!()`s on a missing program still skips
  quietly for such a name. It counts what it matched and fails below fifty, so it cannot go blind
  without saying so.
- Nothing on the host checks `SWISH_CHECK_SCRIPT` in the other direction. A line typing a
  program that no longer exists is found by `script/swish-check`, at the cost of a boot. And the
  forward check is a whole-word match on the program's name, which proves a line mentions it, not
  that the line ran it.
- The wire-id pin covers the thirteen ids shipped before 2026-09-19. A program added after that
  and later removed leaves an id that only the duplicate check protects, and only while nothing else
  claims it. The declaration's written id makes a renumbering visible in review; reuse of a
  post-pin id is not gated. Appending a row to `the_wire_ids_already_shipped_never_move` is how to
  pin a later one.
- Every archive packs every program, including ones that cannot run there, which is the rule
  rather than an oversight (see
  [why-it-works-this-way.md](adding-a-program/why-it-works-this-way.md)). So the aarch64 archive
  carries `serial_driver`, `jh7110_entropy` and `pmap`, and the other two carry `pmap`. `pmap` is
  packed and spawned by nothing (see `crates/pmap`'s `BUGS`); it costs a directory slot, which is what the
  rule prices it at.
- Whether a program may take an argument and an input together is open, and it is an architect's
  call:
  [a-program-that-takes-an-argument-and-an-input.md](../design/roadmap/0498-a-program-that-takes-an-argument-and-an-input.md).
  The tree allows it and nothing uses it. The `crates/swish` sweep that used to go red on it (the
  "eighth edit site" milestone 117's fifth stranger found) now types every operand a manifest asks
  for, so the combination needs no edit outside its own declaration.
- This page is prose and the code can move without it. The step that rots first is the manifest
  field list, which is why it is not repeated here: [program-manifest.md](program-manifest.md) has it,
  and the struct in `crates/grant_plan/src/manifest.rs` is the authority over both.
- **Written from having done it, and wrong most times it was walked.** Every walk before milestone
  150 found the page wrong, and the reason was structural rather than careless: the fact it described
  lived in seven hand-maintained places it did not control, so it went stale inside two days, twice.

  | walk | program | wrong in |
  |---|---|---|
  | 2026-08-16 (run 2) | `doubler` | the aarch64 tier, the riscv `--bin` list, two of the six `grant_plan` edits, the `provisional` spelling the gate rejects |
  | 2026-08-18 (run 3) | `triangle` | the aarch64 tier again (milestone 130 had deleted both shapes it described), and `manifest()` missing from the `grant_plan` list |
  | 2026-08-18 (a lane) | a scratch binary, added and removed | `cargo xtask build` claimed to pack both archives and packs one, the `SWISH_CHECK_SCRIPT` example did not compile, and nothing said which of the seven `grant_plan` edits the machine catches |
  | 2026-08-18 (run 4) | `tally`, added and removed | **nothing.** The first walk of four to find no defect |
  | 2026-08-18 (run 5) | `nth`, kept | an **eighth** edit site: a manifest that requires an argument *and* an input failed a `crates/swish` sweep the walker had no reason to open |
  | 2026-09-19 (milestone 150 (adding a program should not need eight hand-maintained lists)) | `triple`, added and removed | **the count, measured on the new tree.** A plain program was three hand edits and is one. A spawnable program that answers in a register was twelve edits across five files, two of them silent, and is six across four (the `[[bin]]` block, the `programs!` row, then the `manifest()` arm, the `write_outcome` arm, the `SWISH_CHECK_SCRIPT` line and its array length, each demanded by the compiler or a host test). It packed into all three archives with no further edit and answered at both prompts. Removal left a byte-identical tree; the stale `SWISH_CHECK_SCRIPT` line was the one edit nothing on the host named |

  **One walk-through is not a guarantee**, and the next person to add a program should treat a
  surprise here as this page's bug rather than their own.

  **This table is also a leak, and it is worth knowing about before adding to it.** Milestone
  117's fourth stranger read these rows within half an hour and knew from them that it was at
  least the fourth person walking this page under measurement, which changed how it wrote. The
  rows stay, because deleting them would fabricate a tree and because the page's value is that
  it says how often it has been wrong. See notes/stranger-test.md's `BUGS`.
