# The progenitor's stack, measured

*Name: provisional, for this stem. Naming is calef's.*

The progenitor is the first process, and its stack lives for the whole boot because the spawn
service runs inside `system_initializer::boot`. It ran on eight pages (32 KiB), a size whose doc
called it "generous", and nothing measured it. Three lanes found the limit by overflowing it:
#1360, #1374, and milestone 205 (how a foreign program is told what to do) in #1402, which faulted in a debug build at `package install uptime`
after adding 416 bytes to `spawn_service`'s frame. This page records the instrument that measures it
now, the numbers it read, the gate, and why the fix was a raise rather than a trim.

## The instrument

`kernel/src/progenitor_stack.rs`. `boot_progenitor` paints each stack page with milestone 84 (stack high-water: measure kernel stack depth)'s paint
word as it maps it, before the progenitor runs, and remembers the page's kernel address. The
scheduler's idle loop scans from the lowest page up for the first word that is not paint, which is
the deepest the stack has ever been (notes/stack-high-water.md explains why a paint outlives the
frame that destroyed it). Once that mark has been still for sixteen idle passes, the kernel prints
it, once per new peak:

```text
  progenitor stack: 32440 of 49152 bytes at peak, 16712 spare
```

Every architecture speaks from the idle loop. `x86_64` spoke from the yield syscall, one yield in
256, until milestone 505 (an x86_64 input driver that never lets the core idle): its input driver
polled and yielded, so the idle loop stopped running at the hand-over.

The kernel reads the stack, not the progenitor. That keeps the instrument out of
`crates/system_initializer`, which four lanes were editing when this was built, and it means a
progenitor about to overflow is not the program asked to measure itself.

`script/swish-check` takes those lines out of the transcript as they arrive (a `GaugeFilter` in
`xtask/src/swish_check/gauge.rs`), because on a healthy boot they land between a `$ ` and the next line's
echo, and every reader there assumes those two are adjacent. It echoes each one with the command it
followed, so one run is a per-path measurement.

## The numbers

Bytes used at peak, out of 32,768, on `script/swish-check` (2026-09-27, `main` at `92e73a51f`).
Release is `script/swish-check --release`, added for this; `x86_64` release was not run.

| path | aarch64 debug | riscv64 debug | x86_64 debug | aarch64 release | riscv64 release |
|---|---|---|---|---|---|
| boot, first settle | 17,600 | 16,760 | 16,080 | 4,192 | 4,192 |
| at the prompt | 19,000 | 18,976 | 18,184 | 10,608 | 10,688 |
| first commands (`echo hello world \| wc`) | 22,880 | 22,672 | 21,824 | 11,016 | 11,112 |
| `package install` of a file (tampered, then real) | 32,056 | 31,816 | 31,000 | 16,224 | 16,336 |
| `package install` by name (fetched) | 32,440 | 32,184 | 31,304 | 16,432 | 16,544 |

Debug had 328 bytes to spare on aarch64. The `std_exerciser` line (a `std` program's spawn, milestone
595) was not run locally, because it needs the `nife-dev` toolchain; CI runs it and prints its gauge.

Two things the table says:

- Every architecture is within 1.2 KB of the others. The stack is a property of
  `system_initializer`'s code, not of an ISA.
- Debug is twice release, and the difference is one frame. `boot` is 12,848 bytes unoptimised and
  3,200 optimized, and it is live under everything the spawn service does.

## The frames

The largest frames on the install path, aarch64 debug (release in brackets, where the function
survives inlining):

| frame | bytes | what is in it |
|---|---|---|
| `system_initializer::boot` | 12,848 (3,200) | every construction local, each with its own slot unoptimised |
| `system_initializer::spawn_service` | 5,952 (7,424, with `activate` inlined) | the request loop |
| `system_initializer::edit` | 5,456 (4,528) | `new: [u8; PAGE_BYTES]`, the next activation set |
| `system_initializer::activate::{closure#1}` | 4,496 | `old: [u8; PAGE_BYTES]`, the live activation set |
| `system_initializer::vouched` | 4,368 | |
| `package_archive::installable` | 1,040 (304) | |

A walk of direct calls from `_start` finds that chain, `boot`, `spawn_service`, `activate`, its
closure, `edit`, `installable`, at 31,424 bytes. The gauge measured 32,440: the kilobyte between them
is indirect calls and leaf frames a static walk cannot see.

