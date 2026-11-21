---
status: BUILT
raised: 2026-09-27
built: 2026-10-02
milestone_dependencies: 126
decision_dependencies: 242
machine_requirements: none
specific_machine: none
needs_person: no
---
# 613. A system log service: the in-memory half

Minted 2026-09-27 by the maintainer against §242 (a system log),
`design/decisions/0242-a-system-log.md`. calef ruled §242 the same day, on pull request #1423, now
merged. The title and slug are drafts. The number is not.

## Scope

One service. It holds a badged endpoint for writers and a separate endpoint for readers. It stamps
every record's `seq`, `time`, `program`, `user` and `severity` from the writer's badge, never from
anything the writer sends. Writers speak F3: the byte sink framing, plus a body-kind byte (§242,
Question 4). Severity rides in `byte_sink_protocol`'s unused first-word bits. Readers get JSONL, one
JSON object per line, converted from F3 by the service. The service keeps a 64 KiB in-memory ring,
oldest out, with a dropped-count record on overflow. A per-user read filters that one ring by the
stamped `user` field. There is no per-user store. The service also drains the kernel's own ring and
forwards whole lines to the console. It becomes the one thing writing kernel output there.

Out of scope: persisting any of this to RedoxFS. That has its own proposal,
[`design/roadmap/0687-a-system-log-on-redoxfs.md`](0687-a-system-log-on-redoxfs.md). Also
out of scope: the kernel-side ring and its drain system call. Those are milestone 342 (the kernel and
the `console` server drive one UART from two address spaces).

## What was built, 2026-10-02 (UTC)

By lane `milestone/613-system-log-service`. Every name below is provisional: `system_log` (the
crate and the program) and `system_log_protocol` (the wire contracts).

- `crates/system_log_contract`: the F3 header (32 bytes, then at most 224 of text), syslog's eight
  levels, the spawner's control words and the reader's window layout.
- `nifeos/byte_sink_contract`: §242's amendment. Bits 39:32 of a bytes message's first word carry
  the level plus one, so 0 still means "not set". Bits 47:40 carry the body kind, and 0 is text.
- `crates/system_log`: stamping from the badge, one partial line per writer (eight at once), the
  in-tree JSONL writer, and the 64 KiB ring with its per-user filter and dropped-count line.
- `components/src/system_log.rs`: the receive loop. One thread blocks in one `RECV`.
- The kernel: **a plain `SEND` now carries its capability's badge to `RECV` in `x3`.** §230
  (badged endpoint capabilities) delivered a badge on `CALL` and `SEND_CAP` only. A byte-sink
  writer `SEND`s, so the badge §242 stamps from never reached the service. This changes the
  system call surface: `RECV`'s `x3` for an ordinary message was always 0 and is now the badge (0
  when unbadged). It needs a line in §230 from the integrator.

Three choices were made here rather than ruled, and each one is reversible until a second program
links it:

1. The spawner speaks on the writers' endpoint, as badge 0. Only the minter holds the unbadged
   capability, so a badge-0 message can only be the spawner registering what a badge means:
   program, user, inferred severity, reader window. A second endpoint would need a second wait
   point, and this service has one thread.
2. Readers read through a shared page and a notification, not a reply. A reader `SEND`s a
   cursor through its own badge. The service fills that badge's window with whole JSONL lines and
   `SIGNAL`s the reader. The service never blocks on a reader, which is §242's "dropping rather
   than letting a writer wait". A `CALL` reply carries two words, and a reader-supplied byte sink
   would let a stalled reader park the service.
3. The ring stores rendered JSONL, with the stamped user kept beside each line, so the
   per-user read never parses JSON.

**Proof.** 20 new host tests across the three crates, plus a doctest: stamping, F3 framing including
the severity bits, the JSONL conversion against hostile text, interleaved writers, eviction and the
dropped line. Also one QEMU test,
`system_log_tests::two_badged_writers_are_attributed_by_the_badge_and_a_per_user_read_filters`.
It runs two `sink_transcript_writer`s under badges 1 (alice) and 2 (bob), printing identical
text. The system reader sees two lines stamped `source` 1/alice and 2/bob, and alice's per-user
read sees one. It passed locally on aarch64, riscv64 and x86_64.

**Not done here, and why.** Nothing starts the service at boot or forwards kernel lines to the
console yet. Milestone 342 (the kernel and the `console` server drive one UART from two address
spaces) is the first customer that needs it running, and its kernel ring is what would feed the
forwarding. `Log::ingest` is that entry point. The service's own `BUGS` sections record the rest.

## Dependencies

- §242 (a system log), DECIDED 2026-09-27.
- Milestone 126 (the `procps` package: who else is running), pull request #1360. It raises the
  progenitor's capability table to 32 slots. §242 counts the progenitor at 23 of 24 today. A minting
  endpoint for this service needs the spare slot #1360 adds.

## What this unblocks

- Milestone 342 (the kernel and the `console` server drive one UART from two address spaces). It
  names this service as the drainer its kernel-side ring needs before it can build.
- A home for a scheduled job's output and refusals: milestone 152 (durable delegation), pull request
  #1377. Its Fork 6 option C grants a job's declared diagnostics stream to this log, instead of
  discarding it.
- The notice board's history, proposed in pull request #1424. Its own section number is provisional
  and not yet landed.

## Done means

- Host tests, no emulator: stamping (the service, not the writer, fills `seq`, `time`, `program`,
  `user`, `severity`); F3 framing, including the severity bits amending `byte_sink_protocol`; and
  the F3-to-JSONL conversion.
- A QEMU test with two differently badged writers and one reader. The reader must see both writers'
  lines, correctly attributed to each, on aarch64, riscv64 and x86_64 per §19 (architectural parity
  is a tenet).

## Follow-on

- **Milestone 342.** Starting the service at boot, minting log badges for the progenitor's
  children, and forwarding kernel lines to the console. Its kernel ring is the thing to forward.
- **Decision.** `design/decisions/0230-badged-endpoints-name-a-callers-frame.md` owes a line: a
  plain `SEND` now delivers its badge in `RECV`'s `x3`. The integrator mints it, per the shared
  state rule.
- **Milestone 687.** Milestone 687 (the system log persists through RedoxFS: what a directory grant has to answer). `design/roadmap/0687-a-system-log-on-redoxfs.md`, persistence.
- **Recorded.** `crates/system_log/src/lib.rs`: a per-user reader's dropped-count line counts
  every user's evicted records, `time` is stamped when a line completes, and the ninth concurrent
  partial line splits the quietest one.
- **Recorded.** `crates/system_log_contract/src/lib.rs`: control words carry 32-bit badges, and
  names are cut at 32 bytes.
- **Recorded.** `components/src/system_log.rs`: four readers at most, and the one-page stack a
  kernel spawn gives it has not been measured on the crowded-writer path.

## Index row

§242 (a system log) ruled one service that the kernel and every program append to. It stamps
attribution from the writer's badge, because the writer's own claim cannot be trusted. This
milestone is that service's in-memory half: F3 binary framing in, JSONL out, a 64 KiB ring, and
per-user reads filtered by the stamped `user` field. Kernel lines are forwarded to the console as
whole lines. RedoxFS persistence and the kernel's own ring are separate work. This is what unblocks
milestone 342's console arbitration, a home for a scheduled job's output, and the notice board's
history.
