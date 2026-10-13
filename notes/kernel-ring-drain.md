# How the log service drains the kernel's ring (a proposal)

Milestone 342 (the kernel and the `console` server drive one UART from two address spaces), first
step, written 2026-10-03 (UTC) by lane `milestone/342-kernel-console-arbitration`. §242 (a system
log), Question 3, left one thing to the building lane: *"Draining the ring is new authority at the
kernel's boundary: a read method on a new object (Zircon's `zx_debuglog_read`), or a read-only frame
plus a notification. Both are the syscall surface under §10 (process model: capability-based,
microkernel), and the building lane proposes which."* This is that proposal. The name of this note
is provisional.

Everything else in §175 (where the kernel's own output goes once userspace owns the console) is
ruled and stays as ruled. The kernel appends ordinary output to a 16 KiB ring. The service in
milestone 613 (a system log service: the in-memory half) drains it and forwards whole lines. A
panic writes the UART directly, and an undrained ring falls back to direct writes and counts them.
The panic path is identical under both shapes below, so it does not separate them.

## The two shapes

### F, a read-only frame plus a notification

At boot the kernel allocates the ring as a run of frames and grants the progenitor three
capabilities, which it passes to the log service:

- the ring, read-only: a header (the next sequence number, the count of records overwritten, the
  count of lines that fell back to the UART), then F3 records (`system_log_protocol::record`);
- one cursor page, read-write: the service writes the sequence number it has consumed up to;
- a notification the kernel signals when it appends a record.

The service copies records out and checks the header's sequence number after the copy, so a record
the kernel overwrote mid-copy is detected and counted, not delivered torn. That is the seqlock
discipline the clock page already uses. The kernel reads the cursor page through its direct map, so
no user pointer is followed. The cursor is untrusted and decides only one thing: whether the ring
is being drained. A cursor still holding its sentinel means no drainer has attached, which covers
all of boot. A cursor lagging the newest record by more than a bound (provisionally 64 records, or
half the ring) means the drainer is wedged or dead. In both cases a line goes straight to the UART
and is counted.

### R, a read method on a new object

A new object type (a `KernelLog`, Zircon's `ZX_OBJ_TYPE_LOG`) is minted once for the progenitor,
with a `READ` method that returns the next record from a cursor. A wake-up is needed as well, either
a blocking `READ` or a notification registered with the object. Either way the kernel knows directly
whether it is being drained, from the time of the last `READ`.

## The seven questions

### 1. What else was considered, and why each lost.



- R with records returned in registers. `invoke` returns five words, so about 24 bytes of a record
  per call. A median kernel line (57 bytes, from §242's xenon measurement) plus its 32-byte header
  is four calls, and the longest is eleven.
- R with a user buffer. One call per record, but it is the first syscall since milestone 8 (the
  console driver leaves the kernel) that writes into a pointer userspace chose.
  `kernel/src/syscall.rs` states the property this would break: *"No pointer ever crosses this
  boundary ... the kernel follows no user pointer and there is no deputy to confuse."*
- The kernel `SEND`s each record to the service's endpoint, the way a death message reaches a
  supervisor. Refused: `println!` cannot block, and a non-blocking kernel send would be a new IPC
  semantic. That is a larger surface than either shape above.
- F without a notification, the service polling on a timer instead. Refused: it trades one kernel
  signal per line for a wake-up every period on an idle machine, and adds latency to every line.

### 2. What this tree already does in the analogous case.

It does F, twice. The machine statistics page (§225 (`free` sees the machine and your share),
`kernel/src/machine_statistics.rs`) is a frame the kernel writes and grants to the progenitor
read-only at boot (slot 23). It goes without a seqlock on purpose, because its counters need no
consistent snapshot. The ring does need one, and the clock page's `clock_protocol` is the tree's
seqlock reader. The kernel already wakes userspace without a new method by signaling a
notification: an armed timer's expiry does it from the tick (`signal_locked` in
`sched::expire_timers`). `sched::signal_notification_from_interrupt` is the any-context form,
written for exactly this kind of caller and so far called only from tests. A frame run capability
(`cap::page_frame_run_cap`) exists for a multi-page ring. F needs no new object type and no new
method. It needs three new boot grants and one shared-memory layout. R needs a new object type and
at least one new method.

### 3. Prior art, read.



- Zircon: `zx_debuglog_read` (fuchsia.dev, read 2026-10-03) reads *"a single record"* per call into
  a caller buffer. It does not block; with nothing to read it returns `ZX_ERR_SHOULD_WAIT`. *"Gaps
  in the sequence indicate dropped log records."* The handle must be `ZX_OBJ_TYPE_LOG` with
  `ZX_RIGHT_READ`. That is R with a user buffer. From memory and not verified: a reader waits on the
  object's readable signal.
- Linux `/dev/kmsg` (kernel.org ABI document, read 2026-10-03): *"Every read() ... receives one
  record"*, blocking unless `O_NONBLOCK`. An overwrite makes the next read return `-EPIPE`. No
  `mmap` is offered. That is R with a buffer and a blocking read.
- seL4 has no kernel log to drain. Its debug output is a per-character syscall that verified builds
  compile out (§242's table).

Both systems with a drained ring chose R. They chose it in kernels that already copy into user
buffers on every `read`, which this one deliberately does not.

### 4. Is the premise true?

Yes. The splice exists: run 36622549563's transcript has the kernel's `progenitor stack:` gauge
inside the shell's echo (`notes/swish-check-flake.md`). The kernel does write after the handoff: six
gauge lines in a 111-line, 17.6-second aarch64 transcript
(`notes/kernel-console-arbitration-pricing.md`, part 1). The drain does need new authority. No
capability in the tree today names a kernel-owned ring, and handing the service the UART lock
instead is option C, which §175 refused in favor of B.

### 5. What each costs.

Per-record figures come from the HVF bench rows in `notes/benchmarks.md` (null syscall 27 ns,
context switch about 29 ns, measured 2026-08-04). They are derived, not measured on this path.

| | per record | per batch | kernel memory | progenitor slots | new surface |
|---|---|---|---|---|---|
| F | one copy in userspace, no trap | one notification wait (about 56 ns) | 4 ring pages + 1 cursor page | 3 (24 to 26 of 32) | 3 boot grants, 1 layout in `system_log_protocol` |
| R, registers | 4 calls median, 11 worst (about 108 ns median) | the same wait | 4 ring pages | 1 | 1 object type, `READ`, a wake-up |
| R, buffer | 1 call (about 27 ns plus the copy) | the same wait | 4 ring pages | 1 | the above, plus the first user-pointer write since milestone 8 |

At the measured volume (six lines in 17.6 s after handoff) every column is under a microsecond per
second. **Speed does not decide this. The size of the surface does.** The one row where the shapes differ by more than noise is a boot
flood. The 146 lines of a xenon boot drain in one batch under F, and in about 600 calls under R
with registers.

### 6. How reversible, and who has acted.

Nobody has acted on either shape. Both are irreversible once a second program links the result,
because the ring layout or the method numbers become a wire. F's wire is a layout in one crate the
kernel and one service share. R's is an object type and a method number in `abi`, which every
program can see. F's three slots are a boot-grant ABI between the kernel and the progenitor, the
same kind `machine_page` added at milestone 126 (the `procps` package).

### 7. Would we still choose F if both cost the same?

Yes. F has fewer moving parts (no object type, no method, no wait queue) and keeps the no-pointer
property, and its reasons are about surface rather than effort. Measured as work, F is probably
*more* of it: the userspace seqlock reader is new code, where R's copy would live in the kernel.

## Recommendation, and what it asks for

**F.** It is an irreversible fork, where the tree normally gives options rather than a
recommendation. §242 asked the building lane to propose which, so this does, and the options above
are complete enough to overrule it.

If F is accepted, this lane builds the rest of milestone 342 without another ruling:

- the ring and the fallback on all three ISAs;
- starting the log service at boot;
- forwarding whole kernel lines to the console.

The proof is the brief's. A flood of kernel output during the shell's echo cannot splice while the
service is alive. A panic still reaches the UART. An undrained ring falls back and counts. And
`script/swish-check` is green on all three.

If R is chosen instead, the open question is registers or a buffer. The buffer is the variant that
needs `mmu::user_can_read`'s writing twin. That primitive was kept *"for the next syscall that does
take a user pointer"*.
