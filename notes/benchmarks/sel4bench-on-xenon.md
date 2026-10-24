# sel4bench on xenon: refereeing the method against seL4's published Skylake row

*An appendix to [`notes/benchmarks.md`](../benchmarks.md). It holds whether xenon can do for the
seL4 comparison what milestone 127 (the seL4 machine) bought the Jetson TX1 to do, the build and
boot that would answer it, and what calef does at the bench. Name: provisional, minted 2026-10-21
(UTC) by lane milestone/127-xenon-sel4bench-calibration; calef has not ratified it, nor
`bench/xenon-sel4bench/`.*

## The question, and the short answer

[Milestone 127](../../design/roadmap/0127-the-sel4-machine.md) chose the TX1 because the seL4
Foundation publishes aarch64 figures only for it: run sel4bench on the same silicon, reproduce
their numbers, and only then trust our own sel4bench run beside nife's. The same page publishes
x86_64 figures for an Intel i7-6700, a Skylake. xenon's i5-7500T is Kaby Lake, which is the
Skylake core on a refined process: same pipeline, same caches per core, different clocks and L3.

So xenon can referee the method, on hardware in hand, with three caveats this note works through.
The x86 "cycle" is a TSC tick rather than a core cycle. xenon's microcode may be new enough that
seL4 refuses the published configuration. The Foundation's clock settings are not published.
Everything up to the bench is built and rehearsed. The run needs calef at xenon, for the power
button and the POST keyboard halt.

## What the Foundation runs, read from its trees

