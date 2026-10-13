---
status: BUILT
raised: 2026-09-13
built: 2026-09-13
---
# 281. `watch` holds exactly what `ps` holds, so it is nothing

calef, 2026-09-13: *"We can cut `ps` with the `watch` and simplify."* *(Title and filename
provisional: a title is a name and therefore calef's.)*

In brief. In a capability system the question "should these be one program or two" has an answer
that is not a matter of taste: two programs are two programs when they hold different authority.
These held the same authority, from the same named constants, and one was a loop of the other. That
argued for a flag. Following it one step further argued for neither.

There was never a gate on this one. calef decided the shape when he raised it and decided it again
when he cut it; what was still his at build time was the flag's spelling and the fate of
`crates/watch`, and the cut answered both by removing the things they named.

## The measurement that decides it, which outlives the conclusion

Both programs took exactly three slots, and not merely three of the same shape. The same
`grant_plan` constants:

| slot | `user/src/ps.rs` | `user/src/watch.rs` |
|---|---|---|
| report | `REPORT = 0` | `REPORT = 0` |
| the process domain | `grant_plan::DOMAIN_SLOT` | `grant_plan::DOMAIN_SLOT` |
| diagnostics | `grant_plan::DIAGNOSTICS_SLOT` | `grant_plan::DIAGNOSTICS_SLOT` |

The refresh needed no capability of its own, which is the fact that could have gone the other way
and did not. `wait_interval()` was a yield-spin over `monotonic_nanos()`, because this kernel has no
timed wait (`user/src/timetable.rs`'s module docs name the gap). Had the interval needed a clock
capability, a flag would mean `ps` always carries authority it usually does not use, and the separate
program would have been the least-authority answer. It did not, so it was not.

This paragraph is the durable part of this milestone and it is deliberately written to survive the
conclusion moving. Identical cspaces from the same named constants, where one program is the
other's loop, is not a boundary; it is a split with nothing behind it. That is a reusable test for
"one program or two" in a capability system, and it is worth reaching for the next time the question
comes up, whichever way the answer goes.

## And the second program was the first one's loop

`watch.rs` called `ps::collect`, used `ps::Row` and `ps::MAX_ROWS`, followed the same refusal rule
and the same diagnostics-before-output ordering. Its `diag_slot()` fallback carried the comment
"`ps`'s own fallback, verbatim". Verbatim duplication across two binaries is the shape rule 7
exists to prevent, arrived at from the other direction: not a `#[path]` module, but a copy.

## Why a flag became a deletion

The flag was the right call on the authority argument alone, and it is still the right call on that
argument. What changed is that the argument is not the only one.

Follow it one step further and the flag's entire content is a busy-wait over a table of two columns
that barely changes. `abi::survey` carries a tid and a state. Watching a tid and a state refresh
twice a second is not worth a spin loop, a ceiling constant, a clamp function, an escape-sequence
prefix and a line of help text. Folding it into `ps` bought a simplification; deleting it bought more
of the same simplification and cost nothing anyone was using.

What makes that safe to do rather than merely tempting is milestone 282 (a thread's CPU time, and the `top` it makes possible), minted and decided the
same day (`design/decisions/0150-per-thread-cpu-accounting.md`). It adds per-thread scheduled CPU time
as a `u64` on `Thread`, tick-sampled in `sched::on_tick()` and exposed as a fourth word on
`abi::rendezvous::SURVEY`. Once that lands there is something worth watching and something to rank
by, and the live view gets rebuilt properly as `top` rather than as a refresh flag on a static table.
Cutting now is not losing the feature; it is declining to carry a poor version of it across the
milestone that makes a good one possible.

## It was never in `watch`'s family

Upstream `watch` re-runs an arbitrary command. This one never could, and never would have: re-running
a named command needs a program to hold authority to spawn another program by name, which in this
system is the shell's own capability (`grant_plan::spawnproto`) and is granted to nothing the shell
spawns (an interruptible child is built with no capabilities in its cspace at all). So it redrew
the one thing it could already reach without that: the supervision domain it was spawned into, which
is `ps`'s listing.

That makes it a very thin member of `top`'s family wearing `watch`'s name, which is why the name was
wrong and why `top` is the right shape to rebuild under 282.

## The names, and why two unratified ones vanished without a ruling

Neither `watch` nor `crates/watch` was ever ratified, and that was deliberate. Both shipped
flagged provisional in milestone 126, on the argument that `watch` is what upstream `procps` calls
the tool (`dpkg -L procps` lists `/usr/bin/watch`), which the naming tenet calls the best name
available for a standard term a reader already knows. Both were flagged anyway because this program
was genuinely narrower than upstream's: one fixed built-in view, not an arbitrary command line.
calef declined to rule on either while this milestone might retire them. It did, which is the outcome
the provisional flag existed to keep open, and it is recorded here so a reader meeting two names that
disappeared unratified does not have to wonder whether somebody forgot.

The provenance blocks lived at the things themselves (milestone 115's shape), so they died with the
files. What they said is carried in `crates/grant_plan`'s `Prog` enum, at the variant's place, and
in notes/process-view.md's own section, because nobody reads a deleted file any more than they read
a branch.

## What it retires

One program, one `[[bin]]`, one archive entry in each of `xtask`'s two architecture manifests, one
typed command, one `Prog` variant and its wire id, two scripted swish-check lines, one crate, one
kernel test module, and two unratified names. `ps` and `crates/ps` are untouched: the whole point of
the reshaped milestone is that `ps` gains nothing, including the spin loop it would have gained from
the flag.

## The refusals

- Leave them as two programs. Refused because the only argument for it was least authority and
  the measurement above removes it. What is left is that a reader types `watch` rather than
  `ps --watch`, which is a preference about typing and not about the system.
- Fold it into `ps --watch N`. This was the milestone as minted, and it was built before it was
  refused: the working branch carried `ArgSpec::Named { flag }` so the manifest owned the spelling,
  the count riding the one argument register a spawn already carries. Refused because it makes `ps`
  carry a busy-wait for a view not worth watching, and because a flag spelling would have been a
  naming decision spent on something 282 replaces. The commits are on this lane's branch history if
  the shape is ever wanted back.
- Keep `watch` and delete the duplication into `crates/ps`. The smallest change, and it fixes the
  verbatim copy, but it leaves two binaries with identical cspaces, which is the thing that makes the
  split arbitrary.
- Renumber the `Prog` wire ids, rather than leave a hole at 10. Taken rather than refused, and
  worth saying: the ids are a wire format between the shell and init, but both are built from this
  tree and ship in one image, so there is no version skew to protect. A permanent hole with no reader
  would be the worse record.

## BUGS

- `abi::survey` has nothing to rank by, which is the real reason a live view of it was not worth
  carrying: a tid and a state, with tid order that is not creation order once a slot has been reused.
  A `--sort` would be honest only once there is something to sort by. Milestone 282 is exactly
  that, and `crates/ps`'s own `BUGS` names the same gap where a reader meets the listing.
- Nothing in an automated run types `ps` or `pgrep` any more, and one thing used to. The scripted
  swish-check session in `xtask/src/main.rs` typed `watch 3` and `caps watch 3`, and those lines went
  with the program. `ps` and `pgrep` were already reachable only from an interactive prompt, which
  `crates/ps`'s `BUGS` records; this does not widen that gap but it does remove the one neighboring
  line that would have caught a regression in the domain grant at a real prompt.
- `design/roadmap/0158-kernel-object-rename-build.md` is `BUILT` and keeps the old names: it is an
  account.

## Follow-on

- **Milestone 282.** The live view of a supervision domain gets rebuilt as `top`, once per-thread
  scheduled CPU time (DECISIONS §150) gives it something worth watching and something to rank by.
  That is where the redraw idea goes, with an accounting column under it instead of two static ones.
  Nothing needs handing over with it: `CSI 2J`/`CSI H` is four bytes and `video_terminal::Vt` already
  parses both for the line discipline's `^L`.
- **Done.** Pull request #1349 (milestone 126: procps, what is left) rewrote
  `design/roadmap/0126-who-else-is-running.md`, which had still described `watch` in the present
  tense. It now lists `watch` as built 2026-08-24 and cut 2026-09-13 by this milestone. Resolved
  2026-09-26 by the integrator on `maintainer/126-followups`.
- **Recorded.** `design/decisions/0139-cycle-counter-authority/trap-and-emulate.md` names one of this
  program's lines as a spin-yield site, and a developer lane may not edit a decision. It is in this block's own
  `BUGS`, for the integrator.

## Index row

calef, 2026-09-13: *"We can cut `ps` with the `watch` and simplify."* The measurement that decides
it: `ps` and `watch` held the same three slots from
the same `grant_plan` constants, and in a capability system two programs are two programs when
they hold different authority, so the split was arbitrary. The refresh needed no capability of its
own either, the interval being a yield-spin over the ambient monotonic counter. That argued for a
flag; one step further and the flag's whole content was a busy-wait over a table of two columns
that barely changes, so it was deleted instead. Safe rather than merely tempting because milestone
282 (DECISIONS §150) adds per-thread CPU time to `abi::rendezvous::SURVEY`, after which the live
view is rebuilt properly as `top`. Retired two unratified names calef deliberately never ruled.