To list them yourself (aarch64; the frame record's 16 bytes are not counted):

```sh
OBJ="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-objdump"
"$OBJ" -d --no-show-raw-insn -C target/aarch64-unknown-none-softfloat/debug/progenitor | python3 -c '
import re, sys, collections
size, f, n = collections.Counter(), None, 0
for line in sys.stdin:
    m = re.match(r"[0-9a-f]+ <(.*)>:$", line)
    if m: f, n = m.group(1), 0; continue
    n += 1
    m = re.search(r"sub\s+sp, sp, #(0x[0-9a-f]+)(, lsl #12)?", line)
    if f and n < 12 and m: size[f] += int(m.group(1), 16) << (12 if m.group(2) else 0)
for f, v in size.most_common(12): print(f"{v:7d}  {f[:100]}")'
```

## The decision: raise to twelve pages

`INIT_STACK_PAGES` went from 8 to 12 (48 KiB). The debug peak is now 66% of the stack.

What was considered:

- Trim `boot`'s frame out of the way. The biggest single saving, about 12.8 KB in debug, and the
  better design: `boot`'s construction locals are dead once the system is up, and the spawn service
  should not stand on them for the life of the machine. It needs `boot` to return what the service
  needs and `_start` to call `spawn_service` itself. That restructures the function milestones 205,
  595 and 23 are editing today, so it is a proposal rather than this lane's diff:
  `design/roadmap/0696-the-spawn-service-runs-outside-boots-frame.md`.
- Move the two page buffers off the stack. `old` and `new` are 8 KB of the install path, and no
  open lane edits `activate` or `edit`. But the only place to put them is a `static`, which needs
  `unsafe` or a cell type in a program that has neither for this, and makes both functions
  non-reentrant, a constraint the code would carry without saying. That is more moving parts to save
  what 16 KB of RAM buys outright. Not taken, on elegance rather than effort.
- Raise to sixteen pages. Costs the same 16 KB more of RAM, which is nothing. Refused because the
  point of the gate is that growth gets looked at: at sixteen, 25 KB of growth would pass without
  anyone reading a number.

The elegance test, out loud: would I still choose the raise if the restructure were the same work?
**No.** The restructure is the more elegant fix, because it removes the reason debug is twice
release rather than making room for it. The raise wins today on effort and on collision with four
open lanes, and this paragraph says so in those words. It is also the reversible one: one constant,
and the gauge will say what the restructure buys when it lands.

The RAM cost is four pages for the progenitor, and four each for `hello`'s init roles and the riscv64
serial driver, which share the constant and are far shallower.

## Raised again, to fourteen (milestone 809, 2026-10-10 UTC)

Milestone 809 (the package client becomes a program) gave the spawn loop a second sender, `jig`'s
installer endpoint, and a rollback now compares the two generations it moves between
(`activation_set::foreign_change`, under `edit`'s page). The first boot measured 44,432 bytes after
`jig rollback` on aarch64, 3.5 KB past twelve pages' floor. Moving `activate` and the installer's two
helpers out of line took the spawn loop from 11,200 bytes to 10,560, against 9,024 before the
milestone, and the peak stayed over.

So fourteen pages, measured on fourteen by `script/swish-check` in debug: 44,224 (aarch64), 43,744
(riscv64) and 42,480 (x86_64) at peak, each after `jig rollback`, with 13 to 15 KB spare. The
reasons above for raising rather than trimming hold unchanged, and so does the trim that would
remove the need, milestone 696 (the spawn service runs outside `boot`'s frame)'s: `boot`'s 14.7 KB frame still stands under every request. The
elegance test gives the same answer it gave at twelve, for the same reason.

## The gate

`kernel::progenitor_stack::HEADROOM_FLOOR` is two pages (8 KiB). A boot that leaves less makes the
kernel say `BELOW`, and `script/swish-check` fails on it, naming the command that got there. The
same script fails if the gauge never prints at all, because a gauge that stopped printing is how
this stack got raised three times.

The two pages are each a measured fact. One page is the step this stack has actually grown by: the
install path's two biggest frames are each mostly one `[u8; PAGE_BYTES]`. The other covers what the
gauge cannot see, the paths `swish-check` never types and the indirect calls that put the measured
peak a kilobyte above the static chain.

With fourteen pages it fires at 49,152 bytes used: about 5 KB above the debug peak of 2026-10-10.

## BUGS

- **A watermark sees exercised paths only.** A spawn of something `swish-check` never types goes as
  deep as it goes, unmeasured. The floor's second page is the allowance for this, not a proof.
- **The gate is debug-only in CI.** `swish-check --release` exists but no CI row runs it. Release is
  half as deep, so the debug gate is the stricter one, but a release-only deep path (a frame that
  inlining makes larger, as `spawn_service` is) would pass.
- **`swish-check --release` failed `interrupt_ignorer` on aarch64** on its one local run, a `^C`
  timing line. Not investigated; nothing gates that leg.
- **The capability-slot gauge still has the `x86_64` gap this one closed.** It could share the yield
  trigger; that is its owner's change to make.
- This appendix is not yet a row in `notes/stack.md`'s incident table. Touching that file obliges
  the toucher to bring its bold down from 48 spans to 9 under §213 (writing standards), which is an
  edit to calef's teaching prose rather than to this lane's subject. `notes/stack/README.md` links
  it meanwhile, and says so.
