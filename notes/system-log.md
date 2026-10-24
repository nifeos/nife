# The system log service

§242 (a system log) rules what the system log is. This note carries the design of the two crates
that build it: `crates/system_log`, the service's logic, and `crates/system_log_contract`, the wire
contracts it shares with the spawner, its readers, the kernel and the console. The syscalls are in
`components/src/system_log.rs`, which wraps `system_log::Log`.

## The service, in four steps

1. Stamping. The service looks up the badge the kernel read off the writer's capability in what
   the spawner registered (`system_log::registry`). It fills `seq`, `time`, `program`, `user` and
   `severity` itself. A writer supplies text and optionally a level, never its name or its time.
2. Lines. `system_log::assembler` joins one writer's sixteen-byte chunks into lines of at most
   `record::TEXT_MAX` bytes, so interleaved writers still produce whole lines. Eight writers can be
   mid-line at once. A ninth cuts the least recent writer's line into two records, and the first
   is flagged `"cut":true`.
3. F3 to JSONL. A line becomes an F3 `record::Header` plus text, rendered by `system_log::json` as
   one JSON object per line. That is the stored and read form (§242, Question 3).
4. The ring (`system_log::ring`): 64 KiB, oldest out, read by sequence-number cursor. A reader whose
   cursor fell out of the ring gets one dropped-count line. A per-user read filters the same ring.

The dropped-count line counts every user's records, so it leaks how busy the machine was. A
per-user count would be per-user state, which §242 ruled the ring should not need. No writer has a
quota, so one fast badge can evict every other writer's records. A writer picks its own severity,
as `syslog(3)` allows; the badge guarantees only `program` and `user`.

## The intake hears only programs

The intake endpoint must receive nothing the kernel writes. `Log::handle` reads badge 0 as the
spawner and any other badge as a writer. A kernel delivery carries badge 0 (an interrupt or a bound
notification) or a fault address (§26 (the fault endpoint)), and either would be misread. So the
intake is never a fault endpoint, never bound to a notification, and never an interrupt's
rendezvous. Whoever wires the service in inherits that rule.

## The JSONL writer

The writer is in the tree rather than taken as a dependency, as the ruling said under §46 (thin
primitives or whole subsystems): a writer this small is cheaper to own than to vet. Nothing in the
service reads JSON back, because the ring keeps the stamped user beside each line for the per-user
filter. The fixed field order means a reader that greps rather than parses still works.

## Attribution

`system_log_contract` holds three contracts: F3 (`record`), the spawner's `control` words and the
reader's `read` page. F3 is a 32-byte header that the service writes, or the kernel through
`kernel_ring`, then up to `record::TEXT_MAX` bytes of text. It is the transport, never the stored
form: the service converts it to one JSON object per line before storing it (§242, Question 3 as
ruled).

Every field of a record but the text is filled by the service from the badge on the capability the
writer invoked. The kernel stamps that badge (`abi::rendezvous::BADGE`, §230 (badged endpoint
capabilities)), and the spawner that minted it registers what it means through `control` (which
program, which user, which stream). The spawner speaks through the unbadged intake capability, and
that is what makes it the only one who can say it. A writer holds a badged copy, so it can neither
send as badge 0 nor as anybody else.

What a writer sends is not in this crate. A writer speaks the byte sink
(`crates/byte_sink_contract`), whose first word's spare bits carry a severity and a body kind since
§242 amended it. That is the point of the shape: a program that already prints can be given the
log instead of a terminal and change nothing.

The control words pack a badge into the low 32 bits of one word, beside an opcode and a field,
while the kernel's badges are 64 bits. A spawner minting past `u32::MAX` has its registration
refused rather than truncated (`control::badge_fits`). No spawner in the tree comes near it; the
narrowing is a choice, not a fact of the kernel. A name is at most `control::NAME_MAX` bytes, so a
longer program name is cut at registration and a JSONL reader sees the prefix. Every program in the
archive fits.

## The kernel ring

`kernel_ring` is the shape ruled F for milestone 342 (the kernel and the `console` server drive one
UART from two address spaces); notes/kernel-ring-drain.md argues it. Three things cross the
boundary, and none is a pointer or a method:

- The ring: `PAGES` frames the kernel writes and the log service maps read-only. A header (`MAGIC`,
  the next sequence number, the fallback count) comes first, then `SLOTS` fixed slots of one F3
  record each. Record `s` lives in slot `s % SLOTS`.
- The cursor page: one frame the log service writes and the kernel reads through its direct map.
  Word 0 is the next sequence number the service has not yet consumed, or `DETACHED` until a
  drainer first writes it.
- A notification the kernel signals on bit `NOTIFY_BIT` after it appends.

A reader whose copy saw a different `seq` before and after gets `Read::Overwritten`. The ring holds
63 records, not the 180 lines §242 sized 16 KiB for. Fixed 256-byte slots are what make a lock-free
reader one comparison instead of a walk; a byte-packed ring would hold a whole xenon boot. Lines
written before a drainer attaches went to the UART directly as well, so the small ring costs
history in the log, never a line on the console.

## Kernel lines on the console

The log service sends a kernel line to the console's request endpoint in sixteen-byte chunks. Each
chunk's first word is `console::OPERATION_KERNEL_LINE` with the count in the low bits, the
byte-sink packing with a different opcode. The console's own client sends a byte count there, which
never has a top byte, so the two are told apart by the word alone. The console never acknowledges a
kernel chunk, because its acknowledgement belongs to the client that is waiting for it.

`console::Inserter` is what the console does with them, and the reason a kernel line cannot splice
into a terminal line. A kernel line is written only at the start of a terminal line. Arriving
mid-line (a prompt, an echo being typed) it is queued, and goes out the moment the terminal's own
writing reaches a line end. If none comes, the log service sends `console::OPERATION_FLUSH` after a
short wait. The console then puts the queued lines on a line of their own and redraws the partial
line under them.
