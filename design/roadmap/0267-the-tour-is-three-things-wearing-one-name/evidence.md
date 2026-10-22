# Milestone 267 appendix: what the tour costs, who reads its marker, and the proof

An appendix to [milestone 267 (the milestone tour is three things wearing one name, and only one of
them belongs in the kernel)](../0267-the-tour-is-three-things-wearing-one-name.md), moved out of
that page on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. This holds the size measurement of 2026-09-09, the list of `Stage::Tour`'s consumers, and
what the boot printed before and after the narrative was deleted.

## The measurement, first, because it changed what this milestone is about

`script/fastpath-footprint`'s method: release kernels, `llvm-nm --print-size`, `.text` symbols
summed. Measured at `8dd9dbf6`, before anything moved.

| arch | tour build | `--features shell` | what the tour costs |
|---|---|---|---|
| aarch64 | 193,332 | 179,312 | 14,020 (7.3%) |
| riscv64 | 162,964 | 157,136 | 5,828, and it is not the tour |
| x86_64 | 140,842 | 140,842 | 0 |

x86_64 is zero, and had always been. Its boot arm is self-contained and ends in `arch::halt()`,
so the shared milestone tour is unreachable and LLVM had already deleted it. The boot-mode features
have never removed a byte on that architecture.

riscv64's number is a different quantity. That arm halts too, and its `shell` feature swaps its
own arch tour for `user::boot_progenitor`. 5,828 bytes is the difference between two RISC-V boot
paths, not the cost of the 230 lines this block was written about.

So the tour is one architecture's 14 KB. And the symbol-level attribution is the part that
matters, because it says the narrative is not where the bytes are:

| tour-only symbol group | bytes |
|---|---|
| `console_service::start` + `spawn_client` and their closures | ~3,040 |
| the two never-yielding spinner closures in `kernel_main` | ~3,336 |
| `virtio_service::wire` and its closures | ~1,692 |
| `memory_region_service::start` and its closures | ~1,276 |
| `kernel_main` itself (the driving code and the format strings) | +1,460 |
| `pci::find_block_device` | 436 |

Every one of those is a demonstration, and every demonstration is on the [kernel-only
list](../0267-the-tour-is-three-things-wearing-one-name.md#what-was-built). The prose lived in
`.rodata` as anonymous string constants, below the linker's 4 KiB section granularity. As it turned
out, it was below the optimizer's noise floor too: writing the new console-server hoist two
equivalent ways moved `.text` by 340 bytes, which is more than nine lines of text ever weighed.

After the move: 193,996 tour, 179,332 shell. The kernel is 664 bytes larger than it was. That is
the finding and it is worth stating without dressing: the compile-time exclusion was not buying what
its name suggests, and moving the narrative out cannot make it buy less, because what costs bytes is
exactly what cannot leave.

The consequences for the feature pair are in
`design/roadmap/proposals/two-boot-mode-features-for-one-decision.md`. Nothing was retired here.

## The `Reached(Tour)` consumers, since the block asked for them by name

All of them, and none of them observe the thing this milestone moved.

`Stage::Tour` is reached by exactly one line of text, `"nife: the capability core runs on "`, printed
at `kernel/src/main.rs:1401` in the RISC-V arm. The aarch64 tour prints no marker at all. The
consumers are `crates/serial_console/src/progress.rs` (which reaches the stage and labels it),
`watch.rs` (whose `quiet_after` policy exempts exactly this stage, because a boot that ends in `wfi`
is quiet on purpose), `port.rs`, and `xtask/src/main.rs:9383`, which parses `--until tour` on the
command line. Six tests in `board_console` assert on it.

Nothing in that set was touched, and nothing in it could have been: it is the VisionFive 2's boot
that it watches.

## The proof

As built (2026-09-09). `script/server` printed the machine description, then the narrative
arrived from a program at EL0 through a driver at EL0, then the kernel-only demonstrations, then the
progenitor built the system and the prompt appeared. `--features shell` printed the machine
description and went straight to the prompt.

After the deletion (2026-09-13). The same boot with the narrative paragraph missing:
`script/server` prints the machine description, then the kernel-only demonstrations, then the
progenitor builds the system and the prompt appears. `--features shell` is untouched, because it
never compiled the narrative in. The console server still comes up on a tour boot and prints
nothing, which is the Follow-on question on the main page rather than a step in this proof.
