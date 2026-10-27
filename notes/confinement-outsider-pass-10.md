# Confinement outsider pass 10 (2026-10-27 UTC)

*Name: provisional, minted 2026-10-27 (UTC) by milestone 936's lane.*

The tenth outsider pass at risk 7 (the confinement claim is false), milestone 936 (a tenth
outsider pass attacks the confinement claim). Reviewer: GLM 5.3 through opencode 2.0.29, on the
launcher from milestone 920 (an outsider pass is launched pinned and floor-checked), unchanged.

Status: **escape on a shipped path, fix mandatory.** calef ruled it on 2026-10-27 (UTC). The
escape sits on claim 11, where an alias defeats W^X. The lane's count is `booted 11 of 29, floor
short by 18`. The pass does not count toward criterion (c): a pass that finds an escape is not
clean, and it misses the floor.

## The ruling

calef ruled on 2026-10-27 (UTC): "We need the mandatory fix milestone." The fork put to him
offered shipped path or coverage gap, with both readings stated; the mandatory fix accepts the
shipped-path reading. The facts he ruled on sit below: the mechanism, its reachability through
ordinary capabilities, and the lane's independent reproduction on all three ISAs.

The lane proposed the fix milestone in
[`design/roadmap/proposals/a-map-into-refuses-a-frame-mapped-twice-with-different-flags.md`](../design/roadmap/proposals/a-map-into-refuses-a-frame-mapped-twice-with-different-flags.md);
the integrator mints its number, and the refusal's exact shape waits for calef. Until the fix
lands, row 11 of [`notes/confinement-claims.md`](confinement-claims.md) reads falsified a fourth
time, with the replayable patch beside the witness.

## The escape, stated first

`AddressSpace::MAP_INTO` never refuses a frame the space already maps. `MAP_RW` at one `va` and
`MAP_CODE` at another leave every page-table leaf honest while the two aliases make the physical
page both writable and executable: alias defeats W^X, claim 11. A builder holding `WRITE` on a
space and a `READ | WRITE` frame reaches it with ordinary capabilities; the shipped model grants
that rights combination, and nothing in the grant chain misbehaves. The per-leaf Kani proofs the
row cites survive; the claim as published, "no page is both writable and executable", does not.

The reviewer's witness is
`system_tests::user::tenth_pass_map_gate_tests::one_physical_page_is_never_writable_here_and_executable_there`,
opt-in under `NIFE_ESCAPE_WITNESS=1` so the red test stays out of the default suite. The reviewer
booted it red on aarch64 at its head. The lane replayed it red the same day on all three
architectures: aarch64 (writable at 0xffffffffe000, executable at 0xfffffffff000), riscv64
(0x3fffffe000, 0x3ffffff000), and x86_64 (0x7fffffffe000, 0x7ffffffff000). The alias lives in the
shared dispatcher, so one fix covers every ISA.

The falsification patch is
`system_tests/falsifications/user.tenth_pass_map_gate_tests.one_physical_page_is_never_writable_here_and_executable_there.patch`,
cut from the reviewer's escape commit against the pass's pin; it applies clean and carries the two
green boundary-edge tests with the witness. No fix exists yet, so the witness stands as the
failing test committed before any fix, as the block's Done-means asks.

## The launch, and the key ruling

From `launch.json`: the pin is `e5cf69437f689141a547557ebd9a5104ed7d14df`; the image is
`nife-outsider-pass` at digest `921f2f8776a142a5c04df6364e683f7f981dab714e942e0d31cb703f0a130457`.
The model pinned `zai-coding-plan/glm-5.3`; the model reported the same, read back before the
brief went out. The prompt's SHA-256 is
`7dfbf7bcf92df32202be92441e692f06fd540eeb80668e6214069fb0f6068b4e`; the session is
`ses_e8166c1f0ffeaIqR4kTo0YduvL`; the start time is 2026-10-27T18:19:27Z.

calef ruled an exception for the key on the launch day: the pass used the working `zai-coding-plan`
key from the host's auth store, not a key minted for the pass and revoked after, and he rotates it
once collection ends. The key's material appears in no log, no file and no record here; the launch
record names only the variable that carried it.

## The turn that ended early

The launcher died twice, both times lane-side, neither the reviewer's doing.

A harness restart on the host killed the attack container mid-turn, because the launcher ran as
the session's child. The reviewer's stream up to that point survived. The relaunch ran detached in
its own process group, resumed the same session, and the reviewer worked to its end.

The launcher then crashed after that turn: `helpers/launch_pass.py`'s `models()` parsed truncated
`session export` output with no exit-status check and died on a `JSONDecodeError`, so
`launch.json` still reads `attacking` though the session is done. A maintainer fix owns this:
check the exit status and fail soft, so a finished session can never stick.

The reviewer's turn ended before it wrote `results.md`, before its own count, and before the
completion round. What it produced: 17 boot logs, two commits on its branch (`616eb0437`, the
escape with the boundary tests; `07d1555d1`, five dispatcher attack modules), and the attack
stream.

## The refusal log

Empty, by mechanism. The launcher runs opencode with `--auto`, so the harness approves every tool
call itself, asks for no approval, and refuses nothing. The stream is the record of the whole
turn.

## The lane's reproduction

The lane reproduced every booted row itself, one ISA's QEMU at a time, in the same container
recipe with no key in the environment: the six attack modules on all three ISAs, plus the witness
on all three. That gives revocation coverage and session usurpation one more ISA each than the
reviewer reached before its container died. The lane re-ran one leg (consumption, aarch64) after a
flaky cargo archive error left no verdict, and counted nothing from a log without one.

## The count

`script/booted-rows`, over the lane's 21 reproduction logs against the pin:

```
row 1: no pass-written test
row 2: no pass-written test
row 3: no pass-written test
row 4: booted aarch64, riscv64, x86_64
row 5: booted aarch64, riscv64, x86_64
row 6: booted aarch64, riscv64, x86_64
row 7: booted aarch64, riscv64, x86_64
row 8: booted aarch64, riscv64, x86_64
row 9: booted aarch64, riscv64, x86_64
row 10: booted aarch64, riscv64, x86_64
row 11: booted aarch64, riscv64, x86_64
row 12: host (exempt)
row 13: host (exempt)
row 14: host (exempt)
row 15: host (exempt)
row 16: host (exempt)
row 17: host (exempt)
row 18: host (exempt)
row 19: no pass-written test
row 20: no pass-written test
row 21: no pass-written test
row 22: no pass-written test
row 23: booted aarch64, riscv64, x86_64
row 24: no pass-written test
row 25: no pass-written test
row 26: booted aarch64, riscv64, x86_64
row 27: no pass-written test
row 28: no pass-written test
row 29: no pass-written test
row 30: booted aarch64, riscv64, x86_64
row 31: no pass-written test
row 32: no pass-written test
row 33: no pass-written test
row 34: no pass-written test
row 35: no pass-written test
row 36: no pass-written test
booted 11 of 29, floor short by 18
```

Rows 4 to 11, 23, 26 and 30 booted, each on all three ISAs. Rows 12 to 18 are host-exempt. The
reviewer died before it could widen its attack, so 18 bootable rows have no pass-written test.
The brief's floor block, which governs, reads 29 bootable of 36; this block's prose still said 28
of 35, and the same pull request corrects it.

The lane did not run the completion round. Its purpose lifts a below-floor clean pass over the
floor, and no round can make this pass clean after an escape; calef's ruling directs the
fix-first cycle instead.

## What the pass leaves

- The mandatory fix milestone: the proposal above, waiting for a number and for the refusal rule.
- A maintainer fix for the launcher: `models()` must check the exit status of `session export`
  and fail soft.
- The heavy-slot wait livelocks when `script/test` runs as a container's pid 1, because the
  slot's liveness check then always finds the holder alive; the scripts manual's BUGS records it.
- Risk 7's appendix cites this pass through the maintainer under §216 (fatal-risk facts are
  correctable, and verdicts are the architect's). Moving the color stays calef's.
- The key rotation, calef's, once this note merges.
