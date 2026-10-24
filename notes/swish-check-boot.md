# A swish-check leg: what each boot proves, and what the checks read

Name: provisional, minted 2026-10-24 (UTC) by lane comments/929-xtask-03. Named for
`xtask/src/swish_check/boot.rs`, whose design it carries.

`script/swish-check` boots the system on one architecture and types scripts at its prompt. Each leg
is up to four boots. This note holds the reasons behind each boot's shape and behind the boot
claims the leg reads. The code keeps the constraint beside each line.

## The x86_64 leg boots under firmware

The x86_64 leg boots under OVMF, from the same `\EFI\BOOT\BOOTX64.EFI` that `cargo xtask
uefi-image` stages for a USB stick. It does not use QEMU's PVH `-kernel` loader. That image is the
thing a customer boots (DECISIONS §157 (a trivial install is a web page, a USB drive and
packages)), so it is the one worth typing at. The loader, the firmware's memory map and ACPI
tables, and the console server's tee onto the firmware's screen (milestone 400 (the shell on the
firmware's screen)) are all on that path, and the PVH boot has none of them. Milestone 182
(x86_64's own interactive-boot entry point) records what OVMF costs over PVH.

`q35` has no virtio-mmio bus, and every entropy device the kernel can find is virtio-mmio. The
kernel therefore builds the progenitor's entropy service on the CPU's `RDSEED` instruction, which
the runner's `-cpu max` implements (milestone 595 (provisional)). The leg asserts the progenitor
said so, because a boot that fell back to no entropy would otherwise surface only as the `uuid`
lines failing.

The x86_64 leg runs on KVM when the host offers a writable `/dev/kvm`. The aarch64 and riscv64 legs
stay on TCG on purpose: on CI's arm64 runner, TCG puts their kernels in front of a weakly ordered
host (ci.yml's header). Those two legs need no speed (0.2 seconds a line). The x86_64 guest is TSO
under either accelerator, so KVM gives up no ordering that leg was ever shown. Milestone 628 (the
x86_64 swish-check leg costs what the others do) measured what KVM buys, in
notes/benchmarks/swish-check-x86-leg.md.

## The USB keyboard boot

The fourth boot attaches an xHCI controller and a USB keyboard and nothing else that could type. It
types `echo hello` on that keyboard through the QEMU monitor's `sendkey` and requires `hello` back.
It runs on all three architectures, each behind its IOMMU (`iommu=smmuv3`, `riscv-iommu-pci`,
`intel-iommu`), because the kernel refuses to hand the controller over without one.

It proves the chain end to end. The kernel found and confined the controller. The EL0 driver reset
it, enumerated the keyboard and configured its interrupt endpoint, and the `usb :` line says so
before the prompt. The progenitor delegated the line discipline's endpoint to the driver. A key
pressed on the device reached the shell as the same bytes a serial line sends. The serial line
types nothing in this boot, so the echo can have come from nowhere else.

The keyboard enumerates at full speed (`usb_version=1`), the speed real keyboards use. QEMU's
`usb-kbd` defaults to high speed on an xHCI, which takes a different endpoint 0 packet size and a
different interval encoding. At full speed the Evaluate Context step and the frame-based interval
encoding run under QEMU rather than first on the bench. On x86_64 the controller offers MSI with no
MSI-X, as an Intel PCH's xHCI does, so the kernel's MSI fallback delivers the interrupt.

It does not prove a real keyboard on real silicon, which is calef's bench step in milestone 242
(USB host and HID, because on commodity hardware the keyboard is not a UART). It does not prove a
hub, or a low-speed device, which QEMU cannot model.

## The keyboard boot and its NIC

The scriptless keyboard boot is meant to carry no NIC. It types nothing that reaches the network,
and a boot with a gpu, a keyboard, a virtio-rng and the NIC all attached peaked at 30 of 32
capability slots (measured 2026-10-03 UTC). Without the NIC it is the configuration milestone 632
(graphics on demand) measured: gpu, keyboard and rng, at 28.

The guard in `swish_check_runner` removes nothing, though. `host::cargo` sets `NIFE_NET=1` in
xtask's own environment and the runner's `Command` inherits it. notes/capability-peak-trace.md
found `net_stack`'s endpoint among the keyboard boot's 31 slots; without it the boot reads 30.
Removing the variable would change what the gate boots and what `CAPABILITY_TABLE_PEAK_MEASURED`
records, so it is left for whoever owns that constant.

## The progenitor's boot claims

The progenitor prints a positive or a negative sentence for each of four claims, and the leg
requires the positive one. Each claim is in the transcript before the banner, because the banner
comes from a shell the progenitor starts afterwards.

- **It gave the construction budget away** (milestone 22 (trusted init)). It prints the line after
  deleting the root untyped, and only when `RETYPE` and `RETYPE_OBJ` on that slot both answered
  `NoSuchSlot`: the capability is gone, not narrowed. The complaint says only which string was
  missing. All the check knows is whether a string is in a transcript, so naming a capability state
  would claim evidence it does not have.
- **It measured every program it loaded** (milestone 104 (the measurement continues past init)). A
  kernel built without the measurement step refuses to boot. A table that stopped naming programs
  would leave a system that boots, prompts and vouches for nothing, and looks exactly like a healthy
  one. The negative sentence names the programs it refused.
- **Its directory grants go through a bound badge** (milestone 606 (walk cost), calef's rulings D
  and T1 of 2026-09-27). Every leg's image carries `redoxfs_server`, whose bytes carry the
  subtree-grants note. A progenitor that lost the note falls back to a caretaker and says so.
- **It has an entropy service from this machine's source** (milestone 595 (provisional)): a
  virtio-rng on the two `virt` machines, `RDSEED` on `q35`. Without it the `uuid` and
  `std_exerciser` lines fail as a refused draw several lines on, which says nothing about why.

## Nothing may die

The leg fails if the kernel reports killing a user thread anywhere in the transcript, the typed
script included (milestone 233 (`login` dies on every boot, and the boot says it is ready)).
Nothing in `SWISH_CHECK_SCRIPT` traps on purpose. Its three failing lines (`wc` and `doc` with
nothing named, `least_authority_demo` with no argument) are refusals: two at the prompt before
anything is spawned, and one an ordinary non-zero exit. `rm gate.txt`'s refusal is an answer, not a
fault. `echo $?` reading `2` right after `least_authority_demo` proves the distinction, since a
thread the kernel killed does not set a status.

What a deliberate trap does was measured for milestone 233. `least_authority_demo` was patched to
call `supervision_protocol::fail()` on `least_authority_demo 5`, and the gate was run against it.
The check fired, naming the thread and the reason, so it can fail. The prompt also never came back
for `least_authority_demo 6`. The shell waits on the result endpoint of a job that faulted instead
of sending, and nothing wakes that wait, so a spawned command that traps hangs the shell. That
limitation belongs to the shell under `components/src/swish/`.

The check reads the first two of `KERNEL_FAULT_TOKENS`, the same pair `kernel_wrote_during_boot`
reads. A fault during the boot is both a failure here and the one thing that can shuffle a console
line, so the two checks see one fact from two sides.

## The capability-slot gauge

The gauge line must be present, and it must not say `ABOVE`. A gauge that stopped printing is one
nobody would miss until the table filled again. `ABOVE` is the kernel's word for a boot past the
peak `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` records. That constant is a measurement, not a
target, so the check fails on a recorded fact going stale. When it fires, measure, update the
constant, and re-read the headroom arithmetic beside `CAPABILITY_TABLE_SLOTS`.
