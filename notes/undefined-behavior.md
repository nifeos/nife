# Dynamic undefined-behavior checking (Miri)

Milestone 79. `script/undefined-behavior-check` runs `cargo miri test` over the host-testable workspace members, the
same crate selection as `script/test`'s host leg. Weekly in CI (`.github/workflows/undefined-behavior-check.yml`) plus
on demand; not part of `script/test` or `script/ci-build`'s table.

## What Miri checks that nothing else here does

Miri is an interpreter for Rust's mid-level IR that enforces the language's dynamic rules while the
tests run. They are aliasing (no `&mut` that aliases anything live) and pointer provenance (a
pointer is only good for the allocation it was derived from, and an integer-minted pointer only for
allocations whose provenance was exposed). Then uninitialized reads, use-after-free, out-of-bounds,
invalid values,
and leaks at process exit.

The rest of the tree's analysis surface cannot see that class, and Miri is its third leg, after the
proofs and the fuzzers ([interleaving.md](interleaving.md) is the fourth). Kani proves the properties it is
asked about; the fuzzers see crashes and hangs; clippy sees shapes; the type system stops at every
`unsafe` block. There are 224 `unsafe` occurrences under `crates/`, concentrated in `ipc`,
`intrusive_fifo`, `virtio`, and `paging`, and an aliasing bug in one of them passes every
existing gate while being real UB on every target. Miri is the one tool here whose whole job is
that class, and the first full run proved the point: it found two genuine UB defects in
`user_mode_heap`, both invisible to 60 passing native tests.

The complement matters too. Miri only judges the paths the tests execute: it is a dynamic checker,
not a proof, so an `unsafe` block no test reaches is as invisible to it as it was before. Coverage
(`script/coverage`, the 80% floor) is what keeps "the paths the tests execute" an honest
approximation of "the paths".

## The first full run (2026-08-03, nightly-2026-08-03)

Every host-testable workspace member (`--workspace` minus the three bare-metal crates), 41 packages.
After triage, the front door (`script/undefined-behavior-check`, one `--workspace` invocation, 95 test binaries) runs
green in 27 minutes of wall time on the dev machine with a warm compile cache. **That figure is
2026-08-03's and no longer describes the run** (milestone 238): the job was red from 2026-08-10 and
died in under two minutes, so nothing measured it again while `serial_console` and `compositor` grew
expensive underneath. See the section on what three weeks of red hid.
The sequential per-crate triage sweep took about 31 minutes, of which
five packages are 79% (`xtask` 473 s, `cred` 452 s, `measured_boot` 334 s, `coremark` 190 s, `gpt`
127 s); 30 of the 41 finish in under 10 s. The interpreter tax measured about three orders of
magnitude where it was visible. (`network_time_protocol`'s 10^9-value sweep: 0.6 s native, a
projected day-plus interpreted. `calendar`'s 315,000 round trips: about a second native, still
running at 11 minutes
when it was killed and sampled instead).

### Findings, each with its verdict

**1. `user_mode_heap::insert_free`: write through an invalidated borrow. Real UB, fixed.**
The free-list insertion walked the list through a raw `link` pointer derived from `&mut self.head`,
then took `&mut self.head` a *second* time for the predecessor check. A fresh `&mut` is a fresh
unique borrow: it invalidated the tag `link` carried, and the head-insertion store through `link`
was a write through a dead borrow. Every compiler we ran emitted the store anyway, which is why 60
native tests passed; it was still UB, licensed to break on any toolchain bump, in the allocator
under every userspace program. Fix: take the head link once and compare pointers instead of
re-borrowing. This is the exact class the milestone was run for, found on its first pass.

**2. `user_mode_heap`: integer-minted pointers with no exposed provenance. Real UB, fixed.**
The allocator's arithmetic is integer arithmetic (`alloc` mints a tail block at `aligned + size`;
`insert_free` recovers a predecessor node from a link address). A coalesced block can span two
separately donated regions, so no single donated pointer could carry provenance for it even in
principle. Rust's rule for that shape is expose-and-reclaim, and the code was using `addr()`, which
deliberately does not expose. The first `alloc` to carve a block coalesced across a donation
boundary was a write with no exposed tag. Fix: `insert_free` now calls `expose_provenance()` on
every incoming range. Same instruction at runtime; the difference is that the optimizer is told the
escape exists.

**3. `paging` test harnesses: leaked frames. Test bug, fixed.**
`domain.rs`'s synthetic-frame pool and `tests/mapping.rs`'s pretend frame allocator both leaked
their 4 KiB tables on purpose ("a test process is about to exit anyway"), 16 and dozens of reports
respectively. Deliberate or not, a suite that fails the leak check on purpose teaches everyone to
ignore the gate. Both harnesses now free what they allocate through a `Drop` guard per test,
declared first so it drops after the mapper that reads the tables.

