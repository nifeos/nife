---
status: BUILT
promoted_from: pin-the-hot-trap-paths-placement
raised: 2026-10-05
built: 2026-10-20
milestone_dependencies: none
decision_dependencies: none
machine_requirements: riscv64 silicon
specific_machine: radon (the effect was measured there, and the acceptance sweep is radon's)
needs_person: yes
---
# 796. Pin the hot trap path's placement, so a radon per-crossing number stops moving with unrelated code

Raised 2026-10-05 (UTC) by `lane/radon-2026-10-05-record`, recording that evening's radon session.
Title and slug are drafts. `needs_person` is yes only because the acceptance sweep is boots on
radon.

BUILT 2026-10-20 (UTC). Option 1 is built on all three ISAs and gated in CI, and the radon sweep
read the one-task `null_syscall` flat at 121 ticks in all four images ("The 2026-10-20 sweep"
below). The design, the before and after, and the `BUGS` are in
[`notes/benchmarks/kernel-footprint-and-caches.md`](../../notes/benchmarks/kernel-footprint-and-caches.md),
"The hot section".

## What was seen

[`notes/job-mix/radon-2026-10-05.md`](../../notes/job-mix/radon-2026-10-05.md#why-one-task-rose-from-99-to-110):
`main` built four times with milestone 370 (a layout control)'s knob (`fastpath_pad`, `NIFE_FASTPATH_PAD=0`,
`NIFE_FASTPATH_SHIFT` 0, 16, 32, 48), which moves the whole kernel text by N bytes and changes no
code. The job mix's `null_syscall` at one task read 111, 118, 116 and 110 ticks of radon's 4 MHz
timer. An 8-tick (7%) spread with no code change. A bisect had already put a 16-tick one-task
step on a merge (#1659) that added no instruction to the kernel's trap path.

`trap_entry`'s own offset is not the variable: offset 40 mod 64 was the best layout in the bisect
(99, 100) and the worst in the sweep (118). The U74's L1 instruction cache is 32 KiB, two-way,
256 sets indexed by virtual address bits 6 to 13 (`script/fastpath-footprint`'s `--layout`
comment, citing the core manual). Three hot pieces of code landing on one set (the kernel's entry,
its dispatch and lookup, and the userspace stub, which shares the index bits) would evict each other
on every trap. That is the leading reading, and it is unmeasured.

## Why it matters

Every single-crossing number on radon is a comparison between builds, and until this is fixed any
difference under about 8 ticks is noise from where the linker happened to put things. Fatal risk 4
is argued in those numbers. On 2026-10-05 the cost was concrete: a one-task guard written before the
run fired on placement, and an evening of boots went to proving that.

## What the tree already does

- Milestone 370 pins one section, `.text.fastpath_pad_body`, first in `.text` after the boot stub
  (`kernel/link-riscv64.ld`), so its knob moves everything after it uniformly. Nothing else in
  `.text` is placed: `*(.text .text.*)` takes input order.
- `script/fastpath-footprint --layout` already names the hot set (both fast-path closures plus the
  entry set), prints each symbol's address and L1i set on riscv64, and hashes its instructions.
  That is the list to pin and the instrument to check the pin with.

**Reuse:** milestone 370's pinned section and linker-script pattern, and `script/fastpath-footprint
--layout`'s hot set and set indices, both taken. No outside tool considered; the work is a linker
script and a list.

## Options (option 1 taken, 2026-10-07)

1. One output section for the hot set, placed and aligned. Put the `--layout` hot symbols in
   `.text.hot` (by `#[link_section]` on the Rust functions and a section directive on the entry
   assembly), first after the boot stub, aligned to the L1i way size (16 KiB), in a fixed order. The
   kernel's half is then the same addresses in every build. Cost: a list of symbols someone must
   keep in step with the code, which `--layout`'s closure can check; and inlining decisions can
   still pull hot code out of the section.
2. Option 1 plus the userspace stub at a set range the kernel's hot section does not use. The
   stub's placement comes from `crates/user_mode_runtime/link.ld`. Needed only if option 1 alone
   leaves a spread.
3. Measure only. Control layout per experiment with milestone 370's knob, sweeping every time.
   Costs four boots per comparison forever; it is the status quo plus discipline.

This lane recommends 1, then 2 only if the sweep says so. It has not measured which pieces collide.

Option 1 is what landed. Every symbol in `--layout`'s hot set carries a `.text.hot.<name>`
section; each linker script places them `SORT_BY_NAME` from a 16 KiB boundary right after the boot
stub, ahead of milestone 370's sled. On riscv64 the hot path went from 151 lines over 10 pages, two
L1i sets holding three hot lines, to 139 lines over 3 pages with none. Inlining is covered by the
gate rather than by hope: an out-of-line callee that joins the path and is not in the section fails
`script/fastpath-footprint`. Option 2 is not taken. The sweep could not have asked for it, since it
moves kernel text only and the userspace stub sits still in all four images.

## Done means

- [x] The hot set's addresses and L1i sets identical across `main` and a build with unrelated code
  added ahead of it (a `script/fastpath-footprint --layout` diff), in a check that fails the build
  when they move. Built as two checks in `script/fastpath-footprint`, which CI runs. One requires every hot
  symbol inside `.text.hot` on a 16 KiB boundary (all three ISAs). The other builds two kernels
  20,520 bytes of text shift apart and requires every hot symbol at the same address and size
  (aarch64 and riscv64).
  Each was broken on purpose and failed.
- [x] On radon, the 2026-10-05 sweep repeated (shifts 0, 16, 32, 48) reads `null_syscall` at one task
  flat within 2 ticks, where it read 110 to 118. Read 121, 121, 121 and 121 on 2026-10-20, a spread
  of 0 ticks ("The 2026-10-20 sweep" below).
- [x] `script/fastpath-footprint` and `script/bench` within their floors on all three ISAs; the same
  pin applied, or a scope note recording why not, on aarch64 and x86_64. The pin is on all three.
  Scope note: x86_64 has no `fastpath_pad`, so it gets the containment check and not the shift
  check. Footprint drift after the pin: within 2.8% on every figure (local, 2026-10-07); CI reads
  `script/bench`.

## The radon sweep, for calef

Run on patagonia from a checkout of `main` with this milestone merged. Power is calef's alone:
**smart plug 2 only** (radon), never plug 3 (garcia), never the USB hub. Dates below are the day
of the run, UTC.

### Build the four images, and check them before walking to the bench

Each `fastpath-footprint` line must say `18 of 18 hot symbols inside` and print the same `trap_entry` address and set.

```sh
D=$(date -u +%Y-%m-%d); mkdir -p bench/radon-$D
for S in 0 16 32 48; do
  NIFE_FASTPATH_PAD=0 NIFE_FASTPATH_SHIFT=$S \
    script/fastpath-footprint --arch riscv64 --features board,job_mix,fastpath_pad --layout \
    | grep -E 'pin:|  trap_entry'
  NIFE_FASTPATH_PAD=0 NIFE_FASTPATH_SHIFT=$S \
    script/board-image --job-mix --tftp --extra-features fastpath_pad
  rm -rf target/board-shift$S && mv target/board target/board-shift$S
done
shasum -a 256 target/board-shift*/nife-vf2.img | tee bench/radon-$D/payloads.sha256
```

### Each boot

N = 1 to 4 in the order shift 16, 0, 48, 32 (16 was the worst on 2026-10-05, so it
goes first rather than last). Two terminals in the same checkout:

```sh
# terminal A, the server (Ctrl-C the previous one first)
N=1; S=16
script/board-netboot --root target/board-shift$S 2>&1 | tee bench/radon-$D/boot$N-shift$S.netboot.log

# terminal B, the console, started before power
N=1; S=16
script/board-console --for 30m --until none --log bench/radon-$D/raw-boot$N-shift$S.log
```

Power-cycle smart plug 2, wait for `job-mix: done`, end the capture, then:

```sh
LC_ALL=C tr -cd '\11\12\15\40-\176' < bench/radon-$D/raw-boot$N-shift$S.log > bench/radon-$D/boot$N-shift$S.log
LC_ALL=C grep -a -E 'job-mix: done|FAILED|REFUSED|tasks=1 kind=null_syscall' bench/radon-$D/boot$N-shift$S.log
grep -a 'nife-vf2.img' bench/radon-$D/boot$N-shift$S.netboot.log
```

A boot counts only with `job-mix: done` and a netboot log naming that boot's root. If the board
announces another server address, `setenv nife_boot_server 192.168.8.138` then
`source ${scriptaddr}` at the U-Boot prompt, as on 2026-10-05.

### Reading it, written before any boot

The number is `per_job` on the `tasks=1
kind=null_syscall` line.

| Spread of the four | Reading | Next |
|---|---|---|
| 2 ticks or less | the pin holds the crossing still; Done means is met | mark BUILT with the four numbers |
| 3 to 5 | the pin took most of it; something on the path is outside the closure (a callee the walk does not follow, or data that moves with `.text`) | stays PARTIAL; find it with `--layout` before boots |
| 6 or more | the pin is not what moves the number | stays PARTIAL; the 2026-10-05 reading is wrong, record that |

Option 2 is not on this table on purpose: the sweep never moves the userspace stub. Whatever the
spread, also record the one-task number itself against 2026-10-05's 110 to 118, since a contiguous
hot block is a different layout from all of them.

### The 2026-10-20 sweep

Four boots on radon, 19:01 to 19:10 UTC, calef cycling plug 2 for each, from images built at
`b43f19e3c`. Each image's `fastpath-footprint` line read `pin: .text.hot 9082 B at
0xffffffc080204000, 0 B into a 16 KiB way; 18 of 18 hot symbols inside`, with `trap_entry` at
`0xffffffc080206270`, set 137, in all four. Every boot was netbooted (`payload came from net`) and
ended `job-mix: done`. Logs, hashes and two departures from the procedure above (the console
capture tool, and a server address typed at U-Boot) are in
[`bench/radon-2026-10-20/`](../../bench/radon-2026-10-20/README.md).

| Boot | Shift | 1 task `per_job` | 1 task ticks over 336 jobs | 4 tasks `per_job` |
|---|---|---|---|---|
| 1 | 16 | 121 | 40736 | 131 |
| 2 | 0 | 121 | 40707 | 131 |
| 3 | 48 | 121 | 40686 | 130 |
| 4 | 32 | 121 | 40693 | 130 |

**Reading.** A spread of 0 ticks, so by the table above the pin holds the crossing still. The raw
ticks differ by 50 of about 40,700 (0.12%), under one tick per job.

**Against 2026-10-05.** 121 is above that sweep's whole range of 110 to 118. That is not a
regression on this evidence. A contiguous hot block is a different layout from all four 2026-10-05
images, and those were built from `c8b5fd09e`, two weeks of merges before `b43f19e3c`. Telling the
pin's cost from those merges' would take a pinned and an unpinned image of one commit, booted
together; nobody has. Both sweeps ran debug userspace under a release kernel (`script/board-image`'s
`BUGS`; milestone 797 (release userspace in board images) re-baselines every radon number).

## Follow-on

- **Recorded.** Five limitations, in the `BUGS` of
  `notes/benchmarks/kernel-footprint-and-caches.md`. The userspace stub is unpinned and alignment
  inside the block is not controlled. RISC-V call relaxation can change hot code size past 1 MiB of
  text. x86_64 has no shift check. aarch64 and x86_64 carry a little cold code in the block.
- **Recorded.** Option 2 (the userspace stub at a set range the kernel's block does not use)
  is held until a cross-build comparison shows the stub moving the number; a kernel-text sweep cannot.
  Checked 2026-10-07: `crates/user_mode_runtime/link.ld` is unchanged by this lane.

## Index row

A merge that added no instruction to the trap path moved radon's one-task `null_syscall` by 16 ticks, and four builds that only shifted kernel text spread by 7%. Pinning the hot trap path's placement stops a per-crossing number from moving with unrelated code.
