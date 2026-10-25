# xenon bench evening: the I219, the network boot, a fresh install, `jig`, then ioping and the soak

*Name: provisional, minted 2026-10-18 (UTC) by lane `lane/xenon-bench-2026-10-18`, after
`notes/xenon-bench-2026-10.md`; naming is an architect's.*

One evening at xenon, not yet scheduled; the file takes the evening's date when it is.

calef ruled
on 2026-10-24 (UTC) that milestone 260's network boot runs on this evening, beside milestone 494's
bench boot, the fresh install and milestone 801's xenon leg. The steps go in this order, because
each needs the one before:

| step | what | closes | about |
|---|---|---|---|
| 1 | the I219 boot, ending `verdict LEASED-AND-MEASURED` | milestone 494 (a driver for the network card a PC actually has)'s exit criterion 2 | 20 min |
| 2 | flip `PCH_PROVEN_ON_SILICON` citing step 1's log; build the netboot image and the installer from that commit | nothing; it is what steps 3 to 5 boot | 10 min |
| 3 | xenon boots nife over the wire with no stick (`notes/x86-uefi-boot/network-boot-xenon.md` steps 3 and 4) | milestone 260 (boot xenon over the network) | 15 min |
| 4 | the fresh install, then a boot from the disk | re-proves milestone 515 (the installer)'s exit criterion 2 with a current image; clears 801's install blocker (`bench/xenon-2026-10-21/README-801.md`) | 10 min |
| 5 | `jig update` and `jig install ioping` from `basalt.nifeos.org` | milestone 801 (packages over the internet)'s x86_64 leg, one of the three its exit criterion names | 5 min |
| 6 | risk 6, step 1 of its path back to GREEN: ioping on nife (the package step 5 installed), then on Linux, three boots of five passes each | nothing yet; the comparison goes to risk 6 | 1 hour |
| 7 | risk 5: the first of the three boots §259 (a multicore soak counts toward risk 5 at ten million crossings over three boots) needs on xenon, left overnight | nothing yet | overnight |

Steps 1 to 5 are the evening; 6 and 7 run if it goes on. Each step's own section ends with what to
record.

Unit details (serial device names, addresses, serial numbers) are kept off-tree by the maintainers.
Where this card and a procedure page disagree, the machine decides and the page gets corrected.

## Status

Ready to run. Every image below was built from main at `ca9be2ca5` and booted under OVMF. The
evening's date is not set. Step 3's image is the ordinary release image, which CI's
`netboot-rehearsal` row PXE-boots under OVMF on every pull request.

| image | build | under OVMF | SHA-256 |
|---|---|---|---|
| I219 boot, house-LAN peer | `cargo xtask network-bench --stage-only --peer <the build Mac's LAN address>:9494` | `release build`, `82574L ... link up`, `preflight dmar scope : PASS`, `dhcp : leased 10.0.2.15`; gate `verdict LEASED-AND-MEASURED` | `5d3b261e0aebbd6f3cefdf1b5ed536d68bc22df0033aff30931c1e19af8f70b4` |
| I219 boot, Internet Sharing peer (not this evening: step 3 needs the router) | `... --peer 192.168.2.1:9494` | the same lines | `3ed24e29bb43b4503b4b9bcad683d13ce6f504dc3d43ede4fac59e51271c5520` |
| installer, basalt's sources, flag unflipped (rehearsal only) | `cargo xtask std-exerciser`, then `NIFE_INDEX_SOURCES=basalt cargo xtask install-boot` | `install-boot: PASS`, three boots; steps 5 and 6 below | `91adb2e60680be4a2671b38cbf262b334b5d64bd92fb173012143e29d6c9a7dc` |
| soak, debug | `cargo xtask uefi-image --features soak_test` | 4 cores online, 24 user threads; at 65 s `crossings=7080`, `wakerate=402/s`, `refused=0 mismatch=0 stalled=0` | `7be0aef659d4e888700b3dee371a356b989be071da09e8d0b588d0e466668fef` |

The I219 image's peer address is compiled in, so there is one image per network arrangement. Under
OVMF the staged images lease and then end `verdict FAILED: the transfer did not complete`, because
no peer listens there; the gate's own peer gives `LEASED-AND-MEASURED`. The installer that goes to
xenon is built at the bench in step 2, with step 3's, so their hashes are recorded then. Copies are kept off-tree by
the maintainers.

## What the rehearsal shows

