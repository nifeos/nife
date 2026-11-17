# radon, 2026-11-17 (UTC): milestone 1035's hardware run of `jh7110_entropy`

Milestone 1035 (a crate names the version that ran on hardware), run by the steps in
`design/roadmap/1035-a-crate-names-the-version-that-ran-on-hardware/bench-steps.md`: two cold boots
of one network-booted image, judged by `helpers/crate_hardware_run.py`.

| item | value |
|---|---|
| nife commit | `9c394ab715a7221625ea1c9ea6f5f303ac0b3e99` (`origin/main` when the image was built) |
| crate | `jh7110_entropy` 0.3.0-pre.1 at `497d3ec4f0b4d7fc36634dee47d1afd56eb22485` |
| build | `script/board-image --tftp --server <the TFTP host's LAN address>` (features `board`) |
| `nife-vf2.img` SHA-256 | `c94e194223f17803f66a61fa8b965846bb085b98fb1d1ffa5d94cfeed5e788fd` |
| `nife-initial-ramdisk.img` SHA-256 | `cbb1ab870fe777e1bc0a915fa8f6d6b0e64b37004ef22997fee823b3b9340f9a` |
| boot 1 | `1035-boot-1.log`, first draw `625b4bc8`, bring-up 1637 us |
| boot 2 | `1035-boot-2.log`, first draw `7bb8b233`, bring-up 1629 us |

Both boots show `nife: payload came from net`, so the card's old kernel did not run. The log
copies went through the bench-session `tr` that drops control bytes (a backspace, NUL and SOH
the UART emitted, 13 bytes per log, no line removed); the rest is as the board printed it, so the
board's serial number, MAC addresses and private LAN addresses are in, as in the milestone 1049
logs. The unfiltered captures hash to `aba2b2b3bb4924e4f2ee4c07ca2492076d2b7421970f74af24a2e74767ddd6a0`
(boot 1) and `ba52ccaa38a38849c6b23621864790cc2639070e264aeb371b4d625f5e30fdb6` (boot 2).

## The judge's output

Run on the two committed logs:

```
python3 -I helpers/crate_hardware_run.py judge bench/radon-2026-11-17/1035-boot-1.log \
    bench/radon-2026-11-17/1035-boot-2.log --nife-commit 9c394ab715a7221625ea1c9ea6f5f303ac0b3e99
```

```
boot 1: ok, first 625b4bc8, mode '256-bit: all eight RAND words are the answer', crate 0.3.0-pre.1 at 497d3ec4f0b4d7fc36634dee47d1afd56eb22485
boot 2: ok, first 7bb8b233, mode '256-bit: all eight RAND words are the answer', crate 0.3.0-pre.1 at 497d3ec4f0b4d7fc36634dee47d1afd56eb22485
PASS  1. every boot names one crate version and commit: [('0.3.0-pre.1', '497d3ec4f0b4d7fc36634dee47d1afd56eb22485')]
PASS  1. the tag v0.3.0-pre.1 names 497d3ec4f0b4d7fc36634dee47d1afd56eb22485, the boots ran 497d3ec4f0b4d7fc36634dee47d1afd56eb22485
PASS  1. the nife commit is on origin/main
PASS  1. the nife commit's Cargo.lock resolves jh7110_entropy 0.3.0-pre.1 at 497d3ec4f0b4d7fc36634dee47d1afd56eb22485
PASS  2. 2 boot(s) carry a true_random_number_generator line, and the run needs two
PASS  3. every true_random_number_generator line is ok, with two full 32-byte draws that differ
PASS  4. every mode note says 256-bit
PASS  5. no true_random_number_generator line is FAILED
PASS  6. the first draws differ across boots: ['625b4bc8', '7bb8b233']
PASS
```

Exit status 0. The judge reads only the banner and the `true_random_number_generator` lines. It
shows that the crate version and commit the board printed are the ones the tag and the lockfile
name, and that two boots that each started at the firmware banner drew different bytes. Two draws are two measurements; they do not
show that the output is unpredictable (milestone 1049's logs hold the statistics). Nothing in the logs
records that the board was powered off for ten seconds between the boots (step 5); the two
captures were saved two minutes apart, and the `[drive: ...]` lines are the bench driver's own
notes, not the board's.
