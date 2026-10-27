# Proving things about `user/`

Milestone 197. The companion to [kernel-proofs.md](kernel-proofs.md), which did the same thing one
privilege level down; both are downstream of [verification.md](verification.md), which is about
proving the pure crates. This one is about the 68 EL0 programs the prover could not compile until
2026-08-31. It covers the stubs you take on when you point it at them, and the shapes of property
that turned out to be out of reach. The measurements behind two sections are in
[`user-proofs/`](user-proofs/README.md).

*Name provisional: notes are an interface and their names are an architect's call (AGENTS.md).
`user-proofs` says what the file is about and matches `kernel-proofs.md`'s neighborhood; expect it
to change.*

## Why this note exists

`script/verify`'s header has always been honest that `cargo kani -p <crate>` never compiles the
kernel, the user programs, or xtask. Milestone 193 removed the first. Milestone 197's block argued
that `user/` had at least as good a claim on the prover as the kernel did, because it holds real
parsers over bytes this system did not produce.

That premise is half right, and the half that is wrong is worth stating first, because it is a
finding rather than a disappointment.

Most of the parsing is not in `user/` any more. Rule 7 (anything two binaries agree on is a
crate) and the host-testability discipline have already lifted it out. The initrd parser is
`nifefs`, the ELF front half is `elf`, the partition table is
`globally_unique_identifier_partition_table`, the directory entries are
`filesystem_contract`, the terminal escapes are
`video_terminal`, the shell's routing is `swish`, the pattern matcher is `glob`. Every one of those
is in `script/verify`'s table already. What is left in `components/src/*.rs` and `fixtures/src/*.rs`
is overwhelmingly IO glue:
programs that call a crate to decode, call `user_mode_runtime` to move the result, and render. That is the
tree working as designed, and it means the prize this milestone was reaching for had largely been
collected by other milestones under other names.

What is left is still worth proving, and one attempt at proving it found a live defect.

## The defect this found, which is the point

`components/src/rmle.rs` (the editor) holds a document as `MAX_ROWS` rows of at most `MAX_COLS` bytes, and
saves it by joining the rows with `\n` into one scratch buffer before writing that buffer to the
filesystem. The buffer was `MAX_ROWS * MAX_COLS`.

That is the size of the document's text, not the size of what `save` writes. The separators cost
`MAX_ROWS - 1` bytes more. A document at both bounds is 32 rows of 100 columns. The editor's own limits
permit it, and typing 31 characters onto the last line of a freshly loaded full file reaches it. It
staged 3231 bytes into a 3200-byte buffer and panicked the editor on `^S`.

No test found this. It was found by trying to state the property, before any harness ran. That is
the mechanism milestone 191 (did the proofs catch the bugs?) said the tree was missing. A property
has to be written down before it can be checked, and writing it down is where the two constants got
compared for the first time.

The fix is the buffer sized as `MAX_ROWS * (MAX_COLS + 1)`, one separator budgeted per row rather
than per gap, and a `const` assertion beside it rather than a proof:

```rust
const _: () = assert!(FILE_SCRATCH_LEN >= MAX_ROWS * MAX_COLS + (MAX_ROWS - 1));
```

That is rung one of AGENTS.md's ladder, and it is the right rung. The claim is a relationship
between three compile-time constants, so the compiler can hold it for nothing and will stop the
build at the line that is wrong. The next section is why it is not *also* a harness.

## What Kani could not do here, measured

A proof that does not terminate is not a weaker proof, it is no proof. Three properties were written,
run and abandoned before the one that shipped. One bounded a sum of 32 symbolic values, and got no
answer in 20 minutes on CaDiCaL or Z3. One took a symbolic index into a 3.5 KB editor struct, and
ran out of memory. One claimed values off a chain of 20 divisions. The third is the trap most likely to catch the next
person: `wc::render` verifies instantly if the assertion asks nothing about a byte's value, because
the slicer throws the divisions away. A fast harness is not evidence that the code is cheap to reason
about. What Kani is good at here is the opposite shape: bounded byte movement with comparisons.
[`what-kani-could-not-do.md`](user-proofs/what-kani-could-not-do.md) has the table and why each
shape fails.

