# Every place in the tree that enumerates architectures, and whether the list is complete

Swept 2026-08-27, prompted by one instance. `script/stack-frame-check` line 59 read
`arches="aarch64 riscv64"` and exited 2 on anything else. That gate was written 2026-08-14, when
there were two architectures; x86_64 became a `script/test` target on 2026-08-24 (commit
`357a226f`, milestone 161 (the x86_64 kernel port)), ten days later. The premise holds fine on
x86_64: the target is `x86_64-unknown-none`, `kernel/src/arch/x86_64/mmu.rs` prints "guard pages
are holes" from its own map builder, and `sched::every_thread_stack_has_a_guard_page` is
architecture-independent. Nothing technical stopped the third entry going in. Nobody added it, and
the script's `BUGS` section, which honestly records four other limitations, did not record this
one, so a reader met a section that looked complete.

That is the rung-2 failure `AGENTS.md`'s ladder describes. The coverage decision lived only in a
hardcoded string, and when a third architecture arrived nothing fired. The question this note
answers is how many more of those there are, which is `notes/arch-audit.md`'s question in a
different domain and is asked here for the same reason. The interesting output is not the bug that
prompted the sweep, it is the count of its siblings.

## The short answer

Eleven silent gaps, in nine files. Seven are lists that were correct when written and were never
widened. Two are content (an audit's scope, a script's `EXAMPLES`) rather than lists. One is a class
of four `#[cfg]` pairs whose failure mode is a silently empty function rather than a missing entry,
and one is a tripwire that exists, is committed, and is not armed.

Findings 1 through 10 are the sweep as it was first run. Finding 11 was added when the lane holding
`script/fastpath-footprint` turned out to have assessed and not changed it, so it stopped being
somebody else's and became one more of these. The prompting instance, `script/stack-frame-check`,
is deliberately **not** in the table: another lane converted it into a recorded gap the same night,
and that conversion is described in
[architecture-list-sweep/silent-gaps.md](architecture-list-sweep/silent-gaps.md) because it is the
model for how the rest should end.

Nine recorded gaps, which rule 5 explicitly allows and which are working as designed. They are
listed in [architecture-list-sweep/not-findings.md](architecture-list-sweep/not-findings.md) so a
later reader does not re-derive them as findings. `script/lint`'s x86_64 block is
the model and is worth reading before writing another one.

Fifteen-odd things that are legitimately one architecture's, which are not findings and are
listed only so the next sweep does not spend an hour rediscovering that a device tree does not
apply to a PC.

## Disposition, 2026-09-23, by milestone 186

Twenty-seven days after the sweep. Seven of the eleven are closed, one is now a recorded gap, one
is another milestone's, and two were already closed by other lanes before this one looked. That
is the finding worth carrying: a sweep's table is a snapshot, and a lane that acts on it without
re-checking each row will fix something twice and miss what moved.

