---
status: NOT-STARTED
raised: 2026-09-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 383. A gate for a backticked in-tree path that does not resolve

Filed 2026-09-05 as an unnumbered proposal by milestone 259's notes sweep,
which spent more than half its corrections on this one shape; numbered 2026-09-19 by milestone 433's
drain of the proposal pile. Premise re-read against the tree on 2026-09-19 and still true:
`script/lint`'s markdown section still checks relative link targets and the `notes/README.md` index
and nothing else, and its own comment still says backticked repo paths are "deliberately NOT
checked". That comment has already retracted half its own justification. It was re-measured by
milestone 93 (documentation audits as a mechanism)'s sweep at 31 unresolvable backticked
paths over 379 markdown files,
where it had claimed a checker would be 100% false positives. It now rests on the
false-positive *rate* rather than on perfection. This block is the proposal that the rate is
enumerable. *(Number provisional until the merge queue lands it.)*

It reads the tree and needs nothing.

## In brief

262 citations in `notes/` pointed at a crate, a file or a Rust path that had been renamed away,
and every gate in this repository passed them. `script/lint` check 4c verifies that a markdown
*link* target exists. `script/citations` verifies that a `§N` or a `milestone N` resolves to the
thing the author meant. A path in backticks is neither, so `` `crates/fs_proto` `` was
unfalsifiable prose for the two weeks after §75's naming pass renamed the crate.