## What is proved today

Two harnesses, both in `components/src/printenv.rs`, both about `push`, which appends what fits of a byte
string into a fixed 96-byte line buffer and drops the rest.

`printenv` prints `TZ`, `LANG` and `TERM` out of a configuration page `system_initializer` filled,
so two of the three arguments to `push` are bytes this program did not write, and the running
offset across three calls is a function of them. The one thing standing between that page and a
write past the end of a fixed array is a single `*n < buf.len()`.

- `push_never_writes_past_the_buffer_it_was_given`. For every starting offset in the whole of
  `usize` and every content, `push` writes nothing outside `buf`. And given an offset that starts in
  range it ends in range, appends (nothing below the starting offset moves) and reports what it
  wrote (nothing at or above the final offset moves). The starting offset is left unconstrained on
  purpose: memory safety must not depend on the caller, and here it does not, because the guard is a
  comparison rather than a subtraction.
- `the_buffer_can_be_filled_exactly`. A `kani::cover!` that the boundary is reached, so
  the assertion above is not being proved by an assumption set that never gets near it. DECISIONS
  §134 notes the tree had 23 `cover` sites against 141 harnesses; a bound nothing approaches is how a
  proof becomes decoration.

Cost: 2.4 seconds, against `script/verify`'s ~650. Both were falsified before they were
believed (`components/falsifications/`, and the section below on why no script replays it).

The unwind bound is `bytes.len() == 4`, and it is stated at the harness. The loop body is one
comparison and one store per byte, and the starting offset is symbolic over all of `usize`. So four
bytes already exercise every way a push can meet the boundary: entirely below, crossing, exactly on,
entirely past. What it cannot see is a defect that appears only at a larger length, and none is
expressible in a loop whose whole state is a `usize` that only increments.

## What it took, exactly

Shorter than milestone 193's list, because the kernel had already paid for most of it. Kani's own
`-C panic=abort` handled the unwinding error. `#[cfg(not(kani))]` on the panic-handler macro removed
a duplicate lang item. `script/verify` selects the binaries with `--bin`, derived from a grep of
the tree, because Kani refuses a `no_std` crate root that never mentions it. No
`--ignore-global-asm` is needed, since `user/` has no `global_asm!`. The derivation matters: a
hand-written list would silently drop the next binary to gain a harness.
[`what-it-took.md`](user-proofs/what-it-took.md) has each error and fix.

## The stub boundary, enumerated

A proof with an unexamined stub is worse than no proof, because it reads as coverage. This is the
exhaustive list of what a harness in a program package cannot see. The same list is at the top of each
`mod proofs`, where somebody writing the next harness will meet it.

1. Every capability is unreachable, and the boundary is hard rather than soft. `user_mode_runtime`'s
   `send`, `receive`, `call`, `invoke` and `exit` are `svc`/`ecall` through `asm!`, which Kani reports
   as an unsupported construct instead of proving past. So a harness that wanders into a program's
   IO fails loudly rather than reporting a proof about a fiction. This is the good direction and
   it is why the list below is short.
2. `MappedWindow` is a raw pointer to nothing. Every shared page (`CONFIG_VA`, `FS_VA`,
   `TERM_OUT_VA`, the DMA and ring windows) is a fixed virtual address a wiring maps before the
   program runs. Under a model checker nothing is mapped there. Do not write a harness that reads
   one; stub the boundary rather than pretend.
3. The panic handler is absent under `cfg(kani)`, so nothing proved here says anything about what
   a program does after a panic. Note the asymmetry with the kernel: an EL0 program dying is one
   process, and `user_mode_runtime::trap()` is what the supervisor sees.
4. `script/lint`'s harness-clippy pass excludes `components` and `fixtures`, exactly as it
   excludes `kernel`, and for
   the same two tooling reasons that pass's own comment carries. Practical consequence: keep these
   harnesses free of `unsafe`, because `undocumented_unsafe_blocks` does not fire inside them.
