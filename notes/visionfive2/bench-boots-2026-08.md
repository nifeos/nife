# Bench boots 1 to 14, August 2026

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md), beside its BUGS section. It is the
boot-by-boot record of first silicon and the limitations found on the way.

## Boots 1 to 11, 2026-08-14 and 2026-08-15

The kernel first ran on this board on 2026-08-14. It got through its banner (DW-8250 console, DTB
parse, paging, traps, timer, frame allocator) before panicking on the QEMU PCIe constants.
[jh7110-against-virt.md](jh7110-against-virt.md) records that failure and its fix.

The second boot that day carried the PCIe fix. It got through fine paging and ISA discovery, and
died starting hart 0. The vendor tree's `status` lie walked the S7 past the disabled-hart handling,
and vendor OpenSBI crashed rather than refuse the start. That is the second stop in
[hart-roster-stops.md](hart-roster-stops.md).

The boots after that carried the supervisor rule and the PLIC context map through, and hit the
third stop. The first non-contiguous online set this kernel ever ran on exposed every
count-as-index cpu loop at once. Spawn placement put `init` into parked slot 0's inbox first. It
took three boots to diagnose, was fixed in `1329874`, and was swept after.

The supervisor rule and the context map are QEMU- and host-proven, and board-proven as far as
those boots reached. The online-set sweep's {1,2,3} shape is host-proven in `crates/cpu_set`.
Whether the fixed placement carries the board through the demo was the next bench boot's fact. Like
everything QEMU cannot prove, that is what "To measure at the bench" is for.

Boot 7 was the first with the placement fix and the cross-hart `fence.i`. It appeared to hang a
fourth way, in a state the transition audit said no legal path produces. Boot 8's instrumented dump
was read as catching an undelivered wake, and the undelivered-wake gate was built against it
([boots-7-and-8.md](boots-7-and-8.md)).

The fifth stop (2026-08-15) overturned that reading ([boots-9-and-10.md](boots-9-and-10.md)). The
dumps of boots 7 through 9 show the terminal state of a *completed* tour. The parked receivers are
the UART demo's driver and byte receiver. The wake was the worker's real send. svc=20 is the
choreography's exact total. So no undelivered wake, no fabricated state and no corruption have been
observed on this board. The gate and the pop-own-current guard stay as hardening, with their
injection tests. Their origin story is corrected in notes/scheduler.md.

The fifth stop left one thing open. The tour's serial lines after "init : measured, built, started"
were absent from the bench record, while the machine state proves the printing steps ran. Boot 11
carried the discriminators: tour stage in every dump header, `tx=` in the diag line, `serve:` ring
events, and the registry canary. The first move was to re-examine the boot 7 through 9 captures for
the missing lines.

## Boots 12 and 13 closed the story, as the measured-boot pair

Boot 12, on 2026-08-15, was the first under the name `nife`. It cleared the whole bring-up and was
refused at the trust boundary:
`MEASURED BOOT REFUSED: 'program_measurements' is not what this kernel image was built against`.
The kernel halted rather than hand the archive to init. The mismatch was real and ours.
`script/board-image` built the kernel before packing the archive that regenerates the manifest the
kernel compiles in. So the kernel on the card vouched for the *previous* archive. QEMU never hit it,
because `xtask` orders those steps correctly. The fix swapped the script's order, and its comment
carries this story.

Boot 13, with the pair built in order, ran the tour to the final banner. Init measured and built
`worker` from the 6,294,016-byte archive (child sent 81, expected 81). Preemption ran two
never-yield threads 3.3M and 28.3M iterations, with 61 preemptions. Every discriminator boot 11 was
instrumented for reported a healthy machine. The five diag dumps show `svc=20` frozen, identical
event rings, and the two parked receivers of the demo's terminal state.

Two observations from 13 were recorded rather than chased. First, the early `scheduler :` smoke
line reported `0 of 2 kernel threads ran`; boot 12 said 1 of 2. The preemption numbers prove
scheduling, so the smoke line races real timing and its wording overclaims. That was fixed
2026-08-15. The check was four yields on the boot hart, a yield count rather than a duration, which
the other harts outran. It now waits clock-bounded, two seconds, until both threads have run. The
success wording prints only then, with a loud FAILED line on the timeout path. So the next bench
boot should read `2 of 2 kernel threads ran`.

Second, a key press at the prompt did nothing. That confirms on silicon the UART-IRQ limitation
below: the driver armed line 10, and the board interrupts on 32.

The refusal followed by the pass is the measured-boot demonstration end to end. The same board
refused the wrong pair and ran the right pair.

## Boot 14 put all four U74s online

Boot 14 (2026-08-15) was the first boot with the eight-seat, by-hart-id kernel, the revived
fifth-hart work. It printed `5 core(s) in the device tree, 4 startable`, the S7 exclusion line with
its reason, then `4 core(s) online`. The tour ran straight through to the banner with nothing after
it. The stage-gated watcher stayed quiet, exactly as a finished tour should have it. Measured boot
passed silently.

