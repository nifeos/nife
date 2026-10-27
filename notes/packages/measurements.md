# Packages: what proves the producer, and what running and installing cost

*An appendix to [`notes/packages.md`](../packages.md), which carries the design and is written so a
reader can act without opening this file. This one holds the producer's and the installer's proof
lists and the measured costs of running an installed program by its bytes and of installing one.
Name: provisional (2026-09-26).*

## What proves the producer

- 11 host tests in `crates/package_archive`, over the round trip, the refusals, alignment,
  determinism, and four ways a hostile file is refused rather than indexed.
- 4 host tests over the recipe parser in `xtask/src/package.rs`.
- `fuzz/fuzz_targets/package_archive_roundtrip`, the first half of §197 (a package is one archive file)'s accepted debt. 1.39
  million runs in 46 seconds on the development Mac on 2026-09-23, no crashes.
- Two Kani harnesses in the crate, the second half of that debt: a file the solver chose is
  either refused or reads only inside itself, and a file shorter than the header is refused rather
  than indexed. Both discharge, in **4 seconds** together, which is the row `script/verify`'s table
  now carries. **The first run failed and the failure was the bound, not the code**: at
  `#[kani::unwind(4)]` the eight-byte magic comparison reports an unwinding assertion inside
  `<builtin-library-memcmp>` and leaves 270 of 271 checks undetermined, which reads exactly like a
  refuted proof. 9 is the bound that covers it, and the reason is recorded at the attribute rather
  than here, because that is where the next person raising it will be looking.
- The end-to-end run above, which is the first package this project has produced.

## What proves the installer

Moved verbatim from the parent's "What proves it" on 2026-10-13 (UTC) under §212 (a prose budget).

`script/swish-check` boots twice per architecture against one disk, typing the first transcript
above and then the second (`SWISH_CHECK_AFTER_REBOOT`). The host puts a package on the disk and
installs nothing (`seed_installed` in `xtask/src/disk.rs`). The tampered copy has one program byte
flipped and its table of contents rewritten to agree, so the catalog is the only thing that can
refuse it. Green on aarch64, riscv64 and x86_64 (under OVMF) on 2026-09-26. Each line was falsified
once on aarch64:

- skip the catalog check, and the tampered package installs;
- record the program's digest with one bit flipped, and the installed program is refused;
- give the second boot a fresh disk, and every line after the reboot fails;
- make `remove` rewrite the old table, and the removed program still runs;
- make `rollback` stay on the live generation, and the last line is refused.

## What running by bytes costs, measured 2026-09-26

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `uptime`, stripped | 89,168 bytes, 22 frames | 49,168 bytes, 13 frames | 28,352 bytes, 7 frames |
| Messages on the spawn endpoint | 1 `SEND` + 22 `SEND_CAPABILITY`s | 1 + 13 | 1 + 7 |
| File-service calls by the progenitor | 8 (3 opens, 2 reads, 3 closes) | 8 | 8 |
| Pages staged on each side, returned after | 22 | 13 | 7 |
| Progenitor capability peak | 23 of 24, unchanged | 23 of 24, unchanged | the hand-over mark only |

At most one of the caller's frames is in the progenitor's table at a time, beside the staging
region. The gauge reports the boot's high-water mark, so it shows the image path stays under the
peak without measuring the path itself.

One-time costs are the shell's primer page with its page tables, and the progenitor's mapping of the
file page. Per image spawn, the progenitor's scratch window advances a page per frame, which costs a
page table from its own budget about every 23 `uptime` runs on aarch64. Every child already pays
that debt (§162 (whether a holder can give up a mapping)); this pays it about half again as fast.

Two orderings are load-bearing, because a region returns its pages to its parent only if it is the
parent's most recent carve (`memory_regions`' `return_to_parent`). The progenitor splits the child's
region before the staging region, and the shell maps one primer page once so the window's page
tables exist before any staging region does. Either one wrong silently strands every staging page.

## What installing costs, measured 2026-09-26

`package install` on a disk that has never had one, from `script/swish-check`'s own runs:

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| Package file | 90,491 bytes, 23 frames | 50,491 bytes, 13 frames | 29,675 bytes, 8 frames |
| Messages on the spawn endpoint | 1 `SEND` + 23 `SEND_CAPABILITY`s | 1 + 13 | 1 + 8 |
| File-service calls by the progenitor | 51 | 42 | 36 |
| Progenitor capability peak | 23 of 24, unchanged | 23 of 24, unchanged | the hand-over mark only |

The 29 fixed calls on a first install were counted from the code, not traced. They make three
directories under `packages/` and the activation directory, probe for a `current` that is not
there, create and write the generation, stage `current` and rename it, sync, and close. The rest is one `WRITE` per page of the
program, 22 on aarch64. A later install skips the `MKDIR`s and reads the live generation instead.
`remove` sends two messages and costs 20 calls; `rollback` sends one and costs 17. Counted from the
code rather than traced.

Prompt to prompt under TCG, at the gate's 100 ms polling grain: 0.73 s on aarch64 and 11.7 s on
x86_64 under OVMF for the genuine install, 0.3 s and 7.2 s for the refused one. These are the
emulator's numbers, not the system's, and they are here so a regression by a factor is visible, not
as a performance claim.

## What fetching costs, measured 2026-09-26

`package install greeting` against the gate's package source, from `script/swish-check`'s runs:

| | aarch64 | riscv64 |
|---|---|---|
| Messages on the spawn endpoint | 2 `SEND`s (the request and the name) | 2 |
| Package file | 82,715 bytes | 28,291 bytes |
| Prompt to prompt under TCG | 1.28 s | 0.76 s |
| Refused by the catalog (`nosuch`) | 0.11 s, no connection | 0.11 s |
| Refused by digest (the lying `uptime`) | 0.22 s | 0.22 s |
| Progenitor capability peak | 23 of 24, unchanged | 23 of 24, unchanged |

The fetch holds at most three capabilities of its own at once (the socket page's region, a page
between its retype and its delete, and the staging region), the same as a file install's staging
region, frame and page. The times are single runs at the gate's polling grain and are emulator
numbers, like the table above.