5. Only the binaries carrying harnesses are compiled at all. `cargo kani -p user` is never run
   bare, so a construct that would stop Kani in some *other* program is not discovered until somebody
   adds a harness there. That is a cost of the `--bin` selection and is the honest half of the
   argument for it.
6. The C component is not linked. `fixtures/build.rs` compiles `c/c_seam.c` into `c_shim` and
   `c/c_swappable.c` into `c_swappable`; on a host target it warns and emits nothing. Those two
   programs are not provable and `-Z c-ffi` is not enabled.

## Adding a harness to `components/src`

1. Put it beside the code it proves, in a `#[cfg(kani)] mod proofs`, not in a separate file. The
   stub list above is the reason: a reader has to meet the caveats where they meet the harness.
2. Put `#[cfg(not(kani))]` on that binary's `user_mode_runtime::panic_handler!()`, or it will not compile.
3. Check the call graph against the stub list. If it reaches `user_mode_runtime`, stop; Kani will say so
   rather than lie, but you will have spent the compile finding out.
4. Check the shape before you write it. Bounded byte movement, comparisons and small fixed
   arrays are cheap. A sum over many symbolic values, a symbolic index into a large struct, and any
   claim about values downstream of division are the three that did not finish; the table above has
   the numbers.
5. Falsify it. Break the code under it, watch the harness go red, and record the patch (DECISIONS
   §134). Then put the code back.
6. Nothing needs adding to `script/verify`: the `components` row is there and the `--bin` list is
   derived. A harness in `fixtures/src` is the exception, and it is this page's newest `BUGS`
   entry: that package has no row, so nothing would run it.
   If you add a harness to a package that is *not* in that table, `script/lint`'s "every crate with
   proof harnesses is in the verify table" check fails, which is the gate that exists because two
   crates spent months carrying harnesses nothing ran.

## Is `xtask` worth proving? No, and here is the argument

Milestone 197's block names `xtask` alongside `user/` and does not argue for it. Its own BUGS section
says so. This is the argument, and it is a refusal.

The front door is already open, and that is not the question. Measured 2026-08-31:
`cargo kani -p xtask --no-codegen` compiles with no changes to anything -- no `cfg`, no flag, no
`--bin`. It is ordinary host `std` code. So the cost of *starting* is zero and the case has to be
made on value.

Four reasons it is not worth it, in the order they matter:

1. **A defect in `xtask` cannot hurt anything that runs.** It builds images and drives QEMU on the
   developer's machine; it is never on the target, never handles an untrusted counterparty's bytes at
   runtime, and holds no capability. Its failure mode is a wrong gate result, not a compromised
   system. Every argument in DECISIONS §14 for a verified core is an argument about the core.
2. **Proving its parsers would destroy the thing that makes them worth having.** `xtask`'s
   hand-written decoders exist *precisely* to be a second opinion on the shared crate they check,
   which is why such a decoder does not share that crate's wire-format code: otherwise the gate
   would be checking the crate against itself. Aiming the same prover at both halves of a
   deliberately independent pair narrows the independence that is their entire justification.
3. **It is the environment Kani is least differentiated in.** `xtask` is host code with `std`, a
   heap, `#[test]`s, `cargo test`, a debugger and a stack trace. Kani earns its keep where testing
   cannot go: no allocator, no scheduler, an input space nobody can enumerate. None of that is true
   here.
4. **The cost is not zero even though the front door is free.** It is 8,511 lines in one file whose
   logic is process orchestration: `Command`, paths, environment variables, timeouts. That is
   the category a model checker has to stub out entirely, and a suite of stubs is where a proof
   quietly becomes decoration. Its genuinely pure logic already lives in the crates it depends on
   (`nifefs`, `gpt`, `manual`, `measured_boot`, `compositor`, `video_terminal`, `bitmap_font`),
   and every one of those is in `script/verify`'s table today.

What would change this answer: `xtask` growing logic that decides something the target then
trusts. The measured-boot digest is the shape to watch, and it is already in `measured_boot`, which
is the right place for exactly this reason.

## EXAMPLES

Prove the user programs' harnesses:

```console
$ cargo kani -p user --bin printenv --output-format=terse
...
Complete - 2 successfully verified harnesses, 0 failures, 2 total.
```

