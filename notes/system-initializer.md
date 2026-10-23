# The system initializer: the interactive system, built once

`crates/system_initializer` is everything the progenitor does after its kernel starts it.
`components/src/progenitor.rs` is the one first process on every architecture, and all it holds is
the table of slot numbers its kernel granted (`BootEndowment`). From that table alone `boot` parses
the archive, builds the interactive system and then serves the shell's `run` requests for the rest
of the boot. Nothing host-side can run the crate, because it reaches `user_mode_runtime`'s EL0
`asm!`. `script/swish-check`, which boots and types at the prompt, is what proves it.

This note is shared by the crate's modules. Each section covers one part, and the crate's doc
comments link to the section that carries their design.

## What the boot builds

`boot` builds the console, the input driver, the line discipline, the shell, the terminal's sink
adapter when the archive has one, and `job_undertaker`. It builds them in its own budget, wired with
endpoints and pages it creates. The console is the UART, plus the firmware screen's terminal when one
is granted.
GPU and virtio keyboard grants ride out to the shell untouched, because graphics is launched from
the prompt and never at boot. Then the progenitor stays alive as the spawn service.

It carves `PROGENITOR_RESERVED_PAGES` and a renewable `JOBS_BUDGET_PAGES` off the construction root
and deletes the root. It gives up the UART capabilities once the drivers hold them, and keeps the
file service. The negative control after the delete must see `NoSuchSlot`, not `NotPermitted`.

## The measured load

The progenitor grants nothing of its own to what it cannot vouch for, per §219 (how the shell names
an installed program to the spawner). Every archive program is looked up in
`measured_boot::PROGRAM_MEASUREMENTS`, which the kernel's trust root vouches for. A mismatch or an
unlisted name is a refusal, treated as a missing entry. A refused core component (console, input,
`line_editor`, swish, `job_undertaker`) traps, a refused sink adapter costs that feature, and a
refused spawnable program is left out of the spawn service's table. Logging a mismatch and loading
anyway is refused, because an advisory link is not a chain.

The three-way answer itself is `measured_boot::verdict`, beside `verify_in_manifest`, where host
tests reach it. `measurement::measured` is only an archive read and that call, and
`script/swish-check` proves on both ISAs that the boot makes the call with the table the kernel
measured.

## The spawn service

`spawn_loop::spawn_service` is the loop `boot` ends in. The progenitor is the ELF loader the shell
directs, and it inserts only what the shell endows, so a spawned program reaches nothing its command
line did not name. What it holds for the rest of the boot is `Channels`: two channels, one
supervision endpoint it only delegates from, two bounded budgets and the boot's shared pages and
services. The root construction budget is not among them; it is gone.

The spawn service's details are in two appendices.
[`system-initializer/the-spawn-service.md`](system-initializer/the-spawn-service.md) has the requests,
images, `graphical_terminal`, a normal job's grants and the tidy-up after a build.
[`system-initializer/the-child-layouts.md`](system-initializer/the-child-layouts.md) has the native
and `std` layouts of a child's table.

## BUGS

- A refused `console` or `line_editor`, or an absent core component, traps silently.
- Measurement is checked once, at load; nothing re-measures a running process.
- Pages return LIFO: a reclaimed job region below a live one is a hole until that job ends.
- A `graphical_terminal` session takes `GRAPHICAL_TERMINAL_SESSION_PAGES` of the job pool, so while
  one runs a `std` job or a second session gets "could not spawn". `^C` cannot end a session, and one
  whose build fails after its first driver started may keep those drivers' pages until the boot
  ends.
- Scratch windows are unmapped through slot 28 once a child holds them; a kernel that grants no
  slot 28 leaves them all mapped.
- A boot with a NIC waits for DHCP before the prompt, with no console to say why.
- The capability table is finite, and exhausting it inside `build_child` halts in silence.
  `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` records the peak and the headroom rule, and the
  buy-backs for another permanent capability are in `notes/shared-page-audit.md`.
- A screen-narrowed child is outside `deaths`'s domain for its short life, so a concurrent `ps` or
  `pgrep` does not show it.
- A native child's slot 1 is the input source when there is one and otherwise the `--mem` budget.
  That is safe only because no manifest declares both, and `grant_plan` is where it would stop being
  true (`notes/pipes.md`'s BUGS).
- A `std` child holds `WRITE` on the region it is built in, because that region is its heap. `WRITE`
  on a region is also `SPLIT`, and a split region cannot be destroyed until its children are. So a
  program that splits its own heap pins its job region: `job_undertaker` never reclaims it, and the
  pool is one region smaller until reboot. nife's `std` never splits (its allocator only `MAP`s), so
  this takes a program written to do it. Closing it needs a right that allows `MAP` and not `SPLIT`,
  which is the syscall surface and an architect's call.

*Name: provisional. Named for the crate it describes. calef names files.*
