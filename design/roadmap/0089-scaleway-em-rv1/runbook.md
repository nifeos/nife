# Milestone 89's runbook

Moved verbatim on 2026-10-13 (UTC) from [`../0089-scaleway-em-rv1.md`](../0089-scaleway-em-rv1.md),
under §212 (a prose budget), with its heading levels kept. The block holds the plan, the measured
distance, the costs and the BUGS; this file holds the steps calef runs on the rented machine.

## Runbook

Every step below is calef's, because it spends money or acts on his Scaleway account. It assumes
`scw init` has run and an SSH key is registered. Always pass `zone=fr-par-2`: the CLI defaults to
`fr-par-1`, which has no RV1.

### Step 0: Before renting, free

1. Ask Scaleway support to activate the serial console, the only way to see a kernel with no
   network driver. Suggested text: *"Please activate the
   SSH-accessible serial console for Elastic Metal RV1 servers on this account, as described in
   the Labs FAQ for EM-RV1. How do I connect to it once it is active?"* Record the answer in this
   block; it is the one fact the docs do not give.
2. Check stock and price:
   `scw baremetal offer list zone=fr-par-2 subscription-period=hourly name=EM-RV1-C4M16S128-A`.
   If it is out of stock, stop here.
3. On your machine: `script/board-image --th1520`, which writes four files under `target/board/`.

### Step 1: Rent, and read the machine (about an hour)

```sh
scw baremetal os list zone=fr-par-2 offer-id=<offer-id from step 0, item 2>   # pick the Debian or Ubuntu id
scw baremetal server create name=nife-89 type=EM-RV1-C4M16S128-A zone=fr-par-2
scw baremetal server install <server-id> os-id=<os-id> hostname=nife-89 ssh-key-ids.0=<key-id> zone=fr-par-2
scw baremetal server wait <server-id> zone=fr-par-2 timeout=40m0s
scw baremetal server get <server-id> zone=fr-par-2        # note the IP; billing should say hourly
```

Leave the billing period alone: switching to monthly is one-way. Note how long the install took.
Then, on the RV1 over SSH:

```sh
sudo apt-get install -y u-boot-tools device-tree-compiler
mkdir -p ~/nife && cd ~/nife
cp /sys/firmware/fdt live.dtb                    # the tree Linux was handed, after U-Boot's fixups
dmesg > dmesg.txt; cat /proc/cpuinfo > cpuinfo.txt; lsblk -f > lsblk.txt
cp /boot/boot.itb boot.itb.scaleway
dumpimage -l boot.itb.scaleway                   # lists the images and their indices
dumpimage -T flat_dt -p <fdt index> -o rv1.dtb boot.itb.scaleway
dumpimage -T flat_dt -p <opensbi index> -o fw_dynamic.bin boot.itb.scaleway
```

Copy `live.dtb`, `rv1.dtb`, `dmesg.txt`, `cpuinfo.txt`, `lsblk.txt` and `boot.itb.scaleway`'s
`dumpimage -l` output back to patagonia under `bench/rv1-<date>/`. `live.dtb` replaces the modeled
fixture, and every row of the distance table is re-checked against it. Check `/reserved-memory`
in particular: it must cover OpenSBI at physical 0, or the allocator will hand out frame 0.

### Step 2: First light

From patagonia, `scp target/board/nife-th1520.img target/board/nife-initrd.img
target/board/nife-th1520.its target/board/nife-th1520-env.txt <user>@<ip>:nife/`. Open the serial
console in a second terminal, logging to `target/rv1-first-light.log`. Then on the RV1:

```sh
cd ~/nife && mkimage -f nife-th1520.its boot.itb
sudo mv /boot/boot.itb /boot/boot.itb.scaleway && sudo cp boot.itb /boot/boot.itb && sync
sudo reboot
```

What success looks like, after OpenSBI's banner (QEMU's `thead-c906` boot of the default build,
with this machine's values where they differ):

```
nife on RISC-V (rv64, S-mode, Sv39)
  hart 0 booted: high-half kernel, .bss, and the NS16550 console are up.
  ...
  vendor      : T-Head, XTheadMae on, so leaves carry T-Head memory types
  ...
nife machine: riscv64, 4 processor(s), <about 16000> MiB, 100 Hz
nife self-test: 5 of 5 passed
```