[sel4.systems/performance.html](https://sel4.systems/performance.html) was generated on 2026-10-21
for sel4bench-manifest `11d0fc2c`. Its Skylake rows, Default configuration, "without meltdown
mitigation":

| row | IPC call | IPC reply | Notify | IRQ invoke |
|---|---|---|---|---|
| Default | 743 (23) | 597 (2) | 756 (7) | 1605 (61) |
| MCS | 769 (30) | 663 (2) | 1065 (7) | 1948 (61) |

Mean (standard deviation) of 16 samples, in the page's "cycles". Each published figure traces to
one CI run on one machine:

- [seL4/ci-actions](https://github.com/seL4/ci-actions) `sel4bench/builds.yml` (read at
  `4da5cd5`), `results:` maps the Skylake row to run `PC99_64_noksw_skylake` and the MCS row to
  `PC99_MCS_64_noksw_skylake`. `sel4bench/build.py`'s `make_runs` splits every PC99 build into a
  run on machine `skylake` and one on `haswell3`.
- [seL4/machine_queue](https://github.com/seL4/machine_queue) `scripts/systems/skylake` (read at
  `ae07464`): an i7-6700, 4 cores, 8 GB, `MaxCLK` 3.4 GHz, and two boot files (kernel and root
  task). How the queue's server boots an x86 machine is not in the public tree.
- The build command is the page's own: `init-build.sh -DKernelSkimWindow=FALSE -DFASTPATH=TRUE
  -DHARDWARE=TRUE -DFAULT=TRUE -DITERATIONS=5 -DPLATFORM=x86_64`, MCS adding `-DMCS=TRUE`. RELEASE
  is absent because sel4bench's `easy-settings.cmake` defaults it on. The compiler is gcc 14.2.0
  from the `trustworthysystems/sel4` image.
- [seL4/sel4bench-results](https://github.com/seL4/sel4bench-results) is public and holds every
  run, all five iterations, with min, quartiles, median and max. The page shows iteration 0. For
  `PC99_64_noksw_skylake` at `11d0fc2c` that is call min 658, median 748, mean 743; reply median
  596, mean 597. Its `print_results.py` reads a raw sel4bench serial log, so our run is compared
  with the Foundation's own extractor, not ours.

**Their figure is one of two modes.** Over the 138 `PC99_64_noksw_skylake` runs from 2026-02-20
to 2026-10-21, the call mean falls in 714 to 725 in 123 runs and in 741 to 745 in 15; reply
likewise, 574 to 586 and 597 to 601. Every run sits wholly in one mode, all five iterations
together, so the mode is set per boot. Every high-mode run is from 2026-09-22 or later, and the
published run is one of them. Within a mode their spread is under 2%. A single xenon boot may land
in either mode, so it is read against both.

The effective configuration, read from the CMake cache of our build of the same command. These
are the values a reader would otherwise have to guess, and the Default and MCS rows differ only in
`KernelIsMCS`:

| setting | value | what it means here |
|---|---|---|
| `CMAKE_BUILD_TYPE`, `KernelFWholeProgram` | `Release`, `ON` | optimized, whole-program |
| `KernelFastpath` | `ON` | the `Call`/`ReplyRecv` fastpath is what is timed |
| `KernelSkimWindow` | `FALSE` | no Meltdown page-table isolation |
| `KernelX86IBRSMode`, `KernelX86IBPBOnContextSwitch`, `KernelX86RSBOnContextSwitch` | `ibrs_none`, `OFF`, `OFF` | no Spectre-class mitigations either |
| `KernelX86MicroArch` | `haswell` | `-march=haswell`; both parts run it |
| `KernelFPU`, `KernelXSave` | `XSAVE`, `XSAVE` | the client thread's FPU state is saved with `xsave` |
| `KernelSupportPCID` | `ON` | address-space switches keep tagged TLB entries |
| `KernelBenchmarks` | `generic` | the benchmark build sel4bench needs |
| `KernelMaxNumNodes` | `1` | one core |
| `KernelIOMMU` | `ON` | VT-d set up at boot; not on the IPC path |
| `KernelPC99TSCFrequency` | `0` | the kernel calibrates the TSC against the PIT |

**Our build is theirs, byte for byte, whichever host builds it.** The image is multi-architecture.
On 2026-10-21 the noksw configuration built in the amd64 image (native gcc) and in the arm64 image
(Debian's `x86_64-linux-gnu-gcc`, the same 14.2.0-19) produced identical files:
`kernel-x86_64-pc99` sha256 `a3cb713c...dacba1` and `sel4benchapp-image-x86_64-pc99`
`fdbc930f...89c2a8`. So `build.sh` runs the host's own architecture, about twenty times faster on
Apple silicon than the amd64 image under emulation.

## What a "cycle" is on x86

sel4bench's x86 counter is `rdtsc`, fenced by `cpuid` on both sides
(`libsel4bench/arch_include/x86/sel4bench/arch/sel4bench.h` in seL4_libs). It is not a core-cycle
counter. On both parts the TSC is invariant and ticks at the nominal clock: 3.4 GHz on the i7-6700,
2.7 GHz on the i5-7500T. One tick is one core cycle only while the core runs at exactly that clock.

Neither the page nor the machine queue says whether the Foundation's machine ran turbo. Without an
operating system nothing writes `IA32_PERF_CTL`, so the core runs at whatever ratio firmware left,
and seL4 writes none of the power-management MSRs. If their core sat at 3.4 GHz, 743 ticks is 743
cycles. If it turbo'd to the i7-6700's 4.0 GHz, 743 ticks is up to 874 cycles. That is the largest
unknown on their side, and nothing we can run removes it.

On xenon it is measured, not assumed. GRUB reads the MSRs that set the core clock just before it
hands over to seL4 (`bench/xenon-sel4bench/grub.cfg`, `clock_probe`): the nominal and turbo ratios,
the requested and current ratios, the turbo-disable bit and whether hardware P-states are on.
If xenon's requested ratio is the nominal 27 and hardware P-states are off, its ticks are its
cycles and the comparison reads directly. If xenon runs above 27, its ticks undercount its cycles
by 27 over the ratio, and the runbook's turbo-off boot is the comparison that counts.

## Apples-to-apples, item by item

| | Foundation `skylake` | xenon | expected effect on IPC cost in cycles |
|---|---|---|---|
| core | Skylake (i7-6700) | Kaby Lake (i5-7500T), the same core | none |
| L2 per core, L3 | 256 KB, 8 MB | 256 KB, 6 MB | none for a hot-cache loop |
| threads | 8 (SMT on or off: not published) | 4, no SMT | none: a non-SMP seL4 starts only the boot core |
| nominal, turbo | 3.4, 4.0 GHz | 2.7, 3.3 GHz | none on ticks if both run nominal; see above |
| microcode | old enough not to enumerate `IA32_ARCH_CAPABILITIES` | BIOS 1.27.0's, revision read by the probe | see below |
| boot | not published (two files by multiboot) | UEFI, GRUB 2.12 multiboot2 | none after boot |
| sources, compiler | manifest `11d0fc2c`, gcc 14.2.0 | the same, byte-identical images | none |

**The microcode row is the one that can stop the run.** With the skim window (seL4's Meltdown
mitigation) off, seL4's `try_boot_sys` checks `IA32_ARCH_CAPABILITIES.RDCL_NO` whenever CPUID
enumerates that MSR, and refuses to boot a vulnerable CPU. A release kernel has no `printf`, so the
refusal is silence. Skylake and Kaby Lake are vulnerable, so the Foundation's noksw runs boot only
because their i7-6700's microcode does not enumerate the MSR. Whether xenon's does is not
recorded anywhere; later microcode for some client parts added the MSR to report newer
mitigations. Boot 1 of the runbook answers it, and so does the probe entry in GRUB's menu.

So `bench/xenon-sel4bench/build.sh` also makes `noksw-patched` and `mcs-noksw-patched`: the
published kernels with the refusal's one conditional branch (`test $0x1,%al; je`, right after
`rdmsr` of `0x10a`) replaced by a NOP of the same six bytes (`noksw-boots-anyway.py`). Every other
byte of each image is the published one, so every instruction sel4bench times keeps its address
and its bytes. The patch is made in the binary on purpose. Removing the `return false` in the
source left the Default kernel's `.text` byte-identical, but in the MCS build it regrew `boot_sys`
and moved all of `.text` by 60 bytes, which changes the cache-line alignment of the code being
timed. The script refuses unless the sequence occurs exactly once. It also builds `ksw`, the skim
window on, which the
Foundation runs on the same machine as `PC99_64_skylake` and publishes only in sel4bench-results
(at `11d0fc2c`: call 964, reply 823). `ksw` boots unpatched on any microcode, so it is the
unpatched cross-check if the noksw rows need the patch.

Microcode can change costs on paths it touches; the mitigations it added for these parts sit on
`verw`, `rdrand` and vector gathers, none of which seL4's IPC path executes. That is reasoning, not
a measurement. The `ksw` boot is the measurement: if xenon's `ksw` matches their `PC99_64_skylake`
as closely as the noksw rows match, microcode is not moving the path.

## How it boots on xenon

seL4 on x86 is started by multiboot, two files: the kernel, and the root task as a module. xenon's
firmware is UEFI and speaks neither, so GRUB does it: `grub-mkstandalone` packs Debian's GRUB 2.12
(`x86_64-efi`), `grub.cfg` and one configuration's two images into a single
`EFI/BOOT/BOOTX64.EFI`. seL4 reads the ACPI root pointer and the memory map from GRUB's multiboot2
tags, and finds COM1 at `0x3f8`, where xenon's serial module is.

There is one file per configuration, `OUTDIR/esp/<configuration>/EFI/BOOT/BOOTX64.EFI`, each booting
its own entry after a five-second timeout. The configuration is chosen on the serving Mac, by which
directory `script/board-netboot` serves or which file goes on the stick, because
`script/board-console` sends nothing to a board and nobody needs to type at GRUB. Network boot uses
the path milestone 260 (boot xenon over the network) built (`bench/xenon-netboot/dnsmasq.conf` names
`EFI/BOOT/BOOTX64.EFI`); the stick is the path milestone 87 (the x86_64 bare-metal machine) proved.

### What UEFI changes for sel4bench itself, found in rehearsal

sel4bench's IRQ benchmark (`apps/irquser`) programs a timer through libplatsupport's
`ltimer_default_init`. That finds the HPET through ACPI, and its `acpi_init` finds ACPI only by
scanning the legacy BIOS area, `0xE0000` to `0xFFFFF`, for the root pointer. It never asks seL4,
which has the pointer from GRUB. UEFI firmware need not put a copy there. Without one the timer
falls back to the PIT, and the benchmark process is refused the PIT's I/O port
(`arch_simple_get_IOPort_cap not implemented`). The run then never reaches its `JSON OUTPUT` block.
That block is printed once, at the end, and is the only place any result appears, so the IPC
benchmarks that ran before it are lost with it.

Whether xenon's firmware keeps a legacy copy of the root pointer is not recorded, and boot 1 below
answers it. If it does not, the `-noirq` configurations are the same command with
`-DIRQUSER=OFF`. `build.sh` refuses to finish unless their kernel and every other benchmark binary,
the IPC one included, are byte-identical to the full build's, so they time exactly the published
code and lose only the IRQ invoke column.

### The rehearsal under OVMF, and what it cannot reach

On patagonia, on 2026-10-21:

- `script/netboot-rehearsal --esp OUTDIR/esp/noksw` served the published Default image to OVMF
  over the house-network offer in `bench/xenon-netboot/dnsmasq.conf`, through the real
  `script/board-netboot`. The firmware fetched the 8.4 MB file, GRUB 2.12 came up on the serial
  line, the probe skipped the MSRs on QEMU's CPU as designed, and multiboot2 handed over.
- seL4 then stopped with `PCIDs not supported by the processor`. The published configuration has
  `KernelSupportPCID` on, QEMU's TCG cannot provide PCID, and xenon's Kaby Lake has it. Nothing
  past this point of the published images can run under emulation.
- So the rest was rehearsed on images built from the same command plus `-DKernelSupportPCID=OFF`,
  which serve no other purpose. Booted the same way, seL4 started, sel4bench ran all five IPC
  iterations, and the IRQ benchmark failed as described above. With `-DIRQUSER=OFF` and the slow
  apps off as well, the run printed its `JSON OUTPUT` block and `All is well in the universe.`, and
  the Foundation's `print_results.py` read that serial log against their `PC99_64_noksw_skylake`
  series.

The numbers from those runs are meaningless (TCG's TSC moves in steps of 1,000) and are not
recorded. What the rehearsal proves is the path: network, firmware, GRUB, multiboot2, seL4's boot,
sel4bench's output and its reading.

## The bench: what calef does

Build on the maintainer's Mac first, about four minutes once the Foundation's image is pulled
(`podman`):
```console
$ sh bench/xenon-sel4bench/build.sh /path/to/sel4bench-xenon
```

Then, for each configuration in this order, one power cycle:

1. In one terminal, serve the configuration:
   `script/board-netboot --root /path/to/sel4bench-xenon/esp/noksw`. For the stick instead, copy
   that directory's `EFI/BOOT/BOOTX64.EFI` to the stick's `\EFI\BOOT\BOOTX64.EFI`.
2. In another, capture: `script/board-console --board xenon --for 20m --until none`. The log is the
   artifact.
3. Power xenon on, with the keyboard attached for POST. Nothing else at the machine.
4. The log shows GRUB's `probe:` lines, then `probe: handing over to seL4`, then sel4bench's
   output, ending `All is well in the universe.` after a `JSON OUTPUT` block. There are two ways
   for boot 1 to stop short, and each names the suffix every later boot takes:
   - Silence after the handover: seL4 refused the noksw kernel. Add `-patched`.
   - `Failed to get capability for IOPort range 0x43` after `irquser Benchmarks (iteration 0)`:
     no legacy ACPI root pointer. Add `-noirq` (before `-patched` if both apply:
     `noksw-noirq-patched`).
5. Stop the capture once the board has printed `All is well` or repeated the I/O port error for a
   minute; power xenon off.

| boot | serve | what it answers |
|---|---|---|
| 1 | `noksw` | the published Default row unmodified; the clock probe; whether either fallback is needed |
| 2 | `noksw` with boot 1's suffixes, only if boot 1 stopped short | the Default row |
| 3 | `mcs-noksw` with the same suffixes | the published MCS row |
| 4 | `ksw`, or `ksw-noirq` | the unpatched cross-check against `PC99_64_skylake` |
| 5 | boot 2's configuration with Turbo Boost off in setup, only if the probe showed a ratio above 27 | ticks equal to cycles on xenon |

Three presses of the power button if the published configuration runs and the probe shows the
nominal ratio, at most six otherwise (boot 1 can stop short for one reason and boot 2 for the
other).

## Reading the result

```console
$ git clone https://github.com/seL4/sel4bench-results /path/to/sel4bench-results
$ podman run --rm -v /path/to/sel4bench-results:/r:ro -v "$PWD/target":/log:ro \
    docker.io/trustworthysystems/sel4 bash -c 'cd /r && python3 print_results.py --full --abs \
    /log/board-console-<stamp>.log --base 2026/pc99-skylake/PC99_64_noksw_skylake.jsonl'
```

`print_results.py` needs PyYAML, which the Foundation's image carries. For the MCS boot the base is
`PC99_MCS_64_noksw_skylake.jsonl`, and for `ksw` it is `PC99_64_skylake.jsonl`.

The verdict is fixed here, before the run, so the run cannot choose it. With the probe showing
xenon at the nominal ratio, take the means of IPC call and IPC reply, Default and MCS:

- **Validated**: all four within 5% of the published figure. The two modes are 3.5% apart, so
  5% admits a boot in either. Their spread within a mode is under 2%, and the silicon differences
  above are expected to cost nothing, so 5% leaves room for what is unexplained without hiding a
  real difference.
- **Not yet**: any of the four between 5% and 15%. The method ran but something differs; find
  what before trusting a nife comparison on it.
- **Not validated**: any beyond 15%, or the shape wrong (reply above call, a standard deviation
  ten times theirs).

Notify and IRQ invoke are read but not judged: IRQ invoke goes through the interrupt controller and
the timer, which differ by chipset, and neither enters the thesis's IPC claim.

## What this changes for milestones 127 and 25

If xenon validates, the referee role milestone 127 gives the TX1 is filled on x86_64, today, by a
machine in hand: our way of building, booting and reading sel4bench reproduces the Foundation's
published figures on the same core. The TX1 keeps what only aarch64 silicon can answer:
`PMCCFILTR_EL0` and whether seL4's published 413 and 426 counted their kernel, and nife's own
aarch64 cycle counts. Neither is method validation.

For [milestone 25 (cross-OS performance
comparison)](../../design/roadmap/0025-cross-os-comparison.md) it opens an x86_64 seL4 comparison
on xenon. nife on x86_64 counts unhalted core cycles (`IA32_PERF_FIXED_CTR1`,
milestone 309 (unhalted core cycles on x86_64)), not TSC ticks, so a nife-against-seL4 table on
xenon must put both in one unit:
nife's `bench-probe: cycles_per_tick` line gives the ratio on the same machine. That comparison is
milestone 25's, not this note's.

## BUGS

- Nothing here has run on xenon. The rehearsal is QEMU and OVMF, which proves the boot path and
  the reading of the log, not Dell's firmware, the clock, or the published images past seL4's PCID
  check.
- The scheduler benchmark did not finish in 25 minutes under TCG, so no rehearsal ran the full
  suite. It completes in every one of the Foundation's Skylake runs, so a board run that stalls
  there is a finding, not the emulator.
- The Foundation's clock settings are not published, so a match proves the method only under the
  assumption that their core also ran at its nominal clock. A mismatch near 4.0 / 3.4 (18%) on
  every row is what their turbo would look like.
- The patched configurations are not the published command. They differ from its kernels in six
  bytes of boot code, `cmp -l` of the two kernel files shows exactly which, and that is the whole
  argument that they time the same code.
- GRUB comes from Debian's archive on the day of the build; `RECORD.txt` names its version, and
  nothing pins it.
