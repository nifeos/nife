# radon, 2026-11-17 (UTC): the 1062 and storage bench session

Two sessions at radon (the VisionFive 2) the same day. Issue #3509 tracks them. The first covered four things:

- milestone 1062 (the device control planes leave the kernel), steps 2, 3 and 5: the Ethernet
  control plane (#3498) and the JH7110 power-management-chip route (#3499);
- the storage bench (#3488) of milestone 53 (the board's own peripherals: network and storage on real silicon);
- milestone 1035 (a crate names the version that ran on hardware), recorded in
  [README-1035.md](README-1035.md).

The second session, later that evening, added three logs: the network bench on current `main`
(issue #3699), the missing step 1 of the route's bench (#3499), and the proof of the fix in draft
pull request #3695 (issue #3694). They are described in
[the second session](#the-second-session-three-logs-added-later-that-evening).

Every boot printed
`nife: payload came from net`; `<host>` below is the TFTP host's LAN address, which the logs print.

## What the logs are

The copies went through the bench-session `tr` that keeps tab, newline, carriage return and
printable ASCII (`briefs/bench-session.md` step 6). It removed only control bytes the UART
emitted (a backspace, NUL and SOH), between 8 and 14 bytes per log, and no line. The three
second-session logs went through the same `tr`; their counts are in their section. Nothing else was
edited, so each log still holds the board's serial number, MAC addresses and the private LAN
addresses, as the earlier `bench/radon-*` logs do. The list below the table gives the SHA-256 of each unfiltered
capture. The `[drive: ...]` lines are the bench driver's own notes.

| file | ran at | shows |
|---|---|---|
| `network-bench-494.log` | `dd47a83e88` on `lane/1062-designware-ethernet-control-plane` | `LEASED-AND-MEASURED`, 154.033 Mbit/s |
| `network-bench-494-refused-stale-card.log` | same build | `MEASURED BOOT REFUSED` |
| `pmic-route-withheld-3499-step2.log` | `462ad3761d` plus `packaged_only` | the reboot hangs without the route |
| `storage-bench-read-488.log` | `9c394ab715` (`main`) | `READ-OK`, EL0 block server served |
| `storage-bench-read-488-refused-stale-card.log` | same build | read numbers valid, EL0 step refused |
| `storage-bench-scratch-488.log` | `9c394ab715` | scratch write verified, EL0 server thread killed |
| `1035-boot-1.log`, `1035-boot-2.log` | `9c394ab715` | milestone 1035, judge `PASS` |
| `network-bench-main-3699.log` | `cfe18db337` (`main`) | `LEASED-AND-MEASURED`, 139.399 Mbit/s |
| `pmic-route-step1-3499.log` | `462ad3761d` | the route resets the board; no card, so it stops at U-Boot |
| `storage-bench-scratch-3695-fix.log` | `3d004a7bcd` (#3695) | `WRITE-VERIFIED`, EL0 block server served, no `killed` |

Unfiltered SHA-256, in the table's row order (the 1035 pair is in README-1035.md):
`00a624d04700e9ef0e9e4e7d0f980183e3c10446664adb5d8675fed0e0ee9602`,
`3cbf50564dc470cfac2c883738dff18c45cbcaa113014d4ae05e179cdeb5e8d1`,
`09f13dca12b21741bb914fa857eceddafd92a0f1bf72099167325a41dd48f8fb`,
`29c7ec0a63e8988e86f317d682ae864295d9881720f6a9a6a80edac7b7c93519`,
`b2d7ac572246b9cafa3c485f902269e129f3bfd6ff2d091f5d91ea0ed50e17a6`,
`9337e5cb6c99c99dfb3e15a317b8d0bfb46ad431b57ec5e265bd530bd2658c02`.

A `storage-bench-scratch-driver-timed-out` capture (36 bytes: the driver's `timeout in state
autoboot` note, no board output) is not committed.

## Caveats that apply to every row

- The commits are the ones each run was built from. Only `network-bench-main-3699.log` is current
  `main`. The `1062` branches were merged with `main` before the first-session runs and are not
  `main`; the storage and 1035 runs are `9c394ab715`; the other two second-session logs are
  branch heads.
- The stale card script: the microSD card carries a `boot.scr.uimg` that must match the image on
  the TFTP host. Twice the card held an older one than the image (issue #3690). The kernel then
  refuses to hand the archive to the progenitor, printing `MEASURED BOOT REFUSED` with the two
  hashes. Those two logs are kept because they show the refusal firing, not because they prove the
  feature under test.
- Each row is one run on one board and one card. Nothing here is a comparison between builds or
  an estimate of variance.

## Network bench (`network-bench-494.log`)

Milestone 494 (a driver for the network card a PC actually has) names this bench. Built from `dd47a83e88`. The build flags were not saved with this set. By the precedent in
`bench/radon-2026-10-22/README-53.md` the build is
`script/board-image --tftp --server <host> --extra-features network_bench`; I have not verified
that this is what ran. The peer was `<host>:9494`.

The kernel brought the port up, read RGMII from the syscon, found the MAC core at version
`0x4152`, negotiated 1000 Mbit/s full duplex, leased an address by DHCP and received 104857600
bytes in 5.445 s: 154.033 Mbit/s, 27139 receive calls. The rate is radon's own `transfer` line, from the board's 4 MHz counter.

What it does not say: the peer was a laptop reaching the board through a USB ethernet adapter, and
this is one run. The figure is bounded by that peer and adapter and says nothing about the link's ceiling, and
the earlier 14.3 Mbit/s of `radon-2026-10-22/README-53.md` came from a different commit, peer and
adapter, so the two are not a speedup measurement.

`network-bench-494-refused-stale-card.log` is the run before it, on the same build, refused as
above. Its last line is the refusal, so it carries no rate.

## The power-management-chip route (issue #3499, `pmic-route-withheld-3499-step2.log`)

Step 2 of the route's bench: a throwaway worktree detached at `462ad3761d` with
`packaged_only = ["power_management_chip_route"]` added to `components/Cargo.toml`, built with
`script/board-image --tftp --server <host>` (features `board`). Editing
`packages/drivers.package.toml` does not withhold the program, because the board archive packs
every component binary. Images: `nife-vf2.img` SHA-256
`6daa0f2b12b0c100c7c4decc815c56f0061ecf47c67a382628393b0f91c2f6bc`, `nife-initial-ramdisk.img`
`321fb83c6dd55f651d978cfb3a785eeb00b5fc256debdbc84805ab5df3bd6c0a`.

The log has `reboot: JH7110: no PMIC route: this archive carries no route program`, then `reboot`
typed at the prompt, `SBI SRST system_reset, reset type 1 (cold reboot)`, and no firmware banner:
the console prints `i2c read: read reg 32 to` repeatedly and ends at `cannot read pmic power
register`. So in this run the SBI reset alone did not restart the board. The control run (step 1,
route present, board reset through the route) is `pmic-route-step1-3499.log`, below.

## The storage bench (milestone 53, issue #3488)

All three storage logs are `9c394ab715` with the same ramdisk (SHA-256
`cbb1ab870fe777e1bc0a915fa8f6d6b0e64b37004ef22997fee823b3b9340f9a`), the board's microSD slot 1
and one card (29818 MiB). The eMMC socket answers `NoCard(NoResponse(1))`; it holds no part.

| log | build | `nife-vf2.img` SHA-256 |
|---|---|---|
| `storage-bench-read-488.log` | `script/board-image --tftp --server <host> --extra-features storage_bench` | `ad6afb7c42d13e9ff16a6f1d5921c833c75b87a10acb67b5ae3a78baff2272fa` |
| `storage-bench-scratch-488.log` | `NIFE_STORAGE_BENCH_WRITE=scratch script/board-image --tftp --server <host> --extra-features storage_bench` | `3636341931ff1100b7b0f783a158efcaac0a504508693545c1c98b2820d05ce8` |

- `storage-bench-read-488.log`: a polled-FIFO read of 16384 blocks (8 MiB) took 771455 us, 10618
  KiB/s, and the same read by IDMAC took 762585 us, 10742 KiB/s, with the first 64 KiB matching. The
  EL0 block server was started and `Served`. Verdict `READ-OK`.
- `storage-bench-read-488-refused-stale-card.log`: the same build and card, run while the card script did not match the image. The read numbers (10593 and 10740 KiB/s) agree with the clean run to within
  1.5 percent; the EL0 step then hit the refusal. Only the numbers count.
- `storage-bench-scratch-488.log`: the build writes only the eight sectors before the first
  partition (sectors 2040 to 2047). Lines 1 to 24 are not part of this run: they are a late flush
  of the UART buffer from an earlier run whose console was not logged, so the log opens mid-line
  and holds that run's tail (writes verified, then the EL0 server start). The run proper starts at
  the U-Boot banner on line 25. It verified the polled and IDMAC writes (`Verified(2040, 8, true)`
  twice) and then `user thread 4294967302 killed: scause 0xf (code 15)`, the EL0 block server
  thread faulting on a store near its stack. That is issue #3694, with a fix in draft pull request
  #3695. No `verdict` line is printed. The 10588 to 10803 KiB/s reads here match the read-only build's.

The read throughput is the card's and the 4-bit bus's, around 10.5 MiB/s by either path; the IDMAC
path does not read faster than the polled path here. One card, one run each, so a 2 percent
difference is not a finding.

## The second session: three logs added later that evening

Same board, same `tr`, same private details left in. The unfiltered SHA-256 of each capture is
beside its entry. `<host>` is the TFTP host's LAN address.

### Network bench on `main` (`network-bench-main-3699.log`, issue #3699)

- Commit: `main` at `cfe18db3377813dbaa3b962e67f950e433066d65`.
- Command: `NIFE_NETWORK_BENCH_PEER="<host>:9494" script/board-image --tftp --server <host> --extra-features network_bench`.
- Unfiltered SHA-256: `1434f6f2231742daabafddcd697808ed7cef602f22d27513f65402c518777a45`.
  The `tr` removed 13 bytes (11654 to 11641).
- Shows: `coherence : Coherent`, `link : 1000 Mbit/s full duplex`, `transfer : 104857600 bytes in
  6.017 s = 139.399 Mbit/s` and `verdict LEASED-AND-MEASURED` on the commit `main` holds, which is
  the proof issue #3699 waits for. The rate is the board's own `transfer` line, from its 4 MHz counter.
- Caveats: the peer was a laptop behind a USB ethernet adapter, so the rate is bounded by that
  peer and adapter and is not the link's ceiling. This and the 154.033 Mbit/s of
  `network-bench-494.log` (a different commit, 6.017 s against 5.445 s) are each one run. The
  difference between them is not claimed to mean anything.

### Route step 1 (`pmic-route-step1-3499.log`, issue #3499)

- Commit: `462ad3761d5b3a490dc4d3a94e8549cf0fd1e2a7`, the final head of #3499.
- Command: `script/board-image --tftp --server <host>` (features `board`), TFTP boot, no card in
  radon.
- Unfiltered SHA-256: `c480f310247b073c943811d427483f819b5b620658eb76a7c3f61fc70187697f`.
  The `tr` removed 21 bytes (18439 to 18418).
- Shows: the control for `pmic-route-withheld-3499-step2.log`. With the route present the log has
  `reboot: AXP15060: PMIC route, controller 0x12050000, PMIC 0x36 ...`, then `reboot` typed at the
  prompt, `restarting through the board's power controller`, and the firmware banner again. The
  board reset, which the withheld-route run did not do.
- Caveats: this replaces the step 1 log of the first session, whose only copy was lost when its
  worktree was pruned. It is not the same run as that one. It has no card, so after the reset the
  driver did not interrupt U-Boot, the autoboot countdown ran to 0, U-Boot found no card, tried a
  net boot from the router instead of the TFTP host, got `TFTP server died`, and the log ends at
  the `StarFive #` prompt. The earlier run, with a card, booted back to the shell. That difference
  is the card, not the route. Step 1 and step 2 differ in the packaged route program and are separate runs on one board.

### The fix for #3694 (`storage-bench-scratch-3695-fix.log`)

- Commit: `3d004a7bcdc47e5c9a6971b7fa82aa9863a2f925`, the head of draft pull request #3695.
- Command: `NIFE_STORAGE_BENCH_WRITE=scratch script/board-image --tftp --server <host> --extra-features storage_bench`,
  card in radon.
- Unfiltered SHA-256: `ddae6928766321f1b9cecda96e4f77350e9498fdd5a70334aa72f42c03ba5036`.
  The `tr` removed 13 bytes (13602 to 13589).
- Shows: `verdict READ-OK 10603 KiB/s, WRITE-VERIFIED sectors 2040..2048 (were zero), restored,
  IDMAC READ-OK 10740 KiB/s, IDMAC WRITE-VERIFIED, EL0 block server served`, and no `killed`
  line. `storage-bench-scratch-488.log` on `9c394ab715` printed no verdict and its EL0 thread was
  killed.
- Caveats: one run, one card. The read rates (10603 and 10740 KiB/s) match the earlier
  logs' within their one-run spread. The log says the scratch sectors were zero before the write and were restored.

## Dependents

This record does not block them: #3698 amends milestone 1035's block, and #3699 flips
`PROVEN_ON_SILICON`.