From the installer's disk under QEMU, step 5 prints `index updated: 6 packages for x86_64, from
basalt.nifeos.org over TLS`, then `fetched ioping-1.3-x86_64 from
release-assets.githubusercontent.com via github.com over TLS` and `fetched and installed;
generation 1 is live`.

Step 6's nife line, `ioping -D ... n1`, stops at `direct I/O not supported by this platform`.
basalt's ioping is built without `-DHAVE_DIRECT_IO`, and needs a rebuild at a nife pin that
includes #2371. Until then the evening runs the line without `-D`, as step 6 says. That line
completes 999 requests at `min/avg/max/mdev = 80 us / 2.43 ms / 30.8 ms / 7.30 ms` on QEMU's
NVMe, against 41 to 42 us for a cached read, so it reaches the device.

## What calef needs at the bench

- **A monitor and a USB keyboard on xenon.** It halts at POST without a keyboard.
- The serial chain: the C4PDJ module's COM1, the null-modem barrel and the FTDI adapter into
  the build Mac. Every step logs over it. The installer's `INSTALL` is typed there, since nife has no
  USB keyboard driver.
- Ethernet on xenon, on a long cable to the house router. Its room has no port. Step 3 needs it:
  the router carries the network boot's DHCP lines, so macOS Internet Sharing will not do for this
  evening, and step 1 uses the house-LAN image. The build Mac serves from the same LAN.
- Two FAT32 sticks: `NIFE` for the nife images and the ioping RPM, and the Fedora 44 live stick.
- Someone at the build Mac for step 2, which is a commit and a build.

## Before the evening, on the build Mac

In a checkout of the commit under test (a fresh worktree builds nothing it was not asked to):

```sh
pgrep -l qemu                                    # nothing of yours should be running
git log -1 --format=%h                           # goes in every Results row
cargo xtask std-exerciser                        # builds jig; without it the image has no jig
P=$(ipconfig getifaddr en0)                      # the build Mac on the house LAN
cargo xtask network-bench --stage-only --peer "$P:9494"
shasum -a 256 target/esp-network-bench/EFI/BOOT/BOOTX64.EFI
curl -LO https://dl.fedoraproject.org/pub/fedora/linux/releases/44/Everything/x86_64/os/Packages/i/ioping-1.3-10.fc44.x86_64.rpm
```

`cargo xtask std-exerciser` matters: `jig` is packed only when it has been built, and an image
without it answers `jig update` with `could not spawn (the progenitor is out of memory)`.

## Step 1: the I219 boot

The procedure is `notes/e1000e.md`, "calef's bench step on xenon", with its table of every ending.
In short: start the peer in a terminal on the build Mac,

```sh
while true; do head -c 104857600 /dev/zero | nc -l 9494; done
```

then capture serial in a second terminal, copy the image and boot xenon from it:

```sh
script/board-console --board xenon --port /dev/cu.usbserial-<id> --for 10m --until none \
    --log target/xenon-i219-$(date -u +%Y%m%dT%H%M%SZ).log
