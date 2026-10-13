---
status: DECIDED
raised: 2026-09-27
decided: 2026-09-27
ratified_by: calef
---

# 242. A system log: one service that programs and the kernel append to

Raised 2026-09-27 (UTC) by lane `proposal/system-log` (pull request #1423), from calef's question
that day: is there value in a logging service that everything in the OS logs to? He ruled milestone
152 (durable delegation)'s Fork 6 as option C the same day: a scheduled job writes its output
through whatever its schedule entry grants, so a later log capability is one more grant. He asked
for this to include §175 (where the kernel's own output goes once userspace owns the console) and
pull request #1419, which prices §175's options. *(Section number provisional: §241 (threadbare) is claimed by
#1421, so this took 242 on 2026-09-27 and may move at merge. The slug, the service's name and every
constant below are provisional too.)*

## The ruling

calef ruled every question below on 2026-09-27 (UTC), each as a maintainer comment on pull request
#1423 (`gh pr view 1423 --comments` has the full text). §242 moves to DECIDED as a result, and the
sections that follow keep the questions' reasoning with each ruling folded in.

1. **Yes.** One system log service that the kernel and programs append to.
2. **Yes, with a fallback.** The capability shape and the kernel ring below, as recommended,
   plus: when the ring is not being drained, the kernel writes the UART directly instead of
   dropping lines, accepting torn lines only while the service is down. For §175 this is option B
   with the panic escape and the fallback.
3. **The record format is JSONL.** The stored and read format is one JSON object per line. The
   service stamps `seq`, `time`, `program`, `user` and `severity` from the badge, never from the
   writer, and the text goes in `msg`. The binary transport underneath is unchanged (F3: a header,
   a body-kind byte, up to 224 bytes of text); the kernel ring writes that binary form, never JSON,
   and the service converts. The JSON writer is written in-tree, §46 (thin primitives or whole
   subsystems), with no dependency. Severity
   uses syslog's eight levels.
4. **Severity rides in the framing.** `byte_sink_protocol`'s unused first-word bits carry it,
   amending that protocol. A writer that does not set them gets the level inferred from its stream
   (diagnostics warns, ordinary output informs).
5. **Storage: accepted as recommended.** In memory now, RedoxFS later through a directory grant.
   calef asked for a proposal for the RedoxFS half; it is
   `design/roadmap/0687-a-system-log-on-redoxfs.md` (slug provisional).

## What the tree does today, counted

Every diagnostic path in the tree, found by grep on base `140adf859` (the commands are in #1423):

| emitter | count | where it lands | who reads it |
|---|---|---|---|
| kernel `println!`/`print!` | 693 call sites (641 and 52); `lib.rs` 143, `bench.rs` 52, `soak.rs` 43 | the console UART, directly, under the kernel's own lock (`kernel/src/console.rs:737`) | a human, or a harness scraping serial |
| kernel gauges after handoff | 2: `capability slots:` (`kernel/src/cap.rs:358`) and `progenitor stack:` (`kernel/src/progenitor_stack.rs:118`) | the same UART, interleaved with userspace at byte granularity | `script/swish-check`, which needs both lines and filters one of them out first |
| kernel fault report | 3 lines, ~150 bytes, per faulting user thread, all three architectures | the same UART | `script/swish-check` (`KERNEL_FAULT_TOKENS`) |
| semihosting | 58 hits, all exit-with-status | QEMU's exit code | the test harness; no semihosted writes exist |
| userspace output | no debug-put syscall; four syscalls total (`crates/abi/src/lib.rs:119`) | a byte sink (`crates/byte_sink_protocol`, 16 bytes per `SEND`) to whatever slot 1 holds | a terminal, a pipe, a file, or nothing |
| declared second streams, §67 (a program's second stream is a declaration) | 5 programs declare `BytesAndDiagnostics`, of 21 manifests | a second sink the shell plans for them | the terminal, or `2>` |
| manifest `reports` flag | byte 28 of the manifest note | nothing: no spawn code reads it, only test asserts | nobody |
| timetable child report | one word per scheduled child (`components/src/timetable.rs:376`) | the timetable's `CHILD_REPORT` endpoint | the kernel harness in tests; Fork 6 C removes it for durable jobs |
| `login`'s audit trail | one `ATTRIBUTED` message per login | `login_audit_receiver`, which discards it | nobody; its own `BUGS` says a real consumer is owed |
| milestone 480 (a server logs which channel a request arrived on) | 0 lines of code | | REFUSED for want of a consumer |

The tree has one diagnostic destination, the serial line, and no ring buffer, `log` dependency or
log capability. Three records are thrown away today: the audit trail, a detached job's output, and
the kernel's lines once the console server owns the UART. §175's option B, a buffer the kernel
appends to, has nobody to drain it; a system log is that drainer, and it also meets milestone 480's
revisit condition, *"a multi-client server that serves more than one principal and is not anonymous
by design."*

## Prior art, read

Read on 2026-09-27 from primary sources; "(memory)" marks what was not verified.

| system | record | largest record | kernel ring | who names the sender |
|---|---|---|---|---|
| Zircon debuglog | 40-byte header (sequence, length, severity, flags, time, pid, tid), then text | 256 bytes, so 216 of text (derived from `zx_log_record_t`) | 128 KiB, oldest overwritten, a gap in sequence numbers is the loss | the kernel: object ids the writer cannot choose |
| Fuchsia LogSink | one header word (type, severity, size), a time word, typed key/value arguments | 32 KiB | n/a; the archivist forwards klog at INFO, and klog rolled out before it reads is lost | the router: component manager attaches the moniker while routing the capability |
| Linux printk and journald | printk: sequence, time, level, facility, caller id, text; journald: `KEY=VALUE` fields | printk 1,024 per record; journald `LineMax` 48K | 128 KiB plus 4 KiB per CPU; `/dev/kmsg` reports an overwrite as `-EPIPE` | journald fills `_PID`, `_UID` itself; fields starting `_` "cannot be altered by client code" |
| syslog (RFC 5424) | `PRI` and seven header fields, optional structured data, text | a receiver must take 480 octets, should take 2,048 | n/a | nobody: every field is the sender's claim, and the RFC lists spoofing |
| macOS unified log | a format-string offset and the image's UUID, arguments stored apart | private data at most 4,096 | n/a | kernel and `logd`, through per-process buffers (memory) |
| Genode LOG | `write(String)`, text only, no level | 232 bytes | n/a | the parent chain: each hop prefixes the label, and only the last part is the client's |
| seL4 | `seL4_DebugPutChar`, one character | n/a | none | nobody; compiled out of verified builds |
| Plan 9 | `/dev/kmesg`, text | n/a | 16 KiB | nobody; `syslog(2)` appends to a file |

Two things carry over: unforgeable attribution comes from the router or the kernel, never the
writer, and a badged endpoint is both; and every drained kernel ring counts what a slow drainer
lost rather than blocking the kernel.

## Question 1: whether to have one

**Decided: yes.** Three orphaned records and one stuck decision each get a home from the same
service, and milestone 480's refusal (a mechanism with no consumer) no longer holds.

Refused: each producer keeps its own record, which is what the tree gets by doing nothing. The
kernel cannot write a file, so §175 stays stuck, and a reader asking "what happened at 03:00" has to
know every producer. It is not wrong for job output, and Fork 6 C already lets an entry grant a
directory instead of the log.

## Question 2: the capability shape

**Decided: an append capability is a badged endpoint the spawner mints; reading is a separate
capability; the service stamps every record from the badge.**

- Writing. The log service holds one unbadged endpoint and gives it to the progenitor, which mints
  a badged copy per process it spawns (`rendezvous::BADGE`), §230 (badged endpoint capabilities),
  built for milestone 599 (a frame per filesystem client channel). That is §109 (attribution is a
  property of a channel), the mechanism the tree did not yet have. The writer supplies text only,
  never its own name, user or time. A scheduled job is spawned by its
  timetable, not the progenitor, so whether a timetable holds a minting endpoint or its jobs share
  the session's badge is open for the building lane.
- Stamping. The service maps a badge to (program digest, user, process) once, writing that into
  the record's header. That is journald's underscore fields, where the daemon rather than the
  client fills `_PID` and `_UID`, except the kernel's badge replaces `SCM_CREDENTIALS`.
- Reading. A separate capability, because appending and reading are different authorities (an
  untrusted program may write its own diagnostics and must not read `login`'s). Two reads: a
  system read the owner's console holds, and a per-user read the durable session holds, a
  badged read endpoint the service filters by user. Per-user logs are a filter over one store, not
  one service per user, since the kernel's and system services' records belong to no user.
- Declaring. Decided: programs do not declare "logs". A program already declares a
  diagnostics stream (§67); where that stream goes is the spawner's choice, the terminal when a
  person is attached and the log when nobody is. That is Fork 6 C: the entry grants the log to the
  declared stream, and no manifest field changes. The refused alternative, a `log` manifest field,
  costs a manifest note version bump for a fact the spawner already knows.
- The one protocol consequence that matters. A byte sink's writer *must end* on `Gone`
  (`crates/byte_sink_protocol`, the `SIGPIPE` rule), and `SEND` blocks until received; neither suits
  a log, which is why `login_audit_receiver` already parks an undrained audit send forever. A log
  grant means `Gone` is ignored and the service receives unconditionally, dropping rather than
  letting a writer wait.

## Question 3: the kernel's path, and §175

**Decided: §175 option B with the panic escape and a fallback, which the pricing note calls C
wearing a buffer, and which this service is the reason to build.**

- The kernel appends each line to a bounded ring in kernel memory instead of the UART, once a
  drainer has attached. Before that, which is every line of boot up to the handoff, it writes the
  UART directly and the ring, so the log starts with the boot (dmesg's shape).
- The log service drains the ring and is the only thing that forwards kernel lines to the console,
  as whole lines through the console server. That removes the byte-granularity collision #1419
  measured and keeps both gauges and the fault report in the transcript `script/swish-check` reads.
  Unchecked: whether the console server keeps one sink's line whole against another's; if not, that
  is the console server's fix, owed for userspace writers anyway.
- A panic never waits for anybody. It stops appending, breaks the console lock and writes the UART
  directly, what `console::force_unlock` already does and what Fuchsia (`dlog_panic_start`) and
  Linux (`console_flush_on_panic`) do. A panic may splice with the console server's bytes, the price
  already accepted in `kernel/src/console.rs:687`.
- A dead or wedged drainer. **calef's addition to the recommendation, ruled 2026-09-27:** rather
  than drop lines silently, the kernel falls back to writing the UART directly whenever the ring is
  not being drained. That reintroduces the interleaving §175 exists to remove, but only while the
  service is down, and a torn line while a supervisor restarts the service is cheaper than a kernel
  line nobody ever sees. The ring keeps a count of what it also held, so a drop from the ring itself
  (the fallback is per line, not per ring-full) is still counted.

Draining the ring is new authority at the kernel's boundary: a read method on a new object
(Zircon's `zx_debuglog_read`), or a read-only frame plus a notification. Both are the syscall
surface under §10 (the capability-based microkernel process model), and the building lane proposes
which. The other §175 options lost for reasons
#1419 already priced: A (a second port) is absent or unconfirmed on all three boards; D is today;
R (release builds go quiet) does not touch CI, which builds debug.

## Question 4: the record format, which is irreversible

Three things become wire formats, and they are not equally hard to change:

| surface | who agrees on it | how hard to change |
|---|---|---|
| writer to service | every program that writes | hardest; recommended to be the existing byte sink framing, so it adds nothing |
| kernel ring to service | the kernel and one service | one crate, two readers |
| the stored and read record | the service, the reader tool, and a file on disk if persisted | hard once a file outlives a version |

The last two share one record. Options:

| | shape | fixed fields | size bound | costs |
|---|---|---|---|---|
| F1 | a fixed header the service writes, then UTF-8 text | sequence (8), boot-relative nanoseconds (8), source (8: badge-derived id), severity (1), flags (1: kernel, dropped-before), length (2), 4 spare; **32 bytes** | text at most 224 bytes, cut with a flag | the smallest; text only; querying means matching text |
| F2 | the same header, then typed key/value arguments | as F1, plus an argument count | a record up to one page | structured queries; a schema crate every writer links; Fuchsia's and journald's shape |
| F3 | F1 with a body-kind byte, text now | as F1, one spare byte becomes kind (0 text) | as F1 | F1's cost now; F2 later becomes kind 1 without a version change |

Measured to size the bound: a whole xenon boot is 146 kernel lines and 8,791 bytes, median 57 bytes
a line, p90 93, longest 123 (`bench/xenon-2026-09-17/first-light-095500.log`). An eight-hour radon
soak is 6,046 lines and 1,152,699 bytes, median 196 and longest 448
(`bench/radon-2026-09-25/soak-8h.log`). 224 bytes of text holds every kernel line and 99% of the
soak's; the soak's longest would be cut. F1's header plus 224 bytes is 256, the same total as Zircon's
debuglog record, whose own header is 40.

**Decided: F3 for the transport, JSONL for the stored and read record** (the ruling above). F3
costs F1 today and leaves the door F2 would need, spending a byte F1 already had spare. JSONL is a
fourth shape, layered on top rather than replacing F3. The service converts F3's binary records
into one JSON object per line before they are stored or read, so the wire cost stays F3's and only
the stored form pays JSON's overhead. Severity: syslog's eight levels (RFC 5424), riding in the byte
sink framing's unused first-word bits 32 to 55, which amends `byte_sink_protocol`. A writer that
does not set them gets the level inferred from its stream (diagnostics warns, ordinary output
informs).

## Question 5: storage and retention

**Decided: in memory first, RedoxFS second, and neither blocks the other.** The RedoxFS half has
its own roadmap proposal, `design/roadmap/0687-a-system-log-on-redoxfs.md` (slug provisional),
covering what persists, boot handoff, retention and rotation, crash consistency and write cost.

- The kernel ring: 16 KiB (4 pages), which holds a whole boot at 32 + 57 bytes a line (about
  180 lines against the 146 measured). Full means the oldest go and a count is kept.
- The service's ring: 64 KiB in memory, oldest out, one dropped-count record per gap. At the
  soak's rate (one 196-byte line every five seconds) that is about 24 minutes of a chatty reporter,
  and hours of anything quieter.
- Persistence: later, as an ordinary directory grant to the log service, the same way Fork 6 C
  grants a job a directory. Persisting is when the format becomes a file on disk, so F3's version
  byte matters from that day.
- Before storage is up, the kernel ring holds boot, and userspace output before the log service
  starts goes where it goes today, to the console. The progenitor should start the log service
  before anything it would want to log.

## What it costs, measured where possible

- Progenitor capability slots. One endpoint to the log service for life, to mint badges; at 23 of
  24 at peak (`kernel/src/cap.rs:309`), the last slot on `main` (#1360 raises the table to 32). A
  badged copy per child is in the child's table, not the progenitor's.
- Pages. Four for the kernel ring, sixteen for the service's ring, plus the image and a component's
  usual 12-page stack.
- Per line. A 100-byte line is seven byte-sink `SEND`s; at half the measured 350 ns IPC round trip
  (`notes/benchmarks.md`), about 1.2 microseconds, derived not measured.
- Per record: 32 bytes of header on a median 57-byte kernel line, which all three formats pay.

## The seven questions, answered before calef ruled

In [`seven-questions.md`](0242-a-system-log/seven-questions.md).

## What this unblocks

Milestone 342 (the kernel and the `console` server drive one UART), through the §175 ruling above;
a durable job's output under Fork 6 C; a real `login_audit_receiver`; and, once the service exists,
milestone 480's revisit. Building the service, the kernel ring and drain syscall, and the format
conversion are open work for a building lane; the RedoxFS half is proposed separately (Question 5).
