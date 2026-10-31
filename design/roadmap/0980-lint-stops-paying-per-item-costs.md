---
status: PARTIAL
promoted_from: lint-stops-paying-per-item-costs
raised: 2026-10-29
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 980. script/lint stops paying per-item costs

Raised 2026-10-29 (UTC) by a session that measured `script/lint` at calef's request.
[notes/lint-cost-2026-10-18.md](../../notes/lint-cost-2026-10-18.md) ranked the fixes and changed
what a gate refuses. This proposal keeps every verdict and names the per-item costs: places where a
check repeats work once per file, per workflow or per corpus.

## What the run costs today

One run at `9b51d977b`, on patagonia, at load about 5, with another lane's lint already running.
That is the normal condition. The split is `helpers/lint-no-cargo.awk`'s, each section in a fresh
`sh`, timed to the millisecond. The cargo half tracks build state, so the table shows it both warm
and cold.

| half | sections | seconds |
|---|---:|---:|
| `--no-cargo`, the pre-push hook's mode | 88 | 118 |
| cargo sections, warm | 32 | 86 |
| cargo sections, cold | 32 | 184 |

A warm run costs about 205 s and a cold one about 300 s. The long tail is not the problem. Fifty-seven
sections of `--no-cargo` finish under a second and cost 12 seconds together. Eighteen cost two seconds
or more and hold 88 of the 118. A dozen costs are the whole run.

## The slow bits

In `--no-cargo`, with what each one is actually doing.

| seconds | section | the per-item cost |
|---:|---|---|
| 16.0 | every workflow records the run it was watched failing in | `observed_failing.py` runs `git log --follow` once per workflow, 29 times. Those calls cost 19.3 s alone; one whole-history pass costs 1.8 s |
| 9.8 | the heavy-job slots | `heavy-slot-selftest.sh` (7.1 s, with a deliberate `sleep 4.5`) and `verify-parallel-selftest.sh` (3.3 s) |
| 6.5 | the pre-push hook's contract | `lint-sections-selftest.sh` 4.9 s, the rest stub-git fixtures |
| 6.2 | the prose ratchet | 1.9 s selftest plus 5.2 s check: 2,736 documents, 60,913 sentences, 565,000 `re.sub` calls |
| 5.6 | the commit-msg hook | `lint-sections-selftest.sh` 4.4 s |
| 5.0 | names meet the current naming rules | `name_crossings.strip()` walks each `.rs` file a character at a time in Python: 3.6 million regex matches |
| 4.9 | spelling | 4.7 s is `shellcheck`, 0.5 s is `typos` |
| 4.5 | American spelling and metric units | `house_style.py` opens 5,340 files twice |
| 4.1 | citations | a grep-based check plus fourteen git calls |
| 3.7 | the outsider-pass floor checker | `launch_pass.py --selftest` |
| 3.0 | tests the suite cannot see, unsafe fn contracts | whole-tree `.rs` scans in an inline Python block |
| 2.9 | counted claims | `counted_claims.py`'s cross-checks |
| 2.8 | every Kani harness carries a falsification record | `script/falsifications --check` |
| 2.7 | `script/verify --parallel` | the same `verify-parallel-selftest.sh` the heavy-job slots section already ran |
| 2.7 | module-wide dead-code suppression | one `awk` per tracked `.rs` file, 1,631 processes |

In the cargo half, the lint ratchet is the one cost that does not move with build state: 23.2 s warm
and 26.2 s cold. It is a second clippy pass over every configuration. The rest are builds. Cold,
rustdoc costs 32.8 s over four `cargo doc` runs, clippy `user + user_mode_runtime` on x86_64 30.6 s,
and the uefi_loader kernel build 14.0 s. Warm, those fall to 3.2 s, 14.9 s and 2.9 s.

## Five mechanisms, not eighty-eight checks

1. Per-item git history, about 15 s. One bulk pass replaces 29 walks.
2. Independent walks of one corpus, about 30 s. Many helpers call `git ls-files` or a directory walk
   and re-read the same 22.6 MB of Markdown and 19.6 MB of Rust. Nine Python gates in this run do it,
   each alone.