**And then it came back, which is the part worth learning from** (milestone 310). The guard above
was opt-in: each test had to remember to bind one. A `domain.rs` test written after that fix did
not, five frames leaked, and the weekly workflow was red on every scheduled run from 2026-08-11 to
2026-09-17. Nobody was able to see it, because a scheduled job's red is an entry in the Actions
tab with no badge. So the fix here was not the missing line, which would have restored the same
defect for the next author. `domain.rs`'s pool now owns the allocation: `frame` and `frame_at` are
methods on `FramePool`, the free functions are gone, and a test that allocates without holding a
pool does not compile. That is AGENTS.md's ladder moving from rung four to rung one, on a
mechanism that had already been given one chance at rung four and lost it.

`tests/mapping.rs`'s `TableGuard` is still the opt-in shape and every test in that file currently
binds one. It is a latent instance of exactly the same defect; see the `BUGS` note at `TableGuard`
and milestone 310's follow-on.

**4. A `glob` sweep failure in the triage logs: an artifact, not a finding.** One intermediate run
recorded `greedy_agrees_with_exhaustive_search_over_every_short_pattern` failing with
`checked = 43,720`; that run caught the file mid-edit, with the Miri stride applied but the
completeness pin not yet updated. The finished edit passes under Miri and natively.

**Also seen, deliberately left:** integer-to-pointer cast *warnings* in `paging`'s tests, where
`phys_to_ptr` is an identity cast by design (the tests' documented trick is that host addresses
stand in for physical ones). They are warnings about a pattern the harness is honest about, not
errors.

## "Miri-clean" means the sampled paths

An interpreter runs roughly three orders of magnitude slower than the silicon. The exhaustive
suites cannot run under it as-is, and each one gates itself down under `cfg(miri)` with the reason
written next to the test:

| Site | Native | Under Miri |
|---|---|---|
| `network_time_protocol` `every_nanosecond_survives_the_round_trip` | all 10^9 nanoseconds, 0.6 s | strided sample (stride 999,983, prime) plus the edges |
| `calendar` `format_and_parse_round_trip_across_the_range` | ~315,000 round trips | ~300, stride widened 1000x |
| `glob` `greedy_agrees_with_exhaustive_search_over_every_short_pattern` | all 2,657,200 pattern/name pairs | every 61st pattern, 43,720 pairs, the completeness pin adjusted to the exact sample |
| `glob` `the_worst_case_over_the_proof_domain_is_what_the_unwind_bounds_are_set_from` | ~1.8M runs, pins the exact argmax | skipped: a sample that misses the argmax fails against correct code |
| `glob` pathological/quadratic bound tests | 100,000- and 2,000-byte names | 2,000 and 200; both assertions still run |
| `globally_unique_identifier_partition_table` header and entry-array corruption sweeps (`real_disks.rs`; two live, one already `#[ignore]`) | 260k+ parses, each re-CRCing 16 KiB | skipped; the clean-fixture tests walk the same paths |
| `globally_unique_identifier_partition_table` small-table sweep (`table.rs`) | 261,120 parses | skipped, same reason |
| `cred` store tests via `cheap()` | Argon2id at m=256 KiB, t=2 | Argon2's floor (m=8 KiB, t=1); same paths, fewer blocks. The known-answer vector tests keep their published costs |
| `cred` `an_unknown_identity_costs_what_a_known_one_costs` | 50 timed KDF runs | skipped: a wall-clock ratio under an interpreter measures Miri, not the KDF |
| `compositor` `a_damaged_composite_leaves_the_rest_of_the_screen_alone` | all 317,856 screen pixels, 20 ms | every 37th row plus every row the damage rectangle touches, 15,708 pixels, pinned exactly |
| `manual` `every_character_survives` | 547 markdown files off disk, 7.2 MB, 0.74 s | skipped: the corpus is on the filesystem, which Miri isolates; the nineteen other tests in that file drive the same renderer on in-memory input |

### The `manual` row is a different kind of skip, and worth telling apart

Every other row is a sampling decision: the test is too slow under an interpreter, so it runs a
smaller version of the same claim. The `manual` row is not that. That test does host I/O, and Miri's
isolation refuses `open` and `opendir` outright; there is no smaller version, only a decision about
`-Zmiri-disable-isolation`.

**It was refused, on measurement rather than principle** (milestone 238). `crates/documentation` has no
dependencies and no `unsafe`, so the rules Miri enforces cannot be broken by any line it would
interpret there. Against that, `every_character_survives` costs 0.74 seconds natively and had not
finished after 12 minutes under Miri with isolation off, and the flag is not per-test. It would
relax isolation for the whole workspace run, which is the reproducibility every other crate here is
getting for free.