cp target/esp-network-bench/EFI/BOOT/BOOTX64.EFI /Volumes/NIFE/EFI/BOOT/BOOTX64.EFI && diskutil eject /Volumes/NIFE
```

Success is `network-bench: nic : 8086:15e3 I219-LM5 (SPT), link up`, a `dhcp : leased` line,
a `transfer` line and `network-bench: verdict LEASED-AND-MEASURED`. Anything else ends the evening's
network half: route it by that table, and go to step 6's Linux half and step 7.

Record: the log as `bench/xenon-<date>/i219.log`, and `notes/e1000e.md`'s Results row. Read off its
`iommu` line for each VT-d unit, which ends `dmar INTR_REMAP set` or `clear`. That closes milestone
494's exit criterion 2.

## Step 2: flip the flag, build what steps 3 to 5 boot

Only after step 1 ended `LEASED-AND-MEASURED`. On a branch from the commit under test:

```sh
git switch -c lane/i219-proven-on-xenon
sed -i '' 's/^pub const PCH_PROVEN_ON_SILICON: bool = false;/pub const PCH_PROVEN_ON_SILICON: bool = true;/' kernel/src/user/e1000e_service.rs
git commit -am "e1000e: the I219 is proven on xenon (bench log <file>, verdict LEASED-AND-MEASURED)"
cargo xtask uefi-image --release                            # step 3's image, in target/esp
shasum -a 256 target/esp/EFI/BOOT/BOOTX64.EFI
NIFE_INDEX_SOURCES=basalt cargo xtask install-boot          # step 4's, must end install-boot: PASS
shasum -a 256 target/esp-install/EFI/BOOT/BOOTX64.EFI
cp target/esp-install/EFI/BOOT/BOOTX64.EFI /Volumes/NIFE/EFI/BOOT/BOOTX64.EFI && diskutil eject /Volumes/NIFE
```

The commit message names the step 1 log, and the log is committed with it under
`bench/xenon-<date>/`, as the flag's own comment requires. QEMU's NIC is an 82574L, not a PCH part,
so `install-boot` passing says nothing about the I219; step 5 is the I219's test at the prompt.
Built from the flipped commit, step 3's image also brings the I219 up at its prompt, which is a
second look at the card for free.

Record: both SHA-256 lines, in the Results rows of steps 3 and 4.

## Step 3: milestone 260, xenon boots over the wire with no stick

The procedure is `notes/x86-uefi-boot/network-boot-xenon.md`, step 3 and the record in its step 4.
The firmware settings and the router lines are in place, so this is one power cycle. On the build
Mac, from the same checkout, with the image step 2 built:

```sh
script/board-netboot --root target/esp            # allow python3 through the firewall if asked
```

Take the `NIFE` stick out of xenon, and leave no nife medium in any port. Capture serial as in step
1 (`--log target/xenon-netboot-<stamp>.log`), power xenon on, and press F12 then `UEFI PXEv4` if
it does not pick PXE itself.

Success is `board-netboot`'s `done, N bytes in T s (R KiB/s)` line, then the kernel's `nife
self-test:` line and `$` on serial and on the monitor. The runbook's failure table covers every
other ending. A failure here costs only milestone 260; go on to step 4.

Record what the runbook's step 4 lists: photographs of the screen's `nife self-test:` line and of
the empty ports, and the `done` line as text. Add whether option 93 was 7 or 9, from the router's
log if `log-dhcp` is on, and photographs of the `Integrated NIC` setup page. With those, milestone
260 can be marked BUILT.

## Step 4: the fresh install, then a boot from the disk

This destroys what is on the NVMe, which is the 2026-10-04 install that no longer boots. The steps
are the October card's step 2 (`notes/xenon-bench-2026-10.md`), with the serial capture in
`screen`, because `INSTALL` has to be typed:

```sh
cd <checkout>/target && screen -L /dev/cu.usbserial-<id> 115200     # ctrl-a k ends; log is screenlog.0
```

1. Put the `NIFE` stick back and boot it. Check that the offer's `TARGET` names the disk by size,
   then type `INSTALL` in `screen` within 30 seconds. Wait for `install : DONE`.
2. Remove the stick, power off, power on with the Ethernet cable in. Note whether the firmware found
   the disk itself or needed F12.

Success is the installed system's own `nife self-test:` line and `$`, with no stick in. Leave
`screen` running into step 5.

Record: whether the firmware found the disk itself, and the installer's SHA-256. That re-proves
milestone 515's exit criterion 2 with a current image; it closes nothing new in 515.

## Step 5: milestone 801's xenon leg, `jig`

At the installed system's `$`, type:

```text
jig update
jig install ioping
cd packages
ioping -v
```

| line | means |
|---|---|
| `index updated: N packages for x86_64, from basalt.nifeos.org over TLS` | the index came by name over pinned TLS |
| `fetched ioping-1.3-x86_64 from ... over TLS`, then `fetched and installed; generation N is live` | milestone 801's exit criterion is met |
| `ioping 1.3` | the installed program runs |
| `index unreachable at basalt.nifeos.org: not resolved: <kind> (<step>)` | the name did not resolve; the parenthesis names the resolver's failed step |
| `index unreachable at basalt.nifeos.org: <other>` | the name resolved and the connection or TLS failed; the text says which |
| `passed over https://...: <why>` | the Release location was refused; `jig` falls back to Pages' `targets/` |
| `no source sent the package; nothing was installed` or `refused: the index does not vouch for the bytes` | a fetch or digest failure; stop |

Record: `screenlog.0` from steps 4 and 5 as `bench/xenon-<date>/install-and-fetch.log`, and a line
in milestone 801's block. The two success lines meet 801's x86_64 leg. The milestone stays
`PARTIAL` until its radon and Oracle A1 legs run too.

## Step 6: ioping on nife, then on Linux

calef ruled on 2026-10-18 (UTC) that ioping is the benchmark that explains or corrects risk 6's 2.16x
read: unmodified, on nife and on Linux on xenon, with real logs and repeated passes.

### nife, three boots

The target is `n1` in the disk tree: the whole NVMe namespace as one read-only file, served with no
cache, so every request is a device read. In `screen`, after each boot from the disk:

```text
cd /disk/nvme0
ioping -D -c 1000 -i 0 -s 4k -S 64m -o 1m n1
```

