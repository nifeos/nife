# The design question: three options, priced

*Name: provisional.*

An appendix to [`notes/architecture-list-sweep.md`](../architecture-list-sweep.md), which states the
result.

## Option A: one derived list of supported architectures and their triples

What. A single place naming each supported architecture and its triple, that every shell script
and every `xtask` path reads instead of spelling.

What the tree already does here, which is most of the answer. `rust-toolchain.toml` already
carries the list, complete at three, and is already the authority for the pinned toolchain: rustup
reads that array, which is why CI's clippy job explicitly installs only aarch64 and still lints
x86_64 clean. `script/bootstrap` already parses that same file with a one-line `sed` to get
`channel`. So the mechanism, the file, and the parsing technique all exist, and the gap is that
nobody extended the parse by one field.

What it costs, measured. The arch-to-triple table is currently written out four times:
`script/stack-frame-check`, `script/stack-depth-check`, `script/fastpath-footprint` (all three as
`case "$arch" in ... esac`), and `xtask`'s `TARGET` / `RISCV_TARGET` / `X86_TARGET` constants. The
bare triple list is written out four more times: `script/bootstrap`, `script/drift`,
`script/toolchain-bump`, and `deny.toml`. A `script/` entry point printing `arch<TAB>triple` per
line is about fifteen lines of POSIX sh reading `rust-toolchain.toml`. Each `case` block becomes a
lookup; each `rustup target add` becomes a command substitution. `deny.toml` is TOML and cannot
call a script, so it stays a hand-maintained list and needs option C or a counted claim to keep it
honest.

**What it does not do**, and this is the honest limit. It removes the copy, not the
incompleteness. A new gate can still write `for arch in aarch64 riscv64` and nothing stops it.
It also does nothing for findings 6, 7, and 8, which are a missing CI step, a stale `EXAMPLES`
block, and an unaudited directory. Those are content, not lists.

Reversibility. Total. It is one script and its callers, nothing leaves the machine, no wire
format and no name a stranger has learned. By the *move fast on what can be undone* test, nobody
outside this repository has acted on it.

## Option B: a type with no default arm

What. An enum the compiler forces exhaustive matching on, and a `compile_error!` arm where
`#[cfg]` does the dispatching.

This half already exists and has already paid for itself. `ArchLegs` is that enum, and its own
doc records the bug it was created by: two variants, predicates written as `self != the_other_one`,
and every leg answering `true` once there was a third. `crates/elf`'s `EXPECTED_MACHINE` is the
same correction in the same week. Its reach is the problem: no shell script, YAML step, or TOML
array can see a Rust enum, and nine of the eleven silent gaps live in exactly those three languages.
So B is already taken where it applies and cannot apply where the gaps are.

Where it extends, and this is the half worth building. Findings 9 and 10 are five `#[cfg]`
pairs whose x86_64 behavior is an empty function body. Adding

```rust
#[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64", target_arch = "x86_64")))]
compile_error!("barrier(): this architecture has no ordering instruction named here");
```

turns "fell through to nothing" into a build failure. Cost: five sites, three lines each,
mechanical. Precedent in the tree: `entropy_backend`'s backend ladder, whose own module doc says it
"ends in `compile_error!` rather than a fallback", which is the only existing use of the macro here.
Reversible.

**The caveat that makes this a partial answer rather than a fix.** `compile_error!` catches the
*fourth* architecture. It does not catch x86_64 today, because x86_64 is inside the `any(...)` and
the arm still emits nothing for it. Closing finding 9 properly means deciding what a barrier is on
x86_64: `compiler_fence` for the compiler half, since TSO covers the machine half. That is a
memory-ordering decision under rule 4, not a sweep's call, and it is the same argument in four
copies, which is `AGENTS.md` rule 7's own case for one crate rather than four functions. Flagged as
a fork rather than answered here.

Finding 10 is not this option's shape at all and should not be bundled with it. `crates/user_mode_runtime`
already has a panic handler with arms for all three ISAs, and `pgrep` is the only program in
`user/` that hand-rolls its own. The fix is to delete the hand-rolled one, which is deletion rather
than mechanism.

## Option C: a gate comparing each per-architecture file set against the list

What. For every family of files whose name carries an architecture token, assert the set equals
the supported list.

What the tree already does here. `script/lint`'s counted-claims check is exactly this shape one
domain over: it re-derives a number from the tree and fails when a written claim disagrees, with
the marker at the claim (`<!--count:sh-scripts-->`, `notes/counted-claims.md`). A file-set check is
the same primitive over globs instead of counts.

What it costs, measured, and this is why it loses. There are sixteen file-set families in the
tree today. Three are legitimately incomplete and would need an exception on day one (the `dtb`
fixtures, the port notes, the shootdown notes). Three more are recorded gaps and would need an
exception until their milestones land (`targets/*.json`, `bench/fastpath-*.txt`,
`fastpath_pad.rs`). That is six exceptions against sixteen families: the exception table would
be more than a third of the check on the day it shipped. `script/stack-frame-check`'s own `BUGS`
already records what that costs, in the entry that says its exception table is maintained by hand
and that a function which moves keeps its exemption with nothing noticing.

**And it would not have caught the finding that prompted this sweep.** `script/stack-frame-check`'s
gap was a string inside a script, not a missing file.