| # | Where | Disposition |
|---|---|---|
| 1 | `script/stack-depth-check` | **Open.** Phase 3, and not a widening: its `TRAP_FRAME`, `DISPATCH` and `BODY` tables each need an x86_64 row measured against that architecture's own `size_of::<TrapFrame>()` and symbol names, and then the gate may find a real offender. Recorded in its own `BUGS`, which is where it already was |
| 2 | `script/bootstrap` | **Closed.** Derived from `rust-toolchain.toml`'s `targets`, the way `channel` above it already was |
| 3 | `script/drift` | **Was already closed**, by milestone 312 (`script/drift`'s lists are derived), which derives both the target list and the bare-metal subset. Its comment is the model the other two copied |
| 4 | `script/toolchain-bump` | **Closed**, and `.github/workflows/toolchain-bump.yml` with it, which the sweep did not list. Both install against the **new** toolchain, so the pin's array does not cover them; this was the copy that actually bit |
| 5 | `deny.toml` | **Closed.** `x86_64-unknown-none` added and the stale scope note replaced with what happened. `script/supply-chain` is green with the target in, so nothing was hiding in that graph. TOML cannot call a script, so it stays hand-kept and its `BUGS` now says so |
| 6 | CI's bench job | **Was already closed**, on 2026-09-15: `script/ci-build`'s `bench` row runs three legs |
| 7 | `script/bench`'s `EXAMPLES` | **Closed.** All three architectures and their baselines are in the header, with `--real` named as aarch64-only by construction rather than by omission |
| 8 | `notes/arch-audit.md`'s scope | **Another milestone's**, 187 (read the x86_64 arch tree through the lens the first arch audit used), which was minted for it |
| 9 | five driver `barrier()` functions | **Closed, and it was five rather than four.** See below |
| 10 | `components/src/pgrep.rs` | **Was already closed** with the sweep itself |
| 11 | `script/fastpath-footprint` | **Was already closed**: `arches` is three and x86_64 is measured. CI's job for it still installed two targets a month later, which this milestone fixed |

### Finding 9 was five copies, not four, and the fix is one function

The sweep named four `barrier()` copies and there were five: `components/src/entropy.rs` had the
same two arms and no `BUGS` note. They are now one function,
`user_mode_runtime::virtio::virtio_ring_barrier` (name provisional), so a seventh driver gets the
third architecture by calling it. On x86_64 it is a compiler fence: every call site needs only
store-store or load-load ordering, which TSO gives, and every notify leaves through a serializing
syscall. `non_volatile_memory_express.rs` keeps `dmb sy` and a real `mfence`, because its doorbell
is a direct store with no syscall in between. The argument site by site is in
[architecture-list-sweep/virtio-ring-barrier.md](architecture-list-sweep/virtio-ring-barrier.md).

### What is still open after this milestone

- Finding 1, `script/stack-depth-check`, which is phase 3 work that may surface an offender.
- Finding 8, which is milestone 187 (read the x86_64 arch tree through the lens the first arch
  audit used).
- The rung-1 limit this note already stated and which still holds: deriving the list removes the
  copy, not the incompleteness. Nothing stops a new gate writing `for arch in aarch64
  riscv64`, and `deny.toml` cannot read the derivation at all. `virtio_ring_barrier`'s
  `compile_error!` is the one place in this sweep's territory where a missing architecture now
  fails to build rather than failing to order.

## The three classifications, and why the classification is the deliverable

Rule 5 in `AGENTS.md`, from §19 (architectural parity is a tenet):

> A kernel capability ships on every supported architecture, proven by the same suite, or a scope
> note records the gap and the plan. If a feature works on one ISA and silently not another, that
> is the bug.

The word doing the work is silently. A hit is a finding only when all three of these hold:

1. An architecture is absent from the list, match, table, file set, or matrix.
2. The premise would hold there. A stack-frame ceiling means something on x86_64; a device tree
   fixture does not.
3. Nothing in the tree says so. Not a `BUGS` entry, not a `skip!()` reason, not a scope paragraph
   beside the code, not a roadmap block.

Fail 2 and it is architecture-specific. Fail 3 and it is a recorded gap. A sweep that reports the
raw count instead of the split is worse than one that reports four real gaps, because the reader
has to redo the judgment.

## Silent gaps

The eleven, as the sweep found them on 2026-08-27, are tabled with their evidence in
[architecture-list-sweep/silent-gaps.md](architecture-list-sweep/silent-gaps.md), beside the two
severity notes and the two gaps another lane held while the sweep ran. Seven are lists that were
never widened, such as `script/bootstrap` installing two of the three targets the very next
paragraph says "`cargo xtask test` now needs the full set". Finding 5 is the worse variant, a scope
note gone stale: `deny.toml` said x86_64 bare metal "is declared in §19 but not started; it joins
this list when it does", and it started. Findings 9 and 10 are two-arm `#[cfg]` pairs whose x86_64
behavior was an empty function, which no gate catches because it compiles clean. Finding 8 is an
audit's scope that never read x86_64's 6,797 lines.

## The things that are complete, because they are the answer to the design question

Nine recorded gaps are working as designed, and fifteen-odd things are legitimately one
architecture's; both are listed in
[architecture-list-sweep/not-findings.md](architecture-list-sweep/not-findings.md) so the next sweep
does not rediscover them. So are the lists that stayed complete, and the pattern in them is the
answer to the design question. Everything that stayed complete is either a Rust `match` the compiler
pushed on, or a per-architecture file whose absence a build notices. Everything that went stale is a
space-separated string in a shell script, a YAML step, a TOML array, or a sentence in a note. One
hazard is not a finding today: ten `#[cfg(not(target_arch = "x86_64"))]` sites are correct at three
architectures and will silently include a fourth, so a fourth port's first task is a grep for
`not(target_arch`.

## The design question: is there a rung-1 answer

Widening ten lists by hand is rung 2 done ten times. It fixes 2026-08-27 and does nothing on the
day a fourth architecture arrives, which the roadmap contemplates. So: what would make an
incomplete list hard or impossible to write? Three candidates, priced rather than asserted, with
the six questions `AGENTS.md` asks of a fork answered where they apply.

Three options were priced. Option A is one derived list of architectures and triples that every
script reads, about fifteen lines of POSIX sh over `rust-toolchain.toml`; it removes the copy, not
the incompleteness. Option B is a type with no default arm, already taken where Rust can see it, and
extendable as a `compile_error!` arm at the `#[cfg]` sites. Option C, a gate comparing each
per-architecture file set against the list, would ship with six exceptions against sixteen families
and would not have caught the prompting instance. Each option's cost and limits are in
[architecture-list-sweep/design-options.md](architecture-list-sweep/design-options.md).

### The recommendation

This fork is reversible in every part, so it gets a recommendation rather than options, which is
the line `AGENTS.md` draws.

**Take A, take B's `compile_error!` arm, do not take C.**

A, because five of the ten findings are a copied list that went stale and A is the only option that
removes the copying. Its own limit is real and worth saying in the same breath: it prevents
divergence, not omission.

B's arm, because findings 9 and 10 are the only ones whose failure mode is silence rather than
absence, and silence is the mode no reader and no reviewer catches. Three lines at five sites, with
an in-tree precedent.

Not C, because a check that ships with an exception table a third its own size is a check whose
exceptions become the artifact, which is the failure `stack-frame-check` already documents about
itself.

What none of the three fixes, stated plainly so it is not mistaken for covered, is three findings.
Finding 6 wants a CI step, finding 7 wants two lines of `EXAMPLES`, and finding 8 wants somebody to
read 6,797 lines of x86_64 assembly and Rust the way `notes/arch-audit.md` read the other two. Those
are work, not mechanism.

On prior art outside this tree: not researched. `AGENTS.md` asks for it read rather than
recalled, and this lane did not go outside the repository, so nothing is claimed. If it is worth an
hour, the question to ask is how a multi-target project keeps one target list authoritative across
a build system, a CI matrix, and a package manifest, since that is the general form of option A.

## Where the work went

Closing the eleven silent gaps is a real body of work rather than a sed. Finding 1 needs an x86_64
trap-frame size and dispatch symbol names; finding 6 needs a CI leg that may find real drift the
first time it runs; finding 8 is an audit. It is **milestone 186 (derive the architecture list, and
close what it does not reach)**, minted on calef's question against this sweep, and this note is
that milestone's worklist. The block carries the phasing and the scope refusals; the disposition
table above and the sweep's own table carry the items.

Two things the milestone is explicit about, because both were decided here.

**Do not do it as one commit.** Widening an architecture list newly gates that architecture, and a
gate that has never run against x86_64 may have real offenders. It already does:
`script/stack-frame-check --arch x86_64` surfaced `kernel::arch::x86_64::iommu::initialize` at
12,504 bytes against a 4,096-byte guard page, found by the lane on pull request #567 the same night.
Ten widenings in one commit turns `main` red for a reason unrelated to whoever pushed and leaves
nobody able to tell which one did it. The loader commit of milestone 96 (one init: the spawn
service written twice) is the precedent: separateness is the argument.

**Finding 10 was not scheduled, it was fixed.** `components/src/pgrep.rs`'s panic handler was a live
defect rather than a coverage gap: on x86_64 neither arm compiled, control reached the spin loop,
and a panic burned a thread forever. It was the last hand-rolled panic handler left outside
`user_mode_runtime` after milestone 130 swept forty-eight of them, and the fix was to use
`user_mode_runtime::panic_handler!()` like every other program in `user/`. Kept in the sweep's table
as a finding, since the point of an inventory is what the sweep found rather than what survived it.

**Finding 8 is deliberately not in milestone 186 either**, and for the opposite reason: it is too
large, not too small. Reading 6,797 lines of x86_64 assembly and low-level Rust on
`notes/arch-audit.md`'s terms does not fit beside a `rustup target add` line. Proposed there,
provisionally, as its own milestone.

## EXAMPLES

Reproduce the sweep, or run it again after a fourth architecture lands.

The list that should be authoritative:

```sh
$ grep '^targets' rust-toolchain.toml
targets = ["aarch64-unknown-none-softfloat", "riscv64imac-unknown-none-elf", "x86_64-unknown-none"]
```

Every hand-copied triple list, to diff against it:

```sh
$ git grep -n 'aarch64-unknown-none-softfloat' -- script scripts .github '*.toml' \
    | grep -v 'x86_64-unknown-none'
```

Each hit is a place that spells the list and does not mention the third target. Read each one:
some are legitimately single-target (`script/crate-probes` builds one spec on purpose).

Per-architecture file sets, and which are incomplete:

```sh
$ git ls-files | grep -E 'aarch64|riscv64|x86_64' | grep -v '^vendor/' \
    | sed -E 's/(aarch64|riscv64|riscv|x86_64|x86)/<ARCH>/g' | sort | uniq -c | sort -rn
```

A family with a count of 3 is complete. A count of 2 is either a recorded gap, a legitimate absence,
or a finding, and only reading it tells you which. This is the method that found
`bench/fastpath-<ARCH>.txt` at two beside `bench/baseline-<ARCH>.txt` at three.

Two-arm `#[cfg]` blocks with no fallback, which is finding 9's shape:

```sh
$ git grep -l 'target_arch' -- '*.rs' | grep -v '^vendor/' | while read f; do
    a=$(grep -c 'target_arch = "aarch64"' "$f")
    r=$(grep -c 'target_arch = "riscv64"' "$f")
    x=$(grep -c 'target_arch = "x86_64"' "$f")
    [ "$a" -gt 0 ] && [ "$r" -gt 0 ] && [ "$x" -eq 0 ] && echo "$f"
  done
```

Default-arm traps, which is what made `EXPECTED_MACHINE` wrong:

```sh
$ git grep -n 'not(target_arch' -- '*.rs' | grep -v '^vendor/'
```

## Methods, and what each one cannot see

Four methods were run, in order. A grep for architecture names and triples, read file by file,
found findings 1 to 7. Per-architecture file-set counts found the `bench/fastpath-` divergence.
`#[cfg]` arm counts per file found findings 9 and 10, and a read of every "both ISAs" claim found
finding 8. The sweep did not read
`design/roadmap/` or `design/decisions/`, and did not run any gate against x86_64. What each method
is blind to is in [architecture-list-sweep/methods.md](architecture-list-sweep/methods.md).

## BUGS

- It is a snapshot, and the thing it measures moves. Every finding is against the tree at
  commit `c4854083`, on 2026-08-27. Findings 2 through 5 in particular are one-line edits that
  somebody may land the same week, and this note will not know.
- "Complete" here means three, and three is today's number. Every judgment in this note assumes
  the supported set is aarch64, riscv64, and x86_64, which is what §19 says today. A fourth
  architecture invalidates the file-set counts and the `not(target_arch = "x86_64")` hazard becomes
  ten live bugs rather than a note.
- **Severity is not ranked.** The table orders findings by where they live, not by what they cost.
  Finding 8 (an unaudited 6,797-line arch tree in the trusted computing base) and finding 7 (a
  stale `EXAMPLES` block) sit in the same list and are not the same size of problem.
- The three classifications are a judgment and two of them are contestable. In particular,
  findings 9 and 10 could be argued as legitimately architecture-specific on the ground that x86 is
  TSO and needs no machine barrier. This note calls them silent gaps because the compiler-reordering
  half is uncovered and because an empty function body records nothing either way, but a reader who
  disagrees is disagreeing with an argument rather than with a count.
- No gate enforces any of this. The sweep is a one-time read, like `notes/arch-audit.md` and
  `notes/untracked-work-sweep.md` before it, and it will go stale the same way both of those did.
  That is the argument for option A rather than for a longer note.