These two are also the entire content of the three weeks the weekly workflow spent red. It
reported failure from 2026-08-10 to 2026-09-03 and never once for undefined behavior: first an
environment variable Miri does not forward, then a `read_dir` behind it, then five `serial_console`
tests behind that. Each was hidden by the one before because `cargo miri test` stops at the first. The
audit that found it (milestone 232) recorded the fix as one flag, in good faith; it was three
layers. See notes/check-inventory.md and `script/cadence-check`, which exists so the next dead
cadence is noticed in days rather than weeks.

So a green `script/undefined-behavior-check` certifies the memory rules on every path the sampled suite executes, and
does not restate the exhaustive claims; those stay native-only, in `script/test`. The skipped `globally_unique_identifier_partition_table`
sweeps lose nothing Miri-specific: what they add natively is completeness of the CRC argument,
which is not a memory property.

### Three weeks of red hid four separate costs, in a queue

Worth recording as a pattern rather than as four facts, because the shape recurs. `cargo miri test`
stops at the first failure, so a broken run reports exactly one problem however many it has. From
2026-08-10 the weekly job died inside two minutes on a missing environment variable, and milestone
232's audit read that message and reasonably estimated a one-flag fix. Behind it, in order, sat:
`read_dir` against isolation; five `serial_console` tests doing host I/O; five more in
`serial_console` that are wall-clock driven; `serial_console`'s 55-minute cost; and a `compositor`
test sweeping 317,856 pixels that ran 44 minutes without finishing.

None of those were regressions. They accumulated while the job was red, invisibly, because a job
that fails in two minutes never reaches the code that would take an hour. The 27-minute figure
this note and the workflow both quote is from 2026-08-03 and has not described the run since. The
general lesson is the one `script/cadence-check` exists for: a check that has been failing is not
merely not-checking, it is also not-measuring, and the bill accrues the whole time.

**`serial_console` is excluded from the run entirely, which is the fourth exclusion and the
expensive one** (milestone 238). It measured 3,307 seconds, 55 minutes, for its lib tests alone
on 2026-09-03, against roughly four minutes for the whole rest of the workspace, and it has no
dependencies and no `unsafe`. Ten of its forty-one tests could not run under the interpreter
regardless. Five reach the host filesystem, and five in `watch` are wall-clock driven. So a
15-second quiet timeout expires against interpreted time, and the watcher reports `Reached(Banner)`
where a real run reaches `Reached(Tour)`. That second family is exactly the `cred` timing row above.
The reasoning sits in `xtask`'s exclusion list and in the crate's own header, where somebody
pointing Miri at it by hand will meet it. It was never deliberately covered: it joined the workspace
after milestone 79 (Miri over the host crates). The weekly job had been red on an unrelated failure the whole time it existed,
so its cost surfaced only when milestone 238 (two scheduled checks have never once succeeded) cleared what was in front of it.

Two test surfaces stay out entirely, deliberately. `tools/redoxfs_host` and `redoxfs_server` are their
own workspaces whose runtime is spent inside the vendored RedoxFS engine, and a finding in vendored
code lands in the vendor pin, not in a crate this tree can fix (vendor/README.md). They keep their
native gates in `script/test`.

## Running it

```
script/undefined-behavior-check              # everything, what the weekly workflow runs
script/undefined-behavior-check -p globally_unique_identifier_partition_table   # one crate
script/undefined-behavior-check -p glob -- greedy   # any cargo-miri-test args pass through
```

The miri component rides the pinned toolchain: `script/bootstrap` installs it, and `script/undefined-behavior-check`
adds it itself on a machine bootstrapped before milestone 79.

## BUGS

- Miri judges only executed paths. An `unsafe` block without a test is not "Miri-clean", it is
  unvisited. The coverage floor is the guard on that gap, and it is a floor, not totality.
- `-Zmiri-strict-provenance` is not on. The roadmap block named it as a later ratchet.
- The weekly cadence means a regression can live on `main` for up to a week before CI sees it.
  `script/undefined-behavior-check -p <crate>` before pushing `unsafe` changes is a habit, not a gate; nothing
  enforces it.
- Miri interprets the host target (aarch64-apple-darwin locally, aarch64 Linux in CI). Endianness
  and pointer width match the kernel targets today, so nothing is lost, but a finding that depends
  on target-specific layout would be reported against the host's.
- The samples under `cfg(miri)` are hand-maintained twins of the native domains. The `glob`
  completeness pin (43,720) is the honest version of that risk: it is asserted, so a drifted
  sample fails loudly, but most of the other samples have no such pin and would shrink silently.
