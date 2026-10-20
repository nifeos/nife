---
status: REFUSED
raised: 2026-09-03
refused_by: 238
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 244. The largest crate in the tree is proved by nothing a mutation can reach

Refused on its own measurement: this block's "what would make this milestone wrong" clause fired.
The question came from the first published mutation report of milestone 238 (two scheduled checks
have never once succeeded), and the lift it asked for would move too little to be worth a crate of
fragments.

Nothing is missing; what is missing is a way for a test to reach the code.

## Revisit

- **Condition.** `cargo mutants --list -p system_initializer` attributes most of its mutants to pure functions rather than to the boot sequence and its syscalls, or the boot wiring becomes data (the follow-up proposed below), which would give a host test something to reach.

## What the lane was asked to do, and what it found

The block below is kept as it was written, because the finding is a verdict on its own question and
a reader has to see the question to weigh the answer. This section is the answer.

The pure fraction is small, so nothing was lifted. That is the outcome this block named in
advance as the one worth reporting rather than building through, and the numbers are:

`cargo mutants --list -p system_initializer` generates 196 mutants (five more than the 191 the
238 report scored, because the file moved between the two runs). By cargo-mutants' own function
attribution:

| | mutants | share | what it is |
|---|---|---|---|
| `boot` | 97 | 49% | the build sequence |
| `spawn_service` | 32 | 16% | the `RECV` loop, one syscall per step |
| `fill_entropy`, `build_caretaker`, `announce`, `reclaim`, `must`, `must_ok`, `memory_region_split` | 28 | 14% | syscalls with a loop around them |
| `hex_password`, `sentence` and its `push`, `boot`'s own second copy of that `push`, `opt_cap`, `archive_name`, `measured` | **33** | **17%** | pure: bytes in, bytes out, no capability touched |
| top-level `const` arithmetic | 6 | 3% | not movable; they are what the rest is written against |

A sixth of the mutants and about a fiftieth of the lines. The six pure functions total roughly
sixty lines of a 2,632-line file, and every one is a leaf helper of the sequence beside it.
`measured` fills a `Lookup` that only `boot` destructures, `opt_cap` reads one word out of one
`recv_cap`, `archive_name` is `Some(p.name())`. Lifting them buys 33 reachable mutants and costs
exactly what this block predicted: a crate of fragments, a wider public surface, and a reader holding
two files to follow one boot.

The tree's own analogous case was checked and runs the other way up. `script/lint`'s bare-metal
gate names `redoxfs_server` as the pattern to copy: `user_rt` behind an optional `el0` feature, the
sans-IO core host-testable and the EL0 binary behind `required-features`. That works there because
the sans-IO core is most of the package. Here the sequence *is* the package, and the same split would
put 2,570 lines behind the feature and 60 in front of it. Worse, it would make `cargo mutants` report
a score for a crate whose boot path does not exist in the configuration being scored, which is a
number a reader would take for a claim about the init. An honest zero beats a flattering fraction.

### The finding that was worth more than the lane

**`system_initializer` was never in `.cargo/mutants.toml`.** The other three crates in its position
(`supervision_proto`, `swap_proto`, `virtio`) are, and that file's own head comment says its list
"deliberately mirrors script/coverage's exclusions" and asks the next person to "keep the two lists
in step". That is rung four of AGENTS.md's ladder, and it drifted by exactly one crate: the largest
one, added to `script/lint` and `script/coverage` when milestone 96 created it and never to the
mutation config. So the 191-mutants-0-caught line in the first published mutation report was not a
measurement of this crate at all. It was a crate being scored against a suite that could not compile
it, and it was one of the three crates that report blamed for the tree's score falling from 92.4% to
83.4%.

Fixed, and the rung raised so it cannot recur: `script/lint`'s "host pass excludes exactly the
bare-metal crates" gate already derived the set from `cargo metadata` and checked two consumers
(`script/lint`'s own clippy lines, `xtask/src/main.rs`). It now checks four, and the two it
gained are the two that publish a number: `script/coverage`'s exclusions and `.cargo/mutants.toml`'s.
Verified by removing the new entry and watching the gate fail, naming the crate and the file.

### The corrected mutation numbers

Removing a crate no host test could reach changes a published rate, so `notes/mutation-testing.md`
carries the recomputation rather than a quiet edit: the round-robin shard's 83.4% becomes 85.3%
and the `slice` shard's 74.4% becomes 81.0%. Neither is a new measurement. Both are the same runs
with a denominator that no longer counts mutants nothing could have killed.

### What is left, and it is not a host test

**The largest single group of mutants is the one a lift would not have reached either.** Thirty-four
of `boot`'s 97 delete a field from a `ChildEndowment` struct expression: a child built with no
`caps`, or no `stack_pages`, or no `maps`. Twenty-four more are slot-counter arithmetic and twelve
flip a rights mask's `|` to `&` or `^`. Those are not logic with a wrong answer. They are a
declaration of what each component of the system may do, expressed as inline struct literals in
the middle of a syscall sequence. The only two things that can check a declaration are the boot
itself and a test comparing it against a separately written expectation. That is the lane worth
having, and it is proposed below rather than attempted here.

## BUGS

- **This does not close the other three excluded crates.** `supervision_proto`, `swap_proto` and
  `virtio` are excluded for the same reason and are not in this block. Whether the same argument
  applies to them is not answered here; the measurement above is cheap to repeat for each
  (`cargo mutants --list -p <crate>`, then read the function attribution) and nobody has.
- **The 33 pure mutants stay unreachable and that is a cost, not a nil.** `measured` is the one that
  matters: it is measured boot's refusal policy, where "the table says nothing about this name" and
  "the table says something else" must both refuse, and a single mutant sits on it. It is proved
  today only by `script/shell-check` booting a system whose table happens to be right.
- **A mutation score over the lifted crate is not a claim about the init**, and no lift happened, so
  there is no score. What proves the init is `script/shell-check`, unchanged.
- `script/shell-check` remains the only thing that runs a real init, and nothing here changes
  that or should be read as reducing its standing. The gate this lane added is about a *report*, not
  about the boot.

## Proposed follow-up: the boot wiring is a declaration, so let it be data

*(Provisional; the integrator mints the number.)* Thirty-four of `boot`'s mutants delete a field from
a `ChildEndowment` literal, and nothing in the tree would notice. The proposal is not a host unit
test, which cannot see them, but making the wiring a table this crate walks rather than a
sequence of literals. A host test can then assert the table (what each boot component is endowed,
with which rights) without a kernel underneath, and `boot` becomes the walk over it. What that buys
beyond the mutants is the thing this crate's own `BUGS` section already spends four paragraphs on.
The capability-table budget is a property *of the table*, and today it is discovered by a boot that
prints nothing at all.

The honest cost, so it is priced rather than assumed. The sequence's order is load-bearing (this
crate's head comment says so, over three evenings' worth of evidence). The table must be ordered
and the walk must be the same order. A table that is really a sequence with extra ceremony is
worse than what is there now. Whether it can be genuinely declarative is the fork that lane opens
with, and it is an architect's.

