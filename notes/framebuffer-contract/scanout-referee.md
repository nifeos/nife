# The host-side scanout referee: how it is built, and a false lead

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes, after the parent's own
word for the check.*

An appendix to [notes/framebuffer-contract.md](../framebuffer-contract.md), moved there on
2026-10-11 (UTC) under §212 (a prose budget). It records what makes the scanout check a real check,
the constraints found building it, and the 2026-08-26 investigation of a resize bug that did not
exist.

## What makes it a real check

Three details make this a real check rather than a comforting one:

- It runs inside the existing test run, not a second boot. The suite is minutes long per ISA. The
  pattern stays on the scanout from the display test until QEMU exits, so there is nothing to
  synchronize with the guest. No extra QEMU, no marker protocol.
- The geometry is part of the assertion. QEMU's console defaults to 640x480 and `SET_SCANOUT`
  resizes it. So a dump that is 128x64 is itself evidence our scanout rectangle reached the device.
  A scanout never set fails on size before a single pixel is compared.
- The checker has a negative control (`cargo test -p xtask`). It must reject a black scanout and a
  red/blue-swapped one. The swap is what a wrong virtio-gpu format code produces, and exactly what
  the in-guest test cannot see. It must also reject a one-row shift, a single wrong pixel, and the
  default console size. A checker that accepted anything would report success on every run, which
  is worse than no check.

Ordering is load-bearing and deliberately fail-loud. The confinement test resets the device (which
destroys the scanout), so it must run before the pixel test. That is why it is named
`a_backing_outside_the_grant_is_refused_by_the_iommu` rather than `the_iommu_...`, and the reason is
in its doc comment. If the order ever changes, no dump matches and the scanout check fails; nothing
is silently waved through.

Two practical constraints were found while building it, recorded so they are not rediscovered. The
unix socket path must stay under 104 bytes. A worktree checkout plus `target/` gets close, so the
socket lives in `/tmp` while the PPM goes under `target/`. And `socat` is not installed here, so the
monitor client is xtask itself over `std::os::unix::net::UnixStream`.

## A false lead: the resize bug that was a guest hang

- RESOLVED 2026-08-26, in the same lane, a few hours after it was first written below. The
  paragraph that follows was written mid-investigation and its conclusion was wrong: there is no
  QEMU-side resize bug. Root cause was the guest driver, not the host. `components/src/gpu_driver.rs`,
  `fixtures/src/painter.rs` and `components/src/display_terminal.rs` still looped over per-page
  capability slots (`SLOT + k` for `k` in `0..N`). Increment one's move to a single run capability
  (DECISIONS §102 (a Frame names a run of pages)) had already removed those slots. The second
  iteration always failed with
  `NoSuchSlot`, and the driver called `die()` and exited *before* sending the "UP" report anything
  downstream was waiting on. Whatever waited on that report hung, and the hang looked exactly like a
  stuck `screendump`. Every diagnostic below (the qtree check, `edid=off`, an explicit
  `xres=1280,yres=720`) was chasing a symptom that had nothing to do with the display device. The
  guest kernel test suite had genuinely hung and exited on the watchdog. `display_tests::` never
  appeared in the log at all, which the xtask summary's static pass/fail prose did not make obvious.
  After replacing the three per-page loops with single `map_page_frame` calls, the full suite passed
  clean on both architectures. That included the host-side scanout referee reading back 1280x720
  correctly. The investigation below is kept as the record of a false lead, with its long sentences
  split when it moved here on 2026-10-11 (UTC). It is a real account of what was checked and found
  (the qtree state, the two-witness digest agreement, the ruled-out fixes). And the project's own
  convention is to correct the record rather than delete the wrong turn. Read every claim below that
  a QEMU resize bug exists as withdrawn by this paragraph.