3. Gate selftests, about 30 s, or a quarter of the run. This is the checks testing themselves, not
   the tree. Two sections run `verify-parallel-selftest.sh`.
4. Per-file subprocess loops, 3.2 s in one section. One interpreter beats 1,631.
5. Duplicated cargo work, 23 to 26 s. The lint ratchet re-lints what the clippy sections just linted.

## How the work splits

Each part is one lane, ordered by seconds saved per hour of work.

1. One history pass in `observed_failing.py`. Replace the 29 `git log --follow` calls with one
   `git log --diff-filter=A --name-only` pass over all history, and reuse it for every workflow. About
   15 s. No change to what the check refuses.
2. Stop running a selftest twice. Delete the duplicate `verify-parallel-selftest.sh` call from the
   `script/verify --parallel` section, and cut the fixed `sleep 4.5` in the slot selftest to what its
   assertions need. About 8 s.
3. One lexer pass in `name_crossings.py`. Memoize `strip`, `_without_attributes` and `encloser` per
   file, and replace the character-by-character scan with a compiled pattern. About 4 s.
4. One interpreter in the dead-code section. Read the `.rs` files in Python instead of spawning `awk`
   per file. About 3 s.
   Built: `helpers/dead_code_suppression.py` reads the tracked `.rs` files in one process and keeps
   the awk's rules. Its selftest plants a blanket allow, a split attribute and the one exemption. The
   section went from 10 s to under 1 s on a loaded Linux host.
5. One shared corpus index. A small helper that lists the tracked files, their text and their hashes
   once per run, so the text gates read it rather than each walking the tree. An estimate of 10 to
   15 s. This is the largest piece and the one to design carefully.
6. The cargo half. The lint ratchet is the one cost there that does not move with build state, so
   decide whether it can share the clippy sections' work and whether rustdoc's four invocations
   collapse into one. The builds are not worth touching.

Parts 1 through 4 change no verdict and can land alone. Part 5 needs a design note for the shared
index. Part 6 needs a decision about whether the lint ratchet can read the clippy sections' output.

## What done means

- With parts 1 to 4, `--no-cargo` falls under 90 s, and the shared index brings it near 75 s.
- Every section refuses what it refused before, held across planted failures by
  `helpers/lint-sections-selftest.sh`.
- The per-section CSV in `~/.local/share/nife/lint-section-times.csv` shows the fall, and a ceiling
  that only lowers holds the total.

## What holds the gain

- `helpers/lint-sections.sh` already writes one row per section per run, so a cost that regrows is
  visible without a lane looking for it.
- A wall-time ratchet on the `--no-cargo` total, in the shape of the lint ratchet and
  `helpers/ci_job_times.py`'s near-budget warning, fails a run that grows past its baseline. §254 (a
  gate prints what each item cost) already asks for the number; this holds it.

## Cost

Five text lanes plus one cargo lane. Nothing here changes a verdict, so no architect ruling
guards parts 1 through 4. Part 5 touches every text gate, so it wants a design note and a review
before it lands. The dead-code and naming rewrites each need their selftest to stay green, which is
the same bar every other check meets.

Reuse: `helpers/lint-sections.sh`'s timing rows, which already measure the result; the
`git ls-files` walk the text helpers already share in shape; and the ratchet shape (a baseline that
only falls) from the lint ratchet and the prose ratchet. This writes nothing new where one of these
fits.

## Follow-on

- **Recorded.** `helpers/trust_boundary.py --check` fails in a checkout whose `fs_server/` holds only
  a stale `target/` directory: it looks for `fs_server/Cargo.toml`, and the crate is `redoxfs_server`
  since `0c0ef2367`. A leftover local directory turns a gate red and says nothing true. This is a
  separate fix to that helper.

## Index row

A local `script/lint` run costs about 205 s warm and 300 s cold: 118 in `--no-cargo`, and 86 to 184 in
cargo. A dozen sections hold most of it, and each repeats work once per file, workflow or corpus. This
milestone replaces the per-item costs with one pass each, keeps every verdict, and holds the total
to a ceiling that only falls.