`script/board-console --replay target/rv1-first-light.log --board xenon --until selftest` judges
the capture; `xenon`'s empty prologue stands in until the RV1 has a profile. Reading a failure:

| What the console shows | What it means |
|---|---|
| OpenSBI's banner, then nothing | the kernel never printed: U-Boot ignored `load`, OpenSBI jumped elsewhere, or the UART is not at `0xff_e701_4000` in `live.dtb` |
| `nife cannot run on this machine` and a page-table sentence | the MAEE probe disagreed with the build; the sentence says which way |
| the banner, then a panic or silence | a fault after first light; the log is the evidence, so keep it |
| `MEASURED BOOT REFUSED` | the kernel and archive came from different builds; rerun `script/board-image --th1520` and copy all four |

Recovery from any of them: `scw baremetal server reboot <id> boot-type=rescue zone=fr-par-2`, SSH
in as `rescue`, mount the FAT32 boot partition `lsblk.txt` named, rename `boot.itb.scaleway` back
to `boot.itb`, and `scw baremetal server reboot <id> zone=fr-par-2`. If rescue cannot reach the
eMMC, `scw baremetal server install` reinstalls the OS and erases the disk.

### Step 3: The soak, by milestone 225 (run the soak on radon, argon and xenon)'s procedure

`script/board-image --th1520 --soak`, then step 2's install, with the console logging to
`target/rv1-soak-<stamp>.log` for eight hours, radon's duration and reason (`notes/soak.md`).
Afterwards `script/board-console --exposure target/rv1-soak-<stamp>.log --machine rv1 --build <sha>
--start '<utc>'` gives the exposure row for `notes/multicore-defect-curve.md`. Red is
`soak-test: FAILED`, a `[PANIC]`, or three missed beats. The rebooting soak (`--reboot`) is not for
this machine yet: SBI SRST may be absent from Scaleway's OpenSBI.

### Step 4: The risk 9 measurement

The verdict is the per-file count below, completed with what hour one adds and copied into
`design/fatal-risks/the-hal-and-the-next-machine.md` beside the x86_64 port's. Each file is (a)
under `kernel/src/arch/riscv64/`, (b) portable code that reads a device tree, or (c) elsewhere,
with `crates/paging` named separately as the appendix did.

Before renting, measured on this branch against its base `e8834c988`:

| File | Place | What changed |
|---|---|---|
| `kernel/src/arch/riscv64/mmu.rs` | a | device window, per-machine `Format`, memory types and 16 GiB in the boot table, virtio window from the tree |
| `kernel/src/arch/riscv64/machine.rs` (new) | a | the console's address and node |
| `kernel/src/arch/riscv64/isa.rs` | a | the T-Head and MAEE probe |
| `kernel/src/arch/riscv64/instructions.rs` | a | the `th.sxstatus` read |
| `kernel/src/arch/riscv64/fp.rs` | a | XTheadVector's `VS` |
| `kernel/src/arch/riscv64/mod.rs` | a | one `mod` line |
| `crates/paging/src/sv39.rs`, `lib.rs` | c, additive | a new `PageFormat` implementation; no trait or walk change |
| `kernel/src/console.rs`, `kernel/src/user.rs` | c, seam repair | the riscv64 console constant now names `arch::machine`'s |
| `kernel/src/virtio.rs`, `arch/aarch64/mmu.rs`, `arch/x86_64/mmu.rs` | c, seam repair | the probe asks the architecture how many slots exist |
| `kernel/Cargo.toml` | build | the `board_th1520` feature |
| `script/board-image` | packaging | `--th1520` |

Reading it: no portable kernel logic was restructured. Two QEMU assumptions sat in portable code
and moved behind the seam: the console address, written three times, and an unconditional virtio
window. That is the "seam is not quite where the tree says" evidence this block exists to find,
and it is small. Whether the machine adds a row in (c) is what hour one decides.

### Step 5: Release, so billing stops

```sh
scw baremetal server delete <server-id> zone=fr-par-2
scw baremetal server list zone=fr-par-2                   # must not list nife-89
```

Then check the billing page. Stopping is not enough: the CLI says a stopped server "remains
allocated to your account", and allocated hours are billed. Every session ends with a delete.
