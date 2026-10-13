---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: a-system-log-on-redoxfs
milestone_dependencies: none
decision_dependencies: 242
machine_requirements: none
specific_machine: none
needs_person: no
---
# 687. The system log persists through RedoxFS: what a directory grant has to answer

*Title and slug are drafts.*
Raised by lane `proposal/system-log` (pull request #1423), from calef's request for a roadmap
proposal covering §242 (a system log)'s Question 5. Storage is decided as "in memory now, RedoxFS
later through a directory grant," and this is what that later half needs to answer before a
building lane picks it up. Not a build; slug provisional.

## What persists, and where

Each stored record is a JSONL line, one converted F3 record per line: `seq`, `time`, `program`,
`user`, `severity`, `msg` (§242's ruling). The persisted form is a directory grant to the log
service. It matches Fork 6 C's shape for a scheduled job's own directory grant, so the same
mechanism serves both.

Decided, 2026-09-27: one store, filtered at read time rather than split by user. The system log
and per-user logs share one directory of rotated files. The service writes every record it accepts
into the same files regardless of who wrote it. A per-user read capability filters by the `user`
field it already stamps, exactly as the in-memory ring does today. Splitting into per-user files on
disk would copy every kernel and system-service line into files it does not belong to. The volume
(below) does not yet justify that.

Two known limitations follow from one store. A reader of this proposal should meet them here
rather than discover them later.

- A shared quota lets one chatty user crowd out everyone's history (below). The quota rotates and
  drops oldest-first without regard for whose lines those are.
- Deleting one user's records means rewriting files. A user's lines are interleaved with everyone
  else's inside the same rotated file rather than isolated in one of their own.

Whether a busy durable session under milestone 152 (durable delegation) ever earns its own file is
left open. Nothing here blocks adding it later, since the read side already filters rather than
trusting file layout.

## Handoff at boot

Before the grant arrives, records live where §242 already puts them: the kernel's 16 KiB ring and
the service's 64 KiB ring, both in memory. Once the FS server offers the grant, the service
flushes the buffered records in order, as JSONL, inside one transaction (see write amplification,
below, for why one transaction beats one per line). The `seq` field is monotonic from process start
regardless of whether storage exists yet. A flushed batch's numbers therefore slot in ahead of
whatever the service appends afterward, with no renumbering.

Decided, 2026-09-27: event-driven, with no timeout on the switch-over. The service takes the
directory grant whenever the FS server offers it rather than polling or timing out. There is then
no window where a slow-but-working mount looks like a missing one. If the filesystem never comes up
for a boot, the log stays memory-only for that boot, structurally unchanged from today, since §242
already rules that persistence blocks nothing.

A fixed delay is used for exactly one thing, not for the switch-over itself. It writes a marker
line the first time the grant is overdue that boot (not "never," since it may still arrive late).
A reader of a memory-only log then knows persistence was expected rather than inferring it from
silence.

Open item for the builder, not resolved here: this proposal did not verify how the FS server
signals that a grant is ready. Establishing that signal is the first thing a building lane needs,
before the event-driven switch-over above can be wired up.

## Retention and rotation

Decided, 2026-09-27: rotated files from the start, not one growing file per boot. RedoxFS records are capped at 128 KiB regardless
of file size, so an append's recompression cost (below) is bounded either way, whether the log
lives in one growing file or many rotated ones. The deciding reason is retention instead. RedoxFS
cannot drop the front of a file without rewriting it, while deleting a whole rotated file costs
almost nothing. Roll to a new file at a size ceiling, and delete the oldest file once the directory
exceeds its quota.

Decided, 2026-09-27: fixed bytes, not a percentage of the drive. journald's own defaults scale off
filesystem size (`SystemMaxUse` at 10% of the filesystem, capped 4 GiB; `SystemKeepFree` at 15%,
same cap). That solves a problem nife does not have. The target drives (a single USB drive,
`design/decisions/0034-redoxfs-primary.md`'s topology) are small and known ahead of time.

The quota's size is provisional, for the builder to set against `notes/redoxfs-audit.md`'s numbers.
At about 3.5 MB a day (calef's figure, the raw-text rate measured below), 64 MiB holds about 18
days. A builder sizing the actual quota should check it against the *stored* rate instead, since
that is what fills the quota. That is about 5.5 MiB/day once JSONL's overhead is added (also
measured below), which holds closer to 11-12 days in the same 64 MiB. A journald-style keep-free
floor (`SystemKeepFree`) was suggested alongside this ruling but not itself ruled on. It is the
builder's to propose.

Who configures it: wherever the directory grant itself is configured, at the log service's launch,
not a manifest field on writer programs (§242 Q2's "programs declare nothing new" extends here).
What's dropped first: oldest complete file, the same oldest-out rule the in-memory rings already
use, with a dropped-count record for the gap. Age is checked only at a rotation boundary rather
than by scanning timestamps continuously. That follows logrotate's shape over journald's.

## Crash consistency

RedoxFS's own proven guarantee, from the amendment milestone 37 (prove RedoxFS's crash consistency)
earned in `design/decisions/0034-redoxfs-primary.md`, measured rather than assumed: prefix
consistency. A fresh mount after any crash recovers exactly the state after some non-decreasing
prefix of committed transactions. A torn or lost write is caught by its `BlockPtr` checksum and
refused, never returned as wrong bytes.

Applied to a log file, the failure mode at the file level is "ends after N whole lines," not a
scrambled interior. That holds so long as the service commits a whole batch of complete JSONL lines
inside one `fs.tx` closure and never splits a line across two commits. It is a rule for the service
to hold, not something RedoxFS enforces on its behalf.

Where a torn last line can still reach a reader: not from RedoxFS's crash recovery (the header
generation ring already refuses a torn header and falls back to the previous valid one). It can
come from a copy taken outside that recovery path, such as `redoxfs_host extract` reading a live or
inconsistent image. It can also come from the service itself crashing mid-write in violation of the
one-transaction rule above. JSONL's shape is the defense for exactly that case. A reader parses
forward and, on the final line only, tolerates and reports a parse failure rather than refusing the
whole file. That is JSONL's ordinary property and part of why it was chosen for the stored form
over a fixed binary frame (§242 Q4).

## Write amplification and cost

Read from `vendor/redoxfs/src/transaction.rs` (`write_node_inner_records`, around line 1882), not
recalled. Writing into an existing record reads the *whole* record and patches the touched bytes in
memory. Once the record exceeds one raw block, it recompresses the *whole* record with lz4 and
replaces it if the result is smaller. Record size grows with the node up to `RECORD_LEVEL_MAX = 5`
(128 KiB, `BLOCK_SIZE << 5`), the same figure `notes/benchmarks/filesystem-throughput.md` measured
for reads.

So one appended line into a log file whose current record already sits at that ceiling costs a
read, an lz4 pass and a rewrite of up to 128 KiB. It also pays the tree-block and header-generation
commit every RedoxFS write pays, §34 (RedoxFS is the primary filesystem). That cost is not a log's
invention. A log's pattern (many small appends) meets it far more often per byte than the
sequential 4-64 KiB writes `notes/benchmarks/filesystem-throughput.md` measured.

Recommended: batch a transaction to the service's own 64 KiB ring rather than committing per
line, flushing on whichever comes first between the ring filling and a time bound. That amortizes
both the record recompression and the header commit over a batch instead of paying each per line.
It needs no new buffer, since the ring §242 already specifies is the batch.

Bytes per day are derived from §242's own soak measurement (`bench/radon-2026-09-25/soak-8h.log`:
6,046 lines, 1,152,699 bytes over 8 hours, one chatty reporter). Scaled to 24 hours, that is about
18,138 lines and 3.3 MiB of raw text. Converting each line to JSONL (computed here, not measured on
a real build: `{"seq":...,"time":"...","program":"...","user":"...","severity":"...","msg":"..."}`)
adds roughly 117-125 bytes of fixed skeleton per line depending on the program name's length. That
lands around 5.5 MiB/day of stored JSONL at that one reporter's rate, about 65% more than the raw
text.

That is the honest cost of Question 4's structured-later tradeoff landing on disk rather than only
on the wire. A system logging only at boot-time rates (146 lines, 8,791 bytes a boot) adds
negligible daily bytes. The number that matters is set by whichever component logs most, not by
the system as a whole.

## Prior art, read where possible

Read live on 2026-09-27; one line below is marked as memory because the fetch failed.

- journald, read live (`man7.org/linux/man-pages/man5/journald.conf.5.html`). `Storage=`
  defaults to `persistent`, preferring `/var/log/journal` with a fallback to `/run/log/journal`
  (memory-only) before persistent storage exists. `journalctl --flush` or `SIGUSR1`, automated at
  boot by `systemd-journal-flush.service`, moves the volatile journal to persistent storage once it
  is available. That is nife's own "memory now, RedoxFS later" shape, including the same
  flush-on-arrival mechanism, which is evidence the pattern is sound rather than untested.
  Retention: `SystemMaxUse` (10% of the filesystem, capped 4 GiB), `SystemKeepFree` (15%, same
  cap), `SystemMaxFileSize` (an eighth of `SystemMaxUse`, capped 128 MiB), `SystemMaxFiles` (100),
  `MaxFileSec` (rotate by age, default one month), `MaxRetentionSec` (delete by age, off by
  default).
- logrotate, read live (`man7.org/linux/man-pages/man8/logrotate.8.html`). `size`, `maxsize`
  and `minsize` govern rotation by size, alone or combined with age. `maxage` deletes rotated logs
  past an age, checked only when a rotation happens. `rotate` bounds how many old copies survive
  before the oldest is dropped, which matches the oldest-first rule recommended above.
- syslog / RFC 5424: already read for §242's own prior-art table; not re-read here.
- Fuchsia persistent logs: *(memory, unverified)*. A live fetch of Fuchsia's diagnostics pages
  found how logs are generated (LogSink, debuglog) but no page on whether Archivist logs persist
  across reboot; a linked persistence page 404'd. Treat any claim about Fuchsia log durability as
  unconfirmed until someone reads the source.

## Decided

calef ruled all four questions this proposal raised on 2026-09-27 (UTC), each as a comment on pull
request #1423 (`gh pr view 1423 --comments` has the full text). The sections above carry each
ruling's reasoning where a reader meets the design it settles.

1. Rotated files from the start, not one growing file, for retention rather than
   record-recompression cost (Retention and rotation, above).
2. One store, filtered at read time by the stamped `user` field. Two known limitations are recorded
   where they apply (What persists, and where, above): a shared quota lets one user crowd out
   everyone's history, and deleting one user's records means rewriting files.
3. Event-driven, with no timeout on the switch-over; a fixed delay is used only for the
   persistence-expected marker line. The FS server's grant-readiness signal is an explicit open
   item for the builder (Handoff at boot, above).
4. Fixed bytes, with the quota's size left provisional at about 64 MiB (roughly 18 days at the
   measured 3.5 MB/day rate) for the builder to set. A journald-style keep-free floor was suggested
   but not ruled on (Retention and rotation, above).

## Index row

DECISIONS §242 stores the system log in memory now and in RedoxFS later through a directory grant. Proposed: the later half, with what the directory grant has to answer before a building lane picks it up.
