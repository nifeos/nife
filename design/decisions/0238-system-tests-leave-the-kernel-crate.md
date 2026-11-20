---
status: DECIDED
raised: 2026-09-27
decided: 2026-09-27
ratified_by: calef
---

# 238. The system tests leave the kernel crate as a second image, and a new test defaults to userspace

*Section number provisional until the merge queue lands it.*

Raised on pull request #1392, milestone 609 (the system tests leave the kernel crate). It asked
where the 61 `cfg(test)`-only files (20,584 lines) in `kernel/src/user/` go once they leave the
kernel crate proper.

Recommended: option A, a new system-test image crate (provisional name `system_tests`). It links
the kernel as a library and takes those files unchanged. Other options (stop at the edges, port
every test to userspace, a kernel feature with no second crate) are in
`notes/system-tests-and-the-kernel-crate.md`, with why each lost.

## The ruling

calef ruled at 2026-09-27T06:40Z:

1. Option A. The system tests move unchanged into a system-test image crate (provisional name
   `system_tests`) that links the kernel as a library. `script/test` runs both images per
   architecture leg.
2. A rule going forward: a new service test is a userspace program, unless it has to observe
   kernel internals. A test that does has to say which internals, and why.
3. Existing tests that touch no kernel internals migrate to userspace over time.

## Would porting win at equal cost?

calef asked whether porting every test to userspace, the option rule 2's converse would suggest,
would win if it cost the same as option A. It would not, even at equal cost. 1,414 references in
those files observe kernel state no system call exposes. Porting would either drop those observations,
or widen the system call surface that §10 (process model: capability-based, microkernel) bounds, just
to make them visible.

Rule 2 exists because that answer only holds for the *existing* tests. They were written against
kernel internals because nothing else was available. A new test that does not need kernel
internals is already the better kind, which is why it defaults to userspace from here on.

## Built

Built in #1404 (open at the time of this section, but the move is complete there). The kernel
becomes a library plus a thin binary. The 64 test-only files that existed by that point move
byte-for-byte into `system_tests/src/user/`. A `system_tests` kernel feature makes the boot a test
boot. A kernel binary built together with the `system_tests` feature fails to link on purpose, so
cargo cannot unify the feature into a shippable kernel.

Provisional names: the crate `system_tests`, the kernel feature `system_tests`, the facade
`system_test_access`, the hook `system_tests_main`.

## Reversibility

The crate split is mechanical, a rename plus a feature gate, and cheap to undo while the crate is
new. Rule 2 binds future lanes' judgment rather than any wire format, so it is cheap to amend but
not free to ignore. A test written against kernel internals today because it was convenient,
rather than because it needs to be, is the exact failure mode the rule exists to stop.