[`notes/follow-on-work/first-sweep.md`](../../notes/follow-on-work/first-sweep.md#two-things-the-sweep-found-that-were-not-in-its-brief)
recorded the same rot one directory over, in the roadmap's own blocks, and
its sentence is the whole argument for this: *"Nothing had been checking a path cited in a roadmap
block."* Nothing is checking one cited in a note either, and there are more of them.

## What it would check

For each `` `path` `` in a tracked markdown file where the path starts with a directory that exists
at the repository root (`crates/`, `kernel/`, `user/`, `script/`, `design/`, `notes/`, `fuzz/`,
`bench/`, `patches/`, `xtask/`, `tools/`). Assert the path exists, after stripping a `::symbol`
suffix and an anchor.

The false positives are the whole design problem, and they are enumerable, which is what makes
this rung two rather than a `git grep -w TODO` at 82%. Milestone 259 hit exactly four kinds:

1. **Verbatim transcripts.** A quoted panic named `crates/frames/src/lib.rs` and a line number because that is what
   it said, and rewriting it would falsify the evidence. Two instances.
2. Deliberate past tense. `notes/ntlm.md` and `notes/smb.md` describe code removed on
   2026-08-30 and say so in their first line; `notes/heap.md` carries a banner recording that
   `crates/heap` was deleted. Fifteen instances across four files.
3. **A path named because it does not exist.** `notes/register-of-measures.md` says "there is no
   wrapper and there should not be one" about `script/measures`;
   `notes/footprint-perturbation.md` says `kernel/src/arch/x86_64/fastpath_pad.rs` does not exist.
   Three instances.
4. Elided or illustrative paths. `patches/std-nife/.../pal/nife/rt.rs`, and
   `notes/documentation-audit.md`'s `design/dec/` in an argument about spelling words out.

So the check needs an escape, and the cheapest honest one is an allow-list file carrying the
reason per entry, the shape `xtask`'s `ABORTS_ACCEPTED` already uses. Twenty-odd entries is a
readable list, and a new one arriving with no reason is what the gate is for.

## The trap this must not fall into, which the sweep found by falling into it

A crate is named three ways in this tree: as a path (`crates/fs_proto`), as a Rust path
(`fs_proto::PAGE`), and as a bare name in prose (`` `fs_proto`'s own BUGS section ``). The sweep's
first pass matched the first, reported itself clean, and left 167 instances of the other two. The
same thing happened one size smaller in `notes/documentation-audit.md`, whose own path check matched
`` `user/src/virtio.rs` `` and missed `` `user/src/virtio.rs::write_block` `` on the next line.

A check that only knows one spelling reports a clean tree and is worse than none, because it retires
the worry.

**The bare-name form is the hard one and probably should not be gated.** `` `slots` ``, `` `frames` ``
and `` `regions` `` are ordinary English words, and every occurrence in `notes/` turned out to be a
crate reference only because those were bad crate names. A gate over bare identifiers would fire on
prose. The path forms are unambiguous and are where a reader actually goes.

## Scope

**Not scoped to `notes/`.** `fuzz/seeds/README.md` carries the same dead path as
`notes/fuzzing.md` (`crates/elf/tests/fuzz_seed.rs`, deleted by `acc2338a` and restored by
milestone 259), and `design/roadmap/` has the disease `notes/follow-on-work.md` already recorded.
Every tracked markdown file, or the check is the sweep it is trying to replace.

## What it would have caught

Every one of milestone 259's 262 path corrections, and one thing worth more than the prose: the
`crates/elf/tests/fuzz_seed.rs` deletion. That commit was about adding `Segment::p_paddr` and never
mentioned the 62-line test file it removed, so a documented safeguard disappeared and two
documents kept describing it in the present tense for five days. A path check would have failed
that commit's own CI run.

## The symbol half, which this check strips and a rename walks into

*Added 2026-09-19 by the maintainer, from the `ipc` to `inter_process_communication` rename.*

The check above strips `::symbol` and asserts only the path, so `` `ipc::Endpoint` `` passes as long
as the crate exists. §113 renamed `Endpoint` to `Rendezvous` on 2026-08-23, and nine prose sites
still cited `ipc::Endpoint` a month later. A crate-rename sweep would have turned every one into
`inter_process_communication::Endpoint`, a type that has never existed:
design/naming/rename-traps.md's "A sweep can turn a stale pointer into a fabricated one" has the
case. A symbol that does not resolve is the same defect as a path that does not, one `::` further
along.

Measured with a throwaway prototype, 2026-09-19: over every tracked `.md` file, match
`` `crate::Symbol` `` where `crate` is a directory in `crates/`, and grep that crate's `src/` for a
definition of `Symbol`.

- 992 such references; 122 do not resolve (66 in `design/roadmap/`, 9 in
  `design/decisions/`, the rest in `notes/`).
- **The precision is poor, and for a reason this proposal's escape does not cover.** The sample
  mixes true stale pointers (`capability::CSpace`, which §113 renamed) with kernel modules that
  share a crate's name: `pci::find_block_device` and `virtio::register` live in
  `kernel/src/pci.rs` and `kernel/src/virtio.rs`, not in `crates/pci` or `crates/virtio`.

So the symbol check wants two things the path check does not: resolve against both namespaces
(`crates/<c>/src` and `kernel/src/<c>.rs` or `kernel/src/<c>/`), then remeasure before deciding
whether the remainder is an allow-list or a gate. Resolution by grep will still miss a symbol made
by a macro or reached through a re-export, which is a BUGS line for the check rather than a reason
not to write it. Nothing was fixed from the 122; they want classifying first, by status, as the
rename procedure says.

## The directory half, which a drawer rename walks into

*Added 2026-09-24 by a maintainer-delegated lane, from rebasing #1194 (`scripts/` becomes
`helpers/`).*

A directory rename is the one change that makes every old-spelling path wrong at once, and it
cannot be finished by the branch that does it. #1194 swept 665 references and was rebased three
times in one day. Each rebase found new `scripts/` paths that had landed on `main` in the meantime,
from pull requests that never conflicted with the rename: 11 after the first rebase, 2 more after
#1172 and #1202. At the third, five pull requests already in the merge queue ahead of it added about
32 more lines naming `scripts/` (#1208, #1210, #1200, #1203, #1209). Nothing fails in either order.
If the rename lands first, those paths ship dead. If it lands last, only a manual re-enumeration
catches them, and the only thing that prompts one is somebody remembering to. #1194 was dequeued by
hand to wait for them, which is rung zero.

The merge queue is what makes this check worth more than it looks. `script/lint` runs in every
group build (`merge_group` in `.github/workflows/ci.yml`), and a group build is the one place the
rename and a late reference meet on the same tree. So a path check there fails whichever of the two
lands second, in either order, with no one remembering anything.

Three things this adds to the design above:

- **Derive the rooted directories from the tree when the check runs, and do not list them.** The
  list under "What it would check" names `script/` and not `scripts/`, `briefs/`, `.github/`,
  `packages/` or `tools/`. A hard-coded list is the one-spelling trap again. A rename's old spelling
  drops out of scope the moment the directory is gone, so the check goes quiet on exactly the paths a
  rename strands. Treat a path as rooted if its first segment is a top-level directory now, *or* was
  one in history (`git log --diff-filter=D --name-only` at the root, or a short list of retired roots,
  each with a reason). The second clause is what lets it see `scripts/`.
- **Not only markdown.** The rename's own enumeration found ten bare-string forms a path grep
  misses: `sys.path.insert(0, 'scripts')`, a `pathlib.Path('scripts')` glob, `os.path.join(...,
  "scripts")` in `script/names`, and `script/fatal-risks`' `PATHISH` alternation. It also found paths
  in shell comments and in `.cargo/config.toml`. The Python and shell forms fail at run time, and
  some only on a path nobody exercises. Comments and TOML never fail at all. So the check should read
  backticked paths in code comments as well as in `.md` files. The string forms can stay with the
  run-time failure they already have.
- The escape list's first entries are the rename's deliberate survivors. #1194 kept 32 mentions
  of `scripts/` on purpose: captured transcripts, passages whose subject is the `script`/`scripts`
  confusion, dated census rows, and the prose recording the rename. That set was worked out by hand
  three times, once per rebase, by diffing the enumeration against the previous pass. Holding it in
  an allow-list with a reason per entry is exactly what would have made the second and third passes
  mechanical.

## Index row

262 citations in `notes/` pointed at a crate, a file or a Rust path that had been renamed away, and
every gate in this repository passed them. `script/lint` check 4c verifies a markdown *link* target,
`script/citations` verifies that a `§N` or a `milestone N` resolves to the thing the author meant.
A path in backticks is neither, so `` `crates/fs_proto` `` was unfalsifiable prose for the two
weeks after §75 (directories under `design/` and `notes/` carry provenance in their own README)'s
naming pass renamed the crate. The check is one pass over every tracked markdown
file, asserting that a backticked path rooted in a directory that exists really exists, with an
allow-list carrying a reason per entry in the shape `xtask`'s `ABORTS_ACCEPTED` already uses.
Milestone 259 (sweep `notes/` for claims that stopped being true, because the gate cannot see them)
hit exactly four kinds of legitimate exception. Verbatim transcripts, deliberate past
tense about deleted code, a path named *because* it does not exist, and elided illustrative paths.
That is what makes the escape a readable list rather than a shrug. The trap is that a crate is
named three ways here, as a path, as a Rust path and as a bare word. The sweep's own first pass
matched one, reported itself clean and left 167 instances: a check that knows one spelling is worse
than none, because it retires the worry. What it would have caught, beyond the prose, is the
deletion of `crates/elf/tests/fuzz_seed.rs` by a commit about `p_paddr` that never mentioned it,
which left two documents describing a safeguard in the present tense for five days.