- The host-side scanout check does not currently confirm a grown scanout, on aarch64, under QEMU
  11.0.2. This was found 2026-08-26, in milestone 142 (a text display good enough that people use
  it) increment 1, growing the surface from 128x64 to
  1280x720 per DECISIONS §102. `screendump` consistently returns a 640x480 PPM for the whole
  suite, never the requested 1280x720. That holds even after the guest driver has completed
  `SET_SCANOUT` many times across several real-device tests. This is isolated to the *host's* view,
  not the guest's:
  - The driver never reports failure. `RESOURCE_CREATE_2D`, `RESOURCE_ATTACH_BACKING` and
    `SET_SCANOUT` all return their success codes (none of `E_CREATE_2D`/`E_ATTACH_BACKING`/
    `E_SET_SCANOUT` fire). `GET_DISPLAY_INFO` reports room for the requested size before any
    of this runs.
  - `info qtree` on a live monitor confirms the device model itself holds `xres = 1280`,
    `yres = 720`. That matches an explicit `-device virtio-gpu-pci,...,xres=1280,yres=720`, tried as
    a fix and reverted. It changed nothing, since 1280x800 was already the device's own default and
    evidently was never the constraint.
  - The kernel's own two independent witnesses agree the correct pixels reached the correct frames.
    They are the client's digest after the flush and the driver's digest after the device reports
    the transfer complete, both against a value the kernel computed itself. The pixels are right;
    QEMU's own console surface, the thing `screendump` reads, does not appear to resize with them.
  - `edid=off` (tried as a second fix, on the theory that EDID negotiation might bound the console
    to a preferred mode) also changed nothing.
  - Not root-caused further. This is QEMU-internal display-refresh behavior (`screendump` calling
    into `dpy_gfx_replace_surface`/`qemu_console_resize`, or not). It is not something the guest
    driver's protocol correctness can influence from where this was investigated.
  - The practical effect: `cargo xtask`'s scanout referee cannot currently confirm the resized
    scanout. The referee is the third-party check that a real device, not just our own frames, shows
    the picture. So it is graded on the guest's own two-witness proof alone for this milestone's
    increment. That proof is real and independent of the driver grading itself. But it is one rung
    short of the three-party proof this contract otherwise gets, until this is root-caused or a
    different host-side probe replaces `screendump`. A different QEMU version is the first thing
    worth trying.
  - This paragraph is wrong; see the RESOLVED entry above it.

## A dump refused at the door: 2026-10-12

The flake: the scanout check failed while every guest test passed, once in a local `--hvf` run on
2026-10-12 and once in eleven local riscv64 runs on 2026-10-13. CI never showed it. Of 654 failed
test and cpu-matrix jobs from 2026-08-17 to 2026-10-13, 31 logged a failed scanout check, and every
one followed an earlier stop (a guest panic, or a build that never booted).

**The question:** does a poll during a picture's three-second hold reliably produce a dump?

**What the referee did.** Each 100 ms poll connected to the monitor, wrote `sendkey`, and hung up,
then connected again, wrote `screendump`, slept 150 ms and hung up, never reading a reply. QEMU's
monitor socket serves one client and listens with a backlog of one. A connection made while the
previous one is still waiting to be accepted is refused on macOS, and `screendump` treated a refusal
as "try again next poll".

**Measured.** Against an idle QEMU, a connection opened just after a `sendkey` connection was
refused 9 times in 10. Traced in real suites, one line per poll (the patch was not kept).

| run | polls that took a dump | longest run of refused polls |
|---|---|---|
| riscv64 TCG, another lane's QEMUs on the host (load 14 to 19) | 54 of 260 (21%) | 25 polls, 2.6 s |
| riscv64 TCG, four spinners | 116 of 268, 121 of 275 | 8 polls, 0.8 s |
| aarch64 TCG, four spinners | 87 of 118 | 4 polls, 0.4 s |
| aarch64 HVF, four spinners | 19 of 58 | 7 polls, 0.7 s |

A 2.6 s blind stretch against a 3 s hold is the miss. The refusal rate tracks how quickly QEMU's
main loop accepts, which is why a neighboring emulator mattered more than the spinners did. None of
these five runs went red; the miss itself was not reproduced, its mechanism was. CI runs on Linux,
where a connect to a full unix-socket backlog waits rather than failing, which fits zero CI sightings
but was not tested here.

**The fix.** `scanout::Monitor` holds one connection for the whole run and reads up to QEMU's
`(qemu) ` prompt after every command. QEMU prints the prompt only when the command has finished, and
for `screendump` the file is whole by then (50 of 50 dumps measured). No connection waits in the
backlog, and the fixed sleep is gone. The free `sendkey` and `screendump` that `screen.rs`,
`uefi.rs` and swish-check use go through the same type.

**After.** Under the same four spinners every poll took a dump: riscv64 418, 411 and 445 dumps;
aarch64 233; HVF 80 and 80. All green. One more aarch64 run went red on a guest panic
(`an_installed_space_stays_installed_across_a_timer_tick`, the panic CI showed the same day). It is the new report's first
secondary: the composed screen lists 17 dumps graded, and the text and pattern say "never looked
for" rather than the old "no screendump was ever taken". A controlled loop against a paused QEMU,
100 polls each: the old poll took 35 and 48 dumps (idle host, eight spinners), the new one 100 and
100.

**Host tests.** `a_screendump_returns_only_once_the_file_is_written` (falsified: the old
write-and-sleep returns before the file exists), `the_monitor_reply_is_read_past_the_line_editors_echo`
on QEMU 11.1.2's own bytes, and `an_unmatched_picture_says_whether_it_was_ever_looked_for`.
