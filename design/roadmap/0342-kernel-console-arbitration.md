---
status: BUILT
raised: 2026-09-03
built: 2026-10-03
milestone_dependencies: 613
decision_dependencies: 175
machine_requirements: none
specific_machine: none
needs_person: no
---
# 342. The kernel and the `console` server drive one UART from two address spaces, and nothing arbitrates

Premise re-checked 2026-09-19 and it holds. §149 (may the kernel answer on an endpoint) decided how a program *reaches* a console
server and was itself dissolved by §121 (what a device capability is when the device has no page)'s
reopening, which is a different question. `script/swish-check`'s
own `BUGS` still describes the interleaving as a live defect in the system rather than in the script.
Milestone 243 (a machine with no serial port has no way to say anything)'s `BUGS` still records
that it "has its own home", which is this block.

The question's home is §175 (where the kernel's own output goes once userspace owns the console),
which `decision_dependencies` names.

## Priced, 2026-09-27 (PRICING lane, calef's approval)

[`notes/kernel-console-arbitration-pricing.md`](../../notes/kernel-console-arbitration-pricing.md)
answers §175's two open items: the hardware fact per board, and what a panic costs under each
option. It prices a fifth option calef asked for (release builds go quiet, seL4's shape), and
confirms the interleaving with a captured, byte-exact CI transcript already in-tree
(`xtask/src/swish_check.rs`'s `SHREDDED_CI_TRANSCRIPT`), rather than assuming it.

Highlights: a second port is free under QEMU on all three architectures, and confirmed absent,
unconfirmed, or flatly impossible on the three real boards respectively, worst on xenon where there
is no second serial connector at all. The kernel's collision surface is larger than §175 measured.
They are two routine gauges, not just fault reports. One of the two (`capability slots:`) has no
equivalent filter to the one already built for the other gauge after it caused this exact failure
once. The release-only option does not touch CI at all, since CI never builds release.

No option is recommended, per §175's own reasoning and AGENTS.md's rule to give options rather than
a recommendation on an irreversible fork. Still no work on the fork itself: this remains NOT-STARTED
pending calef's ruling on §175.

Where the kernel's own output goes once userspace owns the console is a design
fork rather than a bug to fix, and it is an architect's. The options are genuinely different systems, not
variations on one, and the choice binds every architecture and every future console consumer.

## Decided, 2026-09-27

calef ruled §175: **B, with a panic escape and a fallback**, the same shape as pull request #1423's
system log (`design/decisions/0242-a-system-log.md`). See §175's "The ruling" for the full record
and why A, C, D and E lost.

The ruling names a customer for the kernel's ring: the log service is milestone 613 (a system log
service: the in-memory half), which drains the ring and forwards whole lines to the console. 613
was built on 2026-10-02 (pull request #1494), and this milestone is claimed by lane
`milestone/342-kernel-console-arbitration` (draft pull request #1498) from 2026-10-03 (UTC).

## The drain is a syscall-surface fork, proposed 2026-10-03

§242 (a system log) left how the service reads the kernel's ring to the building lane: a read
method on a new object, or a read-only frame plus a notification. Both change the surface §10 (the
capability-based microkernel process model) governs, so the lane proposes and stops.
[`notes/kernel-ring-drain.md`](../../notes/kernel-ring-drain.md) answers the seven questions and
recommends the frame, which needs no new object type and no new method, keeps the property that no
user pointer crosses the syscall boundary, and matches the machine statistics page's shape. Nothing
past the proposal is built until it is ruled.

**In brief.** Once the `console` server owns the console, two address spaces are writing to the same
UART with no arbitration between them. The kernel writes directly, because a kernel that cannot
print during a fault is a kernel nobody can debug, and the server writes on behalf of userspace. The
streams interleave at byte granularity. Deciding this means saying where kernel output goes: a
second port, a buffer the server drains, a claim the server takes and the kernel respects except in
a panic, or something else.

## Ruled F, and built, 2026-10-03 (UTC)

calef ruled F on pull request #1498: a read-only frame, a cursor page and an append notification,
with a counted UART fallback when there is no drainer or it lags. Built on lane
`milestone/342-kernel-console-arbitration`. The names are provisional: `kernel_log`,
`kernel_ring`, `console::Inserter`, and the probe features and flags.

- **The kernel** (`kernel/src/kernel_log.rs`). Every line becomes an F3 record in a 16 KiB ring of
  63 fixed slots, each slot a seqlock. Until a drainer attaches, lines also go straight to the
  UART, as before. Once one attaches and keeps up, they wait in the ring and the kernel signals
  it, deferred to a point that holds no lock, because the console lock is the leaf of the lock
  order. If the drainer falls half a ring behind, or leaves a line unread for 500 ms, the kernel
  prints the unread lines and the new one itself, flags them `DIRECT` and counts them. A panic
  does the same flush, then holds nothing back.
- The boot: the kernel grants the ring, the cursor page and the notification at progenitor
  slots 24 to 26. The progenitor starts `system_log` right after the console, binds the
  notification to its thread, and drops its own copies. Every boot builds the console since
  milestone 632 (provisional), so every boot starts it.
- **The log service** drains the ring into its log under the program name `kernel` and forwards
  each line the kernel did not print itself.
- **The console** (`system_log_protocol::console::Inserter`) writes a forwarded kernel line only
  at the start of a terminal line. Mid-line it waits for the next newline, or for the service's
  flush 250 ms later, which puts the line on its own line and redraws the partial line beneath.

### Proof

- Two system tests, green on aarch64, riscv64 and x86_64. The first holds a line for a caught-up
  drainer (`tx_bytes` does not move), lets the drainer stall, then sees both lines printed, flagged
  `DIRECT` and counted twice. The second sees a panic print the held line, with the next line
  direct.
- Host tests for the ring's round trip and torn-read detection, and for the inserter.
- `script/swish-check --flood` makes the kernel print a line every 100 ms through the whole
  session. With the service attached, every flood line arrived whole and the gate stayed green:
  254 on aarch64, 263 on riscv64 and 180 on x86_64, 0 spliced. x86_64
  floods once a second rather than ten times. Its TCG leg runs about thirty times slower. At
  ten a second the service fell behind and the counted fallback spliced 29 of 6,990, which is the
  fallback working as ruled.
- The detached control (`--flood-detached`, the kernel printing for itself as before) spliced 83
  lines on aarch64 and 11 on riscv64, and failed the gate both times.
- `--panic-probe` panics on the thirtieth flood line with the service attached. The panic reached
  the UART on all three, after 30 whole flood lines.
- Plain `script/swish-check` passed on aarch64 and riscv64, and every flood leg ran the whole
  script green, x86_64's included.
- `notes/swish-check-flake.md` records the base rate this fixes: 3 of 29 merge-group jobs on
  2026-10-03.

## Why this matters

It corrupts every bench session on argon, radon and xenon. A serial log is the only thing those
three machines can say, and milestone 216 built a tool whose whole contract is recognising a boot
sequence in that stream. Interleaved bytes break that contract in the least visible way available:
the log is present, it looks like output, and the line the tool is matching on has a kernel message
spliced through the middle of it. A gate that reads a board reads this.

It is also already load-bearing somewhere it cannot be fixed. Milestone 243's `BUGS` points at a
home for this question that does not exist, which is the tell AGENTS.md names for being on too low a
rung: a fact that lives only in a citation to nothing.

## What a lane can do before the ruling

The fork deserves its options priced rather than argued, and none of that needs a decision first.
What the kernel actually writes after userspace takes the console, and when, is a measurable list
rather than an opinion. Whether the boards have a second usable port is a hardware fact per board.
What a buffered path would cost during a panic, which is the case that matters most and the case a
buffer serves worst, is the constraint that probably decides it.

## Follow-on

- **Recorded.** `kernel/src/kernel_log.rs`: a line held just before the drainer dies waits for the
  next kernel line or a panic, a fallback can print a line twice, and the boot window before the
  service attaches still splices.
- **Recorded.** `system_log_protocol::kernel_ring` in `crates/system_log_contract/src/lib.rs`: 63
  records, not the 180 lines §242 (a system log) sized 16 KiB for.
- **Recorded.** `components/src/system_log.rs`: at boot it serves only the kernel (no writer badges,
  no readers).
- **Recorded.** `components/src/console.rs`: a redraw replays bytes rather than the line editor's
  state.
- **Decision.** `design/decisions/0175-kernel-console-arbitration.md` owes the ruling F line, and
  §242's Question 3 the drain shape, for the integrator to mint.

## Index row

Once the `console` server owns the console, the kernel and the server write to one UART from two
address spaces with no arbitration, and the streams interleave at byte granularity. The kernel
writes directly, because a kernel that cannot print during a fault is a kernel nobody can debug. The
interleaving corrupts every bench session on argon, radon and xenon, where the serial log is the
only thing those machines can say. It breaks milestone 216's boot-sequence recognizer in the least
visible way: the log looks like output, and a kernel message splits the line being matched. Deciding it means saying where kernel output goes: a second port, a buffer the server
drains, a claim the server takes and the kernel respects except in a panic, or something else. The
choice binds every architecture and every future console consumer. A lane can price the options
first. What a buffered path costs during a panic probably decides it.