---

*Everything below is the block as minted, kept because the verdict above is an answer to it.*

In brief. `crates/system_initializer` is 2,632 lines with zero `#[test]`, and milestone 238's
report scored it 0 caught of 191 mutants. Every mutation of every function in it survives.

**That is not negligence, and reading it as negligence would produce the wrong lane.** `script/lint`
excludes the crate from the host pass on purpose, and says why beside the exclusion: it takes an
unconditional dependency on `user_rt`, which is EL0 `asm!`, so it cannot compile for the host at all.
Four crates are in that position (`supervision_proto`, `swap_proto`, `virtio`, and this one) and this
is much the largest. `cargo mutants` builds the crate and then runs a workspace suite that
structurally cannot reach it, so the zero is arithmetic rather than a discovery.

The crate's own head comment already names what proves it instead, honestly:

> the thing that actually proves this code is `script/shell-check`, which boots both ISAs and types
> at the prompt.

(That gate is `script/swish-check` since 2026-09-23. The quotation and this closed block keep the
name they were written under; the comment in `.cargo/mutants.toml` reads `swish-check` today.)

**That gate is real and it is not nothing**, which is the fact that makes this milestone a judgment
rather than an alarm. `script/shell-check` is the only thing in the tree that runs a real init.
Milestone 96 (one init: the spawn service written twice) exists because a fix landing in one init
and not the other produced *a boot that reaches
userspace and prints nothing at all*, with no fault and no message, three times. What it cannot do is
tell which of 191 mutations it would have caught, and a gate whose coverage nobody can state is a
gate nobody can improve.

### What this milestone is, in one sentence

AGENTS.md's own rule, applied to the largest place the tree breaks it:

> Pure logic (allocator algorithms, page-table math, scheduling policy, filesystem parsing) belongs
> in crates that compile for the host, so most tests run in milliseconds without an emulator.

This is milestone 193's (put `kernel/src` within reach of the prover) option B with a name and a
number. 193 chose option A for the kernel and said the honest answer is probably both, with the split
decided by where a property naturally lives. This block is that sentence cashed out for the one crate
where the cost of not doing it is measured rather than argued.

### What is actually in there, because the split is the whole question

The crate holds two kinds of code and they are not mixed evenly:

- Logic with a right answer that a host can check. Reading the archive, decoding a `grant_plan`
  off the spawn channel, checking a `measured_boot` manifest against what it is about to load,
  choosing addresses to map an image at. All of this is `no_std` arithmetic and parsing over bytes.
- Syscalls on capabilities the kernel granted at spawn. `boot` returns `!`, and every step it
  takes is an `svc`. There is nothing to assert and nowhere to assert it.

**The deliverable is the first kind moved somewhere a test can reach, not the second kind
simulated.** A mock kernel is the failure mode to avoid here: it would produce a large green suite
proving that the mock behaves the way the code expects, which is the thing already assumed.

### The proof that this milestone worked

A mutation run over the new host-reachable crate catches most of what it generates, reported the
way milestone 238's does, plus the number that shows the split was worth making. How many of
`system_initializer`'s 191 mutants now live in code a host test can reach.

Not a line count moved, and not a test count. Either of those is satisfiable by moving the easy half.

### What would make this milestone wrong

Worth stating in advance, because a lane that finds it should say so rather than build anyway:

If the pure fraction turns out to be small. The crate may be mostly syscall sequencing with
arithmetic threaded through it, in which case lifting it produces a crate of fragments, a wider
public surface, and a reader who now has to hold two files. That is a worse tree than a 2,632-line
crate with an honest note saying `script/shell-check` is what proves it. Measure the fraction
before moving anything, and if it is small, say so and stop; the finding is worth more than the
lane.

*(This is the clause that fired. See the verdict at the top.)*

## Index row

Measured: only 33 of 196 mutants are in pure logic, so nothing was lifted. The 0-of-191 score was
a config gap (the crate was missing from `.cargo/mutants.toml`), now fixed and gated