Type the `ioping` line five times. Power off fully between boots. If basalt's ioping was not
rebuilt for direct I/O, the line runs without `-D`: ioping prints `non-cached I/O not supported`
and still measures the device, since the disk tree has no cache. The log and the Results row say
which line ran. `/disk`, `nvme0` and `n1` are provisional names.

### Linux, three boots

Fedora stick in, F12, "Try Fedora", a terminal. Nothing here writes to the NVMe: ioping refuses to
write a device without `-WWW`, and no line passes `-W`.

```sh
sudo dnf install -y ioping nvme-cli || sudo rpm -i /run/media/liveuser/NIFE/ioping-1.3-10.fc44.x86_64.rpm
sudo stty -F /dev/ttyS0 115200 cs8 -cstopb -parenb -crtscts onlcr
```

With `screen` capturing on the build Mac, one command per boot, `B` the boot:

```sh
B=1; {
  date -u +%FT%TZ; uname -r; ioping -v
  cat /sys/block/nvme0n1/queue/scheduler /sys/module/nvme/parameters/poll_queues
  sudo nvme smart-log /dev/nvme0n1 | grep -i temp
  for p in 1 2 3 4 5; do echo "=== boot $B pass $p direct"; sudo ioping -D -c 1000 -i 0 -s 4k -S 64m -o 1m /dev/nvme0n1; done
  for p in 1 2 3 4 5; do echo "=== boot $B pass $p page-cache"; sudo ioping -c 1000 -i 0 -s 4k -S 64m -o 1m /dev/nvme0n1; done
} 2>&1 | sudo tee /dev/ttyS0
```

`-c 1000 -i 0 -s 4k` is the same on both systems: 1000 requests of 4 KiB, no pause, one at a time,
each line a latency. `-S 64m -o 1m` is the 64 MiB window at 1 MiB that risk 6's figure and the
Oct 4 `fio` run used. The `direct` set is nife's line run on Linux, with `O_DIRECT`. The
`page-cache` set leaves out `-D`, so ioping drops the page cache before each read; it is the twin
of nife's line when nife runs without `-D`.

## Step 7: the soak, boot 1 of three

radon crossed at 44 to 150 a second, so ten million on xenon is something like 20 to 60 hours over
three boots. A plain boot soaks until the watcher stops; it is not the `reboot_soak_test` build. The
procedure is `notes/soak.md`, "On radon at a bench". Quit `screen`, check `lsof /dev/cu.usbserial-*`,
then:

```sh
script/board-console --board xenon --port /dev/cu.usbserial-<id> --for 12h --until none \
    --log target/xenon-soak-$(date -u +%Y%m%dT%H%M%SZ).log
```

Boot the soak image. Before walking away, the first beat must show `on 4 online core(s)`,
`wakerate` near 400/s, and `crossings` rising between beats; if not, stop.

## Recording

Under `bench/xenon-<date>/`, each log whole, stripped with `LC_ALL=C tr -cd '\11\12\15\40-\176'`
and nothing else (`notes/bench-runbook.md`'s BUGS):

- `i219.log`, with `notes/e1000e.md`'s Results row; it lands with step 2's commit.
- `netboot.log`, the photographs' numbers and the `done` line, for milestone 260's block.
- `install-and-fetch.log` and `ioping-nife.log` from `screenlog.0`, and the line for milestone 801's
  block.
- `ioping-linux.log`, and risk 6's comparison, every pass, through whoever holds `design/`.
- `soak-boot1.log`, with its last beat in `notes/soak.md`'s table and milestone 225 (run the soak on
  radon, argon and xenon)'s block.

## BUGS

- The nife and Linux ioping rows read the same device window with the same flags, but not through
  the same path. nife's request crosses the disk tree's server to the NVMe driver, and Linux's goes
  through its block layer. That difference is what risk 6 measures, so the row states it rather
  than hiding it.
- ioping prints `n1 (  0 B)` because its device size is filled in only on platforms its
  `parse_device` knows, and nife is not one. The working set comes from `st_size`, which nife
  reports as the namespace's size; ioping refuses `target is too small for this` otherwise. So
  requests land at random offsets in the 64 MiB at 1 MiB.
- Step 2 builds the evening's installer from a branch that has not merged, because the flag may
  flip only after silicon proves it. The branch's pull request follows the evening.
- The soak image is a debug build, as radon's were.
- Every xenon-side line above is a prediction. xenon has booted four cores (2026-09-17) and has
  never run the I219 path, a network boot, `jig` or the soak.
