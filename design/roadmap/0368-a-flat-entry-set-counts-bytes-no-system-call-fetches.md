---
status: NOT-STARTED
raised: 2026-09-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 368. `script/fastpath-footprint`'s entry set is flat, so an inlining flip can move 12% into it

Filed as a proposal on 2026-09-04 by the milestone 133 lane; promoted by
milestone 433 on 2026-09-19. Read against `script/fastpath-footprint` that day and every part of the
premise is intact. The riscv64 entry set is still the flat list `trap_entry`, `trap_return`,
`riscv_trap_dispatch`, `riscv_trap_body`. The failure message still prints a percentage and two
sentences of advice and no per-symbol delta, which is the item this file prices as worth doing
first. The three `#[inline(never)]` patches are all still in the tree, on
`arch::riscv64::timer::tick`, `drivers::plic::disable` and `system_call::dispatch`, each with the
measurement written beside it. Milestone 188's phase 2 changed what aarch64's entry set contains
(one vector slot and `exception_restore` rather than all sixteen entries) and left it just as flat.

Nothing is owed. It wants a lane because the fix is a judgment about what the
number is *for*, and getting that wrong in either direction costs something real. A looser gate
stops catching regressions, and a tighter one fails changes that are not regressions.

In brief. The `ipc_fastpath` half of this gate is a **closure** with a `COLD` exclusion, so
panic, formatting and teardown bytes are kept out of it deliberately and the script explains why at
length. The `system_call_entry` half is **flat**: it sums a fixed list of symbols, and it has no way to
exclude anything, because there is nothing inside a symbol to exclude. That is fine while the
symbols contain only what a system call fetches. It stops being fine the moment LLVM folds something
else into one of them.

## What happened, since the abstract version of this is easy to wave away

`riscv_trap_body` is in the riscv64 entry set for a correct reason the script argues carefully: on
this ISA an `ecall` arrives through the same handler as a page fault and a timer interrupt. Those
bytes really are on a system call's path. But that handler has arms, and **the timer arm is not one a
system call fetches.**

Milestone 133 changed `sched::reap_region_objects` and `crates/inter_process_communication`. It touches no timer, no trap
path, and no system call. LLVM nonetheless decided to inline `arch::riscv64::timer::tick` into
`riscv_trap_body` in the new build and not the old one, which put 226 bytes (1870 to 2096, 12.1%
against a 5% bound) onto a number that measures the system call path. Confirmed by diffing the two
disassemblies symbol by symbol: `system_call::dispatch`, `trap_entry`, `trap_return` and
`riscv_trap_dispatch` were byte-identical, `riscv_trap_body` grew by exactly that, and the call to
`timer::tick` disappeared from it.

The lane closed it with `#[inline(never)]` on `timer::tick`, which restores 1870 exactly and is
defensible on its own (a `jal` once per 10 ms tick costs nothing). That is a patch on the symptom
and the next one will look different, because nothing stops LLVM folding a different arm of a
different handler in next month. The person who meets it will have to re-derive all of the
above from a percentage.

## It happened again the next day, so "the next one will look different" is now observed

2026-09-04, PR #716, the daily toolchain bump. That branch changes one line,
`rust-toolchain.toml` from `nightly-2026-09-03` to `nightly-2026-09-04`, and no kernel source at
all. It failed this gate at **10.4%** (1870 to 2064) on the same symbol, `riscv_trap_body`, which
grew 550 to 744 while `trap_entry`, `trap_return`, `riscv_trap_dispatch` and `system_call::dispatch`
were byte-identical. The inlined callee this time was **`drivers::plic::disable`**, in the
`S_EXTERNAL` device-interrupt arm, which vanished from the disassembly entirely; inlining it also
dragged in `set_enable_bit`'s locked read-modify-write. Closed the same way, with `#[inline(never)]`
on `plic::disable` and the reasoning beside it.

Three things this instance settles that the milestone 133 one could only assert:

- The prediction was right about the shape. A different arm, a different callee, the same
  symbol, one day later. Neither arm is one a system call fetches.
- **The trigger need not be a code change.** Milestone 133's was at least a change to `sched.rs`,
  which made "unrelated change" arguable. This one is a compiler upgrade, so there is no source
  edit anywhere to attribute the 194 bytes to. Any lane can be handed this failure by a toolchain
  bump it did not make.
