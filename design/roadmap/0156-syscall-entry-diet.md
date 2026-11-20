---
status: BUILT
raised: 2026-08-23
built: 2026-08-23
---
# 156. `syscall_entry`'s measured size is every method combined; extract the rest and ratchet both ways

Built 2026-08-23. Minted 2026-08-23, found while fixing milestone 126's `pmap` (`abi::aspace::LIST`
tripped `script/fastpath-footprint`'s 5% bound, +6.7% on riscv64). Extracting `LIST` alone into an
`#[inline(never)]` function fixed that one regression; measuring why turned up a bigger, pre-existing
gap this milestone tracks. Scope widened 2026-08-23 (calef, answering "do we have a mechanism to
ratchet down our performance benchmark" with "add it to milestone 156's scope"): the gate itself has
the same one-directional problem the extraction work is about to make concrete, and both belong in
one milestone since the second is what makes the first's gains actually stick.

## What was built

Both parts landed. Part one: `Untyped::{MAP,RETYPE_OBJ,RETYPE,SPLIT,DESTROY}`, `Frame::{MAP,REVOKE}`,
`Tcb::{CONFIGURE,CAP_INSERT}`, `Irq::{WAIT,ACK}`, `VirtualIo`'s four register methods, and
`Endpoint::REAP` all moved into `#[inline(never)]` functions, the same shape as `LIST`'s own fix.
`DeviceFrame::REVOKE` and `Tcb::START` were tried and reverted: the byte delta was negligible and not
worth carrying the diff. `Endpoint::{SEND,RECV,CALL}` and `Reply::REPLY`, the genuine IPC round trip,
stayed inlined on purpose. Part two: `script/fastpath-footprint`'s comparison now fails symmetrically
on `abs(delta) > TOL` in either direction, requiring `--save` to acknowledge a shrink exactly as it
already required one to justify growth.

**A correction found during landing, not during the lane's own work**. The lane's final merge of
`origin/main` (its own recorded commit) silently dropped `abi::aspace::LIST`'s match arm and its
`aspace_list` function during conflict resolution. That made its own `--save` measurement
(`syscall_entry` 1988 riscv64 / 3444 aarch64) look better than reality -- both numbers were measured
on a build missing a real, shipped feature. Restoring `aspace_list` during a later merge brought the
true, feature-complete numbers to 2210 / 3480. Still a genuine 24% (riscv64) and 16.5% (aarch64)
reduction from `main`'s actual pre-156 baseline (2914 / 4168), just not as large as the invalid
figures the lost merge had produced. `bench/fastpath-{aarch64,riscv64}.txt` reflect the corrected,
verified numbers.

## The second finding: the gate only ratchets in one direction

Checked directly, `script/fastpath-footprint`'s own comparison:

```python
delta = (got - ref) / ref
if delta > TOL:            # only fires on growth past +5%
    ... fail ...
elif abs(delta) > 0.001:
    print(f"    {key}: {delta*100:+.1f}% against baseline ({ref}), within bound")
```

A shrink never fails and never blocks. `--save` re-records the baseline, but nothing prompts anyone
to run it after an improvement. A real gain (this milestone's own extraction work, worth
hundreds of bytes) can land and sit unclaimed as slack in the recorded baseline indefinitely --
exactly the "somebody remembers" failure mode AGENTS.md's ladder names as rung zero. Worse than
cosmetic: a stale-loose baseline silently widens the tripwire's real tolerance. If `syscall_entry`
shrinks 20% and nobody re-saves, a later regression of +15% against the *current* code is actually
+15% against a number 20% too generous, and the gate still says "within bound."

**The fix is symmetry, not a new mechanism**: treat a shrink past the same 5% tolerance as an
equally reportable event as growth, requiring the same `--save` acknowledgment. Unlike growth, a
shrink needs no justification for *why*, just an explicit commit that locks in the tighter number,
so the baseline never drifts stale by more than one tolerance band in either direction.

## The finding

`kernel/src/syscall::invoke` -- the giant `match cap.object { ... match method { ... } }` handling
every capability method this kernel has -- has **no separate symbol in the compiled binary**.
Checked directly: `llvm-objdump` on `target/riscv64imac-unknown-none-elf/release/kernel` finds no
`invoke` symbol at all, only `dispatch`, at roughly 663 instructions (~3 KiB). The compiler folds
`invoke` wholesale into `dispatch`, because `dispatch` is `invoke`'s only call site and nothing marks
either `#[inline(never)]`.

`script/fastpath-footprint` measures `dispatch` **flat, with no closure** (its own header comment:
"a syscall traverses one path through the decoder, but the decoder's own bytes are on every syscall,
and its other arms are not on this path at all" -- the intent already states the property this
milestone restores). Because `invoke` is inlined, every method arm's compiled bytes count against
that flat measurement today, hot or not. `Untyped::SPLIT`/`DESTROY`/`RETYPE_OBJ` (133 source lines),
`Irq::bind`/`enable` (144 lines), `Frame::MAP` (63 lines), `Aspace::MAP_INTO`/`CONFIGURE` and the
rest, none of which run on an IPC round trip.

`Endpoint::SURVEY` (milestone 126's first stratum) never tripped this by accident, not by design: it
is two lines, one call into `sched::survey_supervised`, small enough nobody noticed the shared
problem. `abi::aspace::LIST` was the first arm big enough (a loop, two function calls) to cross the
5% line on its own commit. Every arm before it has been contributing bytes to a number labeled "the
IPC fastpath" the whole time, individually below threshold, collectively real.

## What this milestone does, part one: the extraction

Apply the extraction already proven on `LIST` (`kernel/src/syscall.rs`'s `aspace_list`, milestone
126) to every other administrative method arm still inlined in `invoke`: pull the body into its own
`#[inline(never)] fn`, called from the match arm, leaving only the arm's rights check and the call
site in `invoke` itself. Candidates, by source-line weight (measured 2026-08-23, before milestone
126 lands; re-measure after, since `LIST`'s own extraction changes this table):

| arm | lines | verbs |
|---|---|---|
| `Object::Endpoint` | 150 | `SEND`, `RECV`, `CALL` (**leave inlined -- this is the actual fastpath**), `SURVEY` (already a thin call, leave), `REAP` |
| `Object::Irq` | 144 | `bind`, `enable`, and whatever else this arm covers -- read before assuming |
| `Object::Untyped` | 133 | `SPLIT`, `DESTROY`, `RETYPE_OBJ` |
| `Object::Frame` | 63 | `MAP` |
| `Object::Tcb` | 57 | `CONFIGURE`, `START`, and others -- read before assuming |
| `Object::DeviceFrame` | 19 | probably too small to matter; measure before spending effort |
| `Object::Virtio` | 23 | same caveat |

**`Endpoint::SEND`/`RECV`/`CALL` and `Reply::REPLY` stay inlined.** They are the actual IPC round
trip this gate exists to protect; extracting them would defeat the milestone's own purpose. Read
each arm's real content before deciding it is a candidate -- the table above is source-line count,
not a verdict, and `Endpoint`'s 150 lines mix the fastpath in with `SURVEY`/`REAP`, which do not
belong in the hot measurement either.

## What this milestone does, part two: symmetric ratcheting

In `script/fastpath-footprint`'s Python comparison, change the one-sided `if delta > TOL: fail` into
a check on `abs(delta) > TOL` for both directions, with distinct messages. Growth keeps its existing
"shrink it, or re-record with --save and say why"; a shrink past tolerance prints "this is smaller
than the recorded baseline by more than the tolerance band -- re-record with --save to lock in the
tighter bound" and also fails, on the same reasoning growth does. An inaccurate baseline in
either direction is a gate that is no longer measuring what it claims to. No `--save`-side change
needed; the flag already does the right thing once something calls it.

## Verification, the same shape as `LIST`'s fix

Per arm extracted: `script/fastpath-footprint` before and after, on both `aarch64` and `riscv64`,
confirming the number drops and nothing else regresses. `script/lint` and the full `script/test`
suite after all extractions, since this touches the kernel's syscall boundary. **Do not `--save` a
new baseline until every planned extraction is done and measured together** -- saving mid-milestone
would bake in a smaller-but-still-inflated number and hide how much headroom the full extraction
actually recovers. Once the symmetric check is in place and every extraction is measured, the final
`--save` is what the new check itself will now require rather than merely suggest.

## What this does not decide

Whether `Endpoint`'s `REAP` (and any other genuinely-cold arm inside the 150-line block) is worth
extracting on its own is a judgment call for whoever builds this, guided by the measured delta each
extraction actually buys. Not every rare arm is large enough to be worth the diff. The exact
tolerance for the shrink-side check (whether it should match growth's 5% exactly, or use a wider
band to avoid nagging on noise) is also left to measurement. Check how much run-to-run variance the
un-padded build actually has before picking a number narrower than what the data supports.

## Prior art

None needed outside this tree: `LIST`'s own fix (milestone 126, `kernel/src/syscall.rs`'s
`aspace_list`) is the pattern, already proven to work and already reviewed.

## Follow-on

- **Refused.** Extracting `DeviceFrame::REVOKE` and `Tcb::START`. Both were tried and reverted: the
  byte delta was negligible and not worth carrying the diff, which is the judgment call the "what
  this does not decide" section left to whoever built it.
- **Refused.** Extracting `Endpoint::SEND`, `RECV` and `CALL` or `Reply::REPLY`. They are the IPC
  round trip the footprint gate exists to protect, so pulling them out of `invoke` would improve the
  measured number by moving the thing being measured.
- **Refused.** A shrink-side tolerance narrower than growth's 5%. The symmetric check reuses the
  same band, because nothing has measured the un-padded build's run-to-run variance and a number
  tighter than the data supports produces a gate that nags rather than one that measures.
- **Recorded.** `design/roadmap/0156-syscall-entry-diet.md` records the correction found during
  landing: a lane's own merge silently dropped `abi::aspace::LIST`'s match arm during conflict
  resolution, so its `--save` measurement was taken on a build missing a shipped feature and looked
  better than reality. The corrected numbers are in `bench/fastpath-aarch64.txt` and its riscv64
  sibling.

## Index row

`invoke` has no separate symbol in the compiled binary; the compiler folds it wholesale into `dispatch`, so `script/fastpath-footprint`'s flat measurement has been counting every capability
method's bytes, not just the IPC round trip, since before milestone 126's `pmap` tripped it.
Extract the rest of the rare administrative arms into `#[inline(never)]` functions, the pattern
already proven on `abi::aspace::LIST`; and fix the gate's own one-sided check so a genuine
improvement is required to tighten the baseline, not just permitted to.
