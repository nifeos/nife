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
