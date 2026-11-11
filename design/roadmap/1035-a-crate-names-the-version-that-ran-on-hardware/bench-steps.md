# The bench steps for a hardware run of a crate version

An appendix to [milestone 1035 (a crate names the version that ran on
hardware)](../1035-a-crate-names-the-version-that-ran-on-hardware.md). The main page is complete
without it. These steps follow the bench procedure of [milestone 159 (a real hardware entropy source: the
JH7110's TRNG)](../0159-jh7110-trng-driver/bench-procedure.md). The network boot of [milestone 257
(boot radon over the network, so the microSD card stops being the tax on every
experiment)](../0257-network-boot-radon.md) replaces its card. A person runs
them at the board; an agent can drive the host side and read the result.

The maintainers keep the bench's unit-specific details off-tree: which serial device is the
board's, which outlet powers it, and which outlets and hubs must never lose power. Read those
before touching anything.

## What it needs

- A VisionFive 2 with its serial console on UART0 (header pins 6, 8 and 10, 115200 8N1), the
  boot-mode switches on QSPI, and a microSD card that carries the network-first boot script
  `script/board-image --tftp` writes. A boot from that card prints `nife: tftp server is <address>`.
- A host on the board's local network with the toolchain from `script/setup`, UDP port 69 free
  and a clean checkout of nife.

## The steps

1. Choose the nife commit. Check out `origin/main`, or the commit on it that bumped the pins,
   with no local changes:

   ```
   git fetch origin && git checkout --detach origin/main && git status --short
   git rev-parse HEAD
   ```

   `git status --short` prints nothing. Record the commit. Its three manifests
   (`kernel/Cargo.toml`, `components/Cargo.toml`, `crates/machine_discovery/Cargo.toml`) state the
   crate's `version` and `rev`, and the judge in step 6 checks that the `rev` is the commit of the
   tag `v<version>`.

2. Build. `script/board-image --tftp`. It builds the archive and then the kernel, in that order,
   so the two are one measured pair, and it writes them under `target/board/`. Record the SHA-256 of
   `target/board/nife-vf2.img` and `target/board/nife-initial-ramdisk.img`
   (`shasum -a 256` on macOS, `sha256sum` on Linux).

3. Serve and watch. In two terminals:

   ```
   script/board-netboot
   script/board-console --port <device> --until selftest --for 5m --log target/1035-boot-1.log
   ```

4. First boot. Power the board on from off. The log shows `nife: payload came from net`, then
   the boot tour. `board-console` stops once the self-test verdict prints. If the log shows
   `payload came from card` instead, the board booted an older kernel from the card; the judge
   refuses that boot because its `trng` line names another crate commit or none. Fix the network
   and boot again.

5. Second boot. Power the board off, wait ten seconds, start `board-console` again with
   `--log target/1035-boot-2.log`, and power it on. Two cold boots are what make condition 6 (the
   first draws differ across boots) mean that the device reseeds per boot.

6. Judge.

   ```
   python3 -I helpers/crate_hardware_run.py judge target/1035-boot-1.log target/1035-boot-2.log \
       --nife-commit <the commit from step 1>
   ```

   It prints one row per condition of the main page's definition and ends `PASS` or
   `FAIL, N problem(s)`. Each boot's line reads like this, and its last clause names the crate:

   ```
   self-test       : trng       ok      JH7110 TRNG at 0x1600c000 (tree says disabled): report 0x524e475550,
       STAT 0x00040308 (256-bit: all eight RAND words are the answer), draws 32/32 bytes, first 93a34471..,
       differ true; bring-up 551 us, 64 bytes in 68 us (937728 bytes/s);
       crate jh7110_entropy 0.2.0 at cfdbeb78e4f63eb251cd11b5dcf1521d4ed0ca06
   ```

   A `FAILED` line, or one the judge refuses, goes to milestone 159's [table of what each line
   means](../0159-jh7110-trng-driver/bench-procedure.md) before anyone theorizes.

7. Record. Commit the two logs under `bench/<board>-<date>/` with a `README-1035.md` that names
   the nife commit, the crate version and commit, the two image hashes and the judge's output. Put
   each boot's `trng` line, the nife commit and the crate version in the main page's block. Only then
   does the crate's README name the version.

## What the log holds

A board log carries the board's serial number, its MAC addresses and private local addresses. The
runs of milestone 1049 (`jh7110_entropy` is ready to publish: the release review's findings)
committed theirs as the board printed them. The person may instead trim those lines first, and the
bench README then says so. The judge reads only the banner and the `trng` lines.