The preemption line is where the fourth core shows. It recorded 82 preemptions. The two never-yield
threads' iteration counts landed far closer together (6.5M and 7.4M) than boot 13's three-core
spread. The `scheduler :` smoke line said `0 of 2` again, its second board boot in a row. That
line's wording overclaims on real timing, and it now has enough evidence to be a lane, not a shrug.

## Three limitations found while building those

These were honestly not fixed that night. The first has since been closed, and its entry carries
the record.

- The tour's UART-driver step armed QEMU's interrupt number on the board. `main.rs` passed
  `UART_IRQ = 10` (QEMU `virt`'s NS16550 line) to `riscv_uart_driver_demo`. That binds it and
  enables that PLIC source. On the JH7110, UART0 interrupts on line 32 [dtsi]. So the board build
  enabled an unrelated source, and the driver could never receive a real keystroke there. It was
  quiet in practice that night: source 10 never fired, or the driver's dump row would show it
  running rather than parked. It was not the fifth stop's bug. But the number needed to come from
  the device tree, like everything else on the parent page, before the driver demo meant anything
  on silicon. Boot 13 confirmed it on silicon on 2026-08-15: a key press at the completed tour's
  prompt reached nothing, exactly as this entry predicted.

  It was fixed on 2026-08-15, the same day boot 13 confirmed it. The number now comes from the
  machine's own tree. `memory::init` reads the console node's `interrupts`. It resolves the
  inheritable `interrupt-parent`: the serial node's own on QEMU riscv64, the root's on QEMU
  aarch64, `/soc`'s per the mainline dtsi. It asks the controller that phandle names for its
  `#interrupt-cells`. Then it decodes the entry per that count rather than assuming it
  (`isa::interrupt_id`). One cell is a PLIC source verbatim. Three are the GIC's
  `<type number flags>`, with the bank base added.

  Host tests hold the whole claim (`crates/machine_discovery/tests/interrupt_ids.rs`). The same
  read answers 10 on QEMU's tree and 32 on both JH7110 fixtures. It answers 33 on aarch64 `virt`,
  where `UART_RX_INTID = 33` was the same bug one board away. That was fixed in the same motion:
  `user::boot_progenitor` now asks the tree first. The constants survive as the documented fallback
  for a tree that does not say. Every boot path prints a `uart irq` line naming which source won, so
  the next bench transcript answers this question instead of raising it. What QEMU cannot prove, as
  ever, is that a keystroke at the board's prompt now reaches the driver. That is the next bench
  boot's fact. The `uart irq    : source 32 (device tree)` line in its transcript is the first thing
  to read.

- The shell path's userspace input driver still speaks QEMU's UART layout.
  `components/src/input.rs` reads the NS16550 at byte-stride offsets (LSR at 0x05). So on the board
  the kernel console will print, but the interactive shell's input path reads garbage until that
  driver learns the same shape the kernel driver did. It is not on the first-boot path, since the
  tour and test builds take no input. It bites at the shell milestone.

- `cpu::MAX_CPUS` was 4 against this SoC's five described harts. It was fixed 2026-08-14. The
  constant is 8, and the roster seats each core at the slot its own hart id names
  (`smp::read_cpu_list`). So the unusable S7 occupies only slot 0, and U74 hart 4 sits at slot 4
  instead of falling off a positional truncation. The logical-id-equals-hardware-id assumption is
  untouched. Seating by id makes it hold by construction. A hart whose id has no seat gets a named
  line and stays parked: the same refusal as before, earlier.

  It is QEMU-proven at `-smp 5` and `-smp 8`, with every described hart online. It is host-proven
  against both JH7110 fixtures, with the startable set exactly {1,2,3,4}. What QEMU cannot prove is
  the fourth U74 actually running this kernel's code beside the other three on silicon. That was
  boot 10's fact. Its banner should read "5 core(s) in the device tree, 4 startable" plus the cpu 0
  exclusion line, then "4 core(s) online" (three secondaries beside the boot hart). That is correct
  arithmetic and, for the first time, the whole machine. Boot 14 above is the boot that showed it.

- The boot script had never run on the board. It was written 2026-09-02 from the two captured
  transcripts, with radon powered down and unreachable. So every claim about it was reasoning plus
  a byte-level check of the image format on the host. Three things only a bench boot could settle,
  and `design/roadmap/0218-hands-free-board-boot.md` carries the ordered procedure. Does this vendor
  U-Boot's distro boot scan for scripts at all? Is `scriptaddr` set in the default environment it
  falls back to? That is the same environment whose missing `fdt_addr_r` is why the extlinux path
  failed. And does its parser accept the seven lines as written? Every one of those failures leaves
  the board at the `StarFive #` prompt rather than hung. That is already better than what it
  replaced, and the manual commands still work from there. (It ran on 2026-09-03; see
  [read-off-the-board-2026-09-03.md](read-off-the-board-2026-09-03.md).)

## Two standing caveats

The `text_offset` in the Image header encodes one board's DRAM base. The header comment in `boot.s`
and [jh7110-against-virt.md](jh7110-against-virt.md) carry the caveat.

Everything cited from "mainline" (Linux dtsi, U-Boot doc and source) describes current upstream. The
flash on the board runs StarFive's vendor fork of unknown vintage. The relocation logic was verified
in the vendor branch too. The environment defaults were not, which is why they sit on the bench
list.
