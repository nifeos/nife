# Confinement outsider pass 10 (2026-10-27 UTC)

*Name: provisional, minted 2026-10-27 (UTC) by milestone 936's lane.*

The tenth outsider pass at risk 7 (the confinement claim is false), milestone 936 (a tenth
outsider pass attacks the confinement claim). Reviewer: GLM 5.3 through opencode 2.0.29, on the
launcher from milestone 920 (an outsider pass is launched pinned and floor-checked), unchanged.

Status: **escape on a shipped path, fix mandatory.** calef ruled it on 2026-10-27 (UTC). The
escape sits on claim 11, where an alias defeats W^X. Counted from the tests in this tree, the pass
reads `booted 2 of 29, floor short by 27`. The pass does not count toward criterion (c): a pass
that finds an escape is not clean, and it misses the floor.

## The ruling

On 2026-10-27 (UTC) calef ruled that the escape needs a fix milestone and that the fix is mandatory.
The fork put to him offered shipped path or coverage gap, with both readings stated; a mandatory fix
accepts the shipped-path reading. The facts he ruled on sit below: the mechanism, its reachability
through ordinary capabilities, and the lane's independent reproduction on all three ISAs.

The fix is milestone 969 (no frame is writable and executable in one address space),
[`design/roadmap/0969-no-frame-is-writable-and-executable-in-one-space.md`](../design/roadmap/0969-no-frame-is-writable-and-executable-in-one-space.md).
Row 11 of [`notes/confinement-claims.md`](confinement-claims.md) states what holds with the fix,
and its limit.

## The escape, stated first

`AddressSpace::MAP_INTO` never refuses a frame the space already maps. `MAP_RW` at one `va` and
`MAP_CODE` at another leave every page-table leaf honest while the two aliases make the physical
page both writable and executable: alias defeats W^X, claim 11. A builder holding `WRITE` on a
space and a `READ | WRITE` frame reaches it with ordinary capabilities; the shipped model grants
that rights combination, and nothing in the grant chain misbehaves. The per-leaf Kani proofs the
row cites survive; the claim as published, "no page is both writable and executable", does not.

The reviewer's witness is
`system_tests::user::tenth_pass_map_gate_tests::one_physical_page_is_never_writable_here_and_executable_there`,
committed in `system_tests/src/user/tenth_pass_map_gate_tests.rs` beside its two boundary tests,
and opt-in at the pass's head so the red test stayed out of the default suite. The
reviewer booted it red on aarch64 at its head. The lane replayed it red the same day on all three
architectures: aarch64 (writable at 0xffffffffe000, executable at 0xfffffffff000), riscv64
(0x3fffffe000, 0x3ffffff000), and x86_64 (0x7fffffffe000, 0x7ffffffff000). The alias lives in the
shared dispatcher, so one fix covers every ISA.

The falsification patch is
`system_tests/falsifications/user.tenth_pass_map_gate_tests.one_physical_page_is_never_writable_here_and_executable_there.patch`.
At the pass's head the patch turned the witness's opt-in on and replayed the escape as it stood,
red at the witness assertion on all three architectures, so the witness was the failing test
committed before any fix, as the block's Done-means asks. With the fix in, the witness runs in the
default suite and the patch removes the fix.

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
completion round. It produced 17 boot logs, the attack stream and two commits on its branch: the
escape with the boundary tests, and five dispatcher attack modules. Neither commit reached a
remote, and no copy of either survives. The escape module survives whole because the
falsification patch carried it, and this tree holds it. The tree holds none of the five dispatcher
modules. Of three more (`tenth_pass_authority_return_tests`, `tenth_pass_session_usurper_tests`
and `tenth_pass_revocation_coverage_tests`) only a one-paragraph doc comment survives each.
Recreating them from that little would put the lane's attacks in the reviewer's name, so the lane
did not.

## The refusal log

Empty, by mechanism. The launcher runs opencode with `--auto`, so the harness approves every tool
call itself, asks for no approval, and refuses nothing. The stream is the record of the whole
turn.

## The lane's reproduction

The lane replayed the committed module itself on 2026-10-27 (UTC), one ISA's QEMU at a time under
QEMU 11.1.2, with the falsification patch applied. On aarch64, riscv64 and x86_64 the witness went
red at its assertion and the two boundary tests stayed green. Those three logs are the count's
input.

## The count

`script/booted-rows`, over the lane's three replay logs of the committed module against the pin:

```
row 1: no pass-written test
row 2: no pass-written test
row 3: no pass-written test
row 4: no pass-written test
row 5: no pass-written test
row 6: no pass-written test
row 7: no pass-written test
row 8: no pass-written test
row 9: no pass-written test
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
row 23: no pass-written test
row 24: no pass-written test
row 25: no pass-written test
row 26: no pass-written test
row 27: no pass-written test
row 28: no pass-written test
row 29: no pass-written test
row 30: no pass-written test
row 31: no pass-written test
row 32: no pass-written test
row 33: no pass-written test
row 34: no pass-written test
row 35: no pass-written test
row 36: no pass-written test
booted 2 of 29, floor short by 27
```

Rows 10 and 11 booted, each on all three ISAs. Rows 12 to 18 are host-exempt. The lane's earlier
reproduction also booted rows 4 to 9, 23, 26 and 30, from the reviewer's other modules. The tree
holds neither those modules nor their logs, so the nine rows stay unverified and out of the count.

The lane did not run the completion round. That was the lane's own judgment, and no ruling waives
the round. The round exists to lift a below-floor clean pass over the floor, and this pass cannot be
clean after an escape. A round would spend the reviewer's tokens and a fresh key on a count that
cannot change the pass's standing. Whether that waiver stands is calef's call, and the pull request
asks it.

## What the pass leaves

- The mandatory fix milestone: the proposal above, waiting for a number and for the refusal rule.
- A maintainer fix for the launcher: `models()` must check the exit status of `session export`
  and fail soft.
- The heavy-slot wait livelocks when `script/test` runs as a container's pid 1, because the
  slot's liveness check then always finds the holder alive; the scripts manual's BUGS records it.
- Risk 7's appendix cites this pass through the maintainer under §216 (fatal-risk facts are
  correctable, and verdicts are the architect's). Moving the color stays calef's.
- The completion-round waiver above, put to calef on the pull request.
- Rows 4 to 9, 23, 26 and 30 have no committed pass-written test; the next pass's floor covers
  them again.
- The key rotation, calef's, once this note merges.
