---
status: BUILT
raised: 2026-09-27
built: 2026-09-27
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 609. The system tests leave the kernel crate

Raised 2026-09-27 (UTC), when calef took proposal P2 of pull request #1389 (the packages design).
The kernel should be buildable and releasable on its own. A base-service change should not need a
kernel change. calef chose the shape at 2026-09-27T06:40Z. The suite moves unchanged into a
system-test image crate that links the kernel as a library. The measurements, the options, what
was built and the migration backlog are in
[notes/system-tests-and-the-kernel-crate.md](../../notes/system-tests-and-the-kernel-crate.md).
Number, slug, the crate name `system_tests`, the kernel feature of the same name and
`NIFE_TEST_FILTER_ACROSS_IMAGES` are all provisional.

## Index row

The kernel crate was the kernel, the integrator and the whole-system test harness at once. Every
kernel build compiled service and fixture code, and every service test lived in the kernel. The 66
test-only files, 22,824 lines, now boot as their own image from `system_tests/`. That image links
the kernel as a library. The kernel crate links no service or fixture crate, and `script/lint`
check 13 fails if one creeps back.

## Built

- Pull request #1392: the kernel's test-only and fixture dependencies left its normal graph. Check
  13 (number provisional) holds it. Planting `ps` back into `[dependencies]` fails the lint.
- Pull request #1404: the kernel is a library plus a thin binary. The 66 `cfg(test)`-only files
  moved byte for byte to `system_tests/src/user/`. A kernel `system_tests` feature makes the boot a
  test boot and hands over to the suite. The kernel's modules stay private, reached through a
  facade only that feature compiles. `script/test` runs both images on every leg. So do the HVF leg
  and `uefi-test`. `--test` counts its selection across the two images. Every gate that read the
  kernel's test image now reads both. Fourteen falsification records moved with their tests.
- calef's rule for new tests is written where `script/test` is documented, in notes/scripts.md. A
  new service test is a userspace program unless it must observe kernel internals. One that does
  says which, and why.

## Follow-on

- **Milestone 697.** Milestone 697 (the trust root leaves the kernel binary). The trust root leaves the kernel binary (calef, 2026-09-27), which is what still
  makes a kernel build pack the archive first:
  `design/roadmap/0697-the-trust-root-leaves-the-kernel-binary.md`.
- **Milestone 690.** Milestone 690 (the harness-only system tests move to userspace). The three system-test files that observe nothing in the kernel but the harness move
  to userspace, calef's third rule:
  `design/roadmap/0690-harness-only-system-tests-move-to-userspace.md`.
- **Recorded.** Five service crates are still kernel dependencies because the kernel uses them for
  real (`calendar`, `jh7110_entropy`, `video_terminal`, `block_roster_contract`,
  `non_volatile_memory_express`); whether each is a service at all is pull request #1389's division
  question. The BUGS section of notes/system-tests-and-the-kernel-crate.md carries it.
- **Done.** `system_tests` is its own package, `system-tests`, of kind `test`, in
  `packages/system-tests.package.toml`; the package name and kind are provisional.