One harness on its own, with a counterexample trace on failure:

```console
$ cargo kani -p user --bin printenv \
      --harness push_never_writes_past_the_buffer_it_was_given
```

The whole suite, `components` included, the way CI runs it:

```console
$ script/verify
```

Falsify a harness before you believe it. Milestone 191 is why this is not optional: it measured
141 harnesses and found none that had caught a defect after the day it was written. No script
replays this one (see BUGS), so it is done by hand:

```console
$ git apply components/falsifications/proofs.push_never_writes_past_the_buffer_it_was_given.patch
$ cargo kani -p user --bin printenv --output-format=terse
Failed Checks: index out of bounds: the length is less than or equal to the given index
 File: "components/src/printenv.rs", line 126, in push
Complete - 0 successfully verified harnesses, 2 failures, 2 total.
$ git apply -R components/falsifications/proofs.push_never_writes_past_the_buffer_it_was_given.patch
```

Both go red on one relaxed comparison. That was run on 2026-08-31 and is the reason these harnesses
are worth their place rather than an assertion that they are.

## BUGS

- `fixtures` is not a row in `script/verify`'s crate table, so a harness added to a fixture runs
  nowhere and nothing says so (milestone 175). It carries no harness today, and a row with none
  fails the way that file's own comment describes: Kani refuses every `#![no_std]` crate root that
  does not mention it, so the run dies on the first binary without one. The failure mode is the
  invisible one this note already records twice, `multicast_dns_protocol` and then `jh7110_entropy`, where
  the suite goes green *faster* because a scope got smaller. If you add a `#[kani::proof]` under
  `fixtures/src`, add the row in the same change, and copy the `--bin` derivation `components` has
  a few lines above it. The honest reason this is recorded rather than fixed is that fixing it means
  either a row that cannot pass or machinery for a case that does not exist.

- ~~`script/falsifications` walks `crates/` only, so this milestone's record is outside its
  census and its sweep, and so are `kernel`'s two harnesses from milestone 193 (put `kernel/src`
  within reach of the prover), which carry no `Falsification:` record at all. The count that
  script prints is therefore a ratio over `crates/` rather than over the tree, and it does not
  know it. Two things are needed and neither is one line: the walk has to be derived from `cargo
  metadata` the way `script/lint`'s verify-table check already is, and `--sweep` shells `cargo
  kani -p <crate>`, which for the `components` package selects 49 binaries rather than one.
  Proposed as a milestone in this lane's report.~~ Closed. Milestone 212 (script/falsifications
  walks crates/ only, so the ratio it prints is not the tree's), 2026-09-01: the walk comes from
  `cargo metadata`, and `--sweep` derives `--bin` per package. Both this note's own harness and
  `kernel`'s now carry records and are counted. Struck 2026-09-23, part 5 of milestone 323 (the
  falsification record is incomplete in five ways), the same unstruck-`BUGS`-entry shape
  found twice more in `design/roadmap/0197-user-and-xtask-proofs.md`.
- Two harnesses is not coverage of 68 programs, and the number to watch is not the count but
  whether the properties are ones a defect would violate. This one is: the same guard, one character
  different, is the whole failure.
- The three shapes in [`what-kani-could-not-do.md`](user-proofs/what-kani-could-not-do.md)'s
  table are recorded, not solved. In particular the editor's
  editing operations (`insert_char`, `insert_newline`, `backspace`, `delete_at_cursor`) are the
  richest untrusted-input surface left in `user/` and are out of reach as the document is laid out
  today.
- `user/` is a package with 68 binaries and no library, so there is no `cargo kani -p user` that
  means "everything". Stub 5 above is the consequence.
- `user`'s 3 seconds in `script/verify`'s table is a dev-Mac number, like
  `jh7110_entropy`'s and `kernel`'s, and the wrong machine for that column. Replace it from the first
  CI log that carries it. Almost all of it is compile rather than solver time, so it will grow with
  the harnesses and not with the programs.
- This note does not price the rest of the work. What was measured is which shapes open and
  which do not.