- The cost of the current answer is now measurable as a rate. Two instances in two days, each
  costing a lane a build-and-diff of two disassemblies to name one symbol. The last bullet below,
  *report the delta per symbol when the gate fails*, would have turned both of those into reading
  the failure message. It is the cheapest item on the list and it is the one that pays every time.

### And a third instance the same day, in the opposite direction

2026-09-04, milestone 220's lane, whose change adds a clock-and-reset driver and touches no
trap path, no timer and no system call. It failed this gate on aarch64 at 35.1%, and the sign is
the point: `system_call_entry` shrank, 3304 to 2144, because
`_RNvNt...6kernel7system_call8dispatch` (1160 bytes) vanished from the symbol table entirely. LLVM
folded the dispatcher into the exception handler in the new build and not the old one. The only
plausible trigger is that the lane added a dependency to the `kernel` crate, which is a
whole-crate codegen perturbation and not a change to any measured path. `exception_dispatch` and
`exception_vectors` were unchanged.

Two things this adds to the two instances above.

The failure can read as good news. A gate that says "your change made the system call path 35%
smaller" invites exactly one response, which is to re-record the baseline and take the win. Doing
so would have locked in an under-measurement: the next lane whose build inlines the other way sees
a 54% *growth* it cannot attribute, on top of a reference that no longer counts the dispatcher.
The script's own message ("no justification needed for shrinking, just an explicit acknowledgment")
is written for a real shrink and cannot tell this apart from one.

The trigger set is now wider than "a code change" or "a compiler upgrade". It is *adding a
dependency*, which any lane may do and which the diff makes look inert. Combined with the two
instances above, the honest statement is that any perturbation of the kernel crate can hand a
lane this failure, in either direction, on any of three ISAs.

Closed the same way as the other two, with `#[inline(never)]` on `system_call::dispatch` and the
reasoning beside it, which restores aarch64 to exactly 3304. That is now three patches on the
symptom in two days, which is the rate this proposal's last bullet is priced against: *report the
delta per symbol when the gate fails*. All three lanes spent a build-and-diff of two disassemblies
to name one symbol, and all three would have read it off the failure message.

## What the options look like

- Do nothing, and treat the attribute as the pattern. Cheapest, and it is not absurd: the tree
  already carries `#[inline(never)]` for exactly this reason on `sched::grant_cycle_counter` and
  milestone 156's spawn-path bodies. The cost is that the pattern is invisible until you trip it,
  and each instance is discovered by a red gate on an unrelated change.
- Measure the arms rather than the symbols. Follow the branch structure inside a root and count
  only the blocks a system call can reach. This is what the closure half already does one level up, and
  it is the honest answer. It is also the expensive one, and it needs the disassembly parser to
  understand conditional branches, which the script deliberately does not today (its `CALL` regex
  excludes them, and says why).
- Give the flat set the same exclusion the closure has, by attributing bytes inside a symbol to
  an inlined callee where the debug info can say so. Middle cost; depends on what the release build
  keeps.
- Report the delta per symbol when the gate fails, whatever else is decided. The diff above took
  a disassembly of two builds and a short script; the gate could have printed it. This is the piece
  worth doing first regardless, because it turns a percentage into a cause.

## Where it came from

Milestone 133's Follow-on. `design/roadmap/0132-the-fastpath-footprint.md` owns the gate.

## Index row

The `ipc_fastpath` half of `script/fastpath-footprint` is a closure with a `COLD` exclusion; the
`system_call_entry` half is flat, summing whole symbols with no way to exclude anything, which is fine
only while those symbols hold nothing but what a system call fetches. Three times in two days an
inlining flip moved bytes that no system call fetches into that number: `timer::tick` folded into
`riscv_trap_body` under milestone 133 (ending a permanently blocked thread, and deciding who may)'s
change, +12.1%. `plic::disable` under a one-line toolchain
bump, +10.4%. And `system_call::dispatch` vanishing into the exception handler under milestone 220 (this
kernel drives no clock or reset controller, and the first real device will need one)'s,
-35.1%. That failure reads as good news and would have locked in an under-measurement if anyone
had taken the win. Each was closed with `#[inline(never)]` on the callee, which is a patch on the symptom, and
each cost a lane a build and a two-disassembly diff to name one symbol. Any perturbation of the
kernel crate can hand a lane this failure, in either direction, on any of three ISAs. Reporting the
delta per symbol when the gate fails is the cheapest item and pays every time; measuring the arms
rather than the symbols is the honest answer and the expensive one.
