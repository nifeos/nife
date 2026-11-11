# An interrupted read: 2026-09-24

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Written by the `fix/inbound-check-flake` lane, pull request #1244. The
check's longer history is [notes/net/the-inbound-check.md](../net/the-inbound-check.md).)*

## The reds

`inbound check (riscv64)` failed five times with the guest suite otherwise green. Four were in
the `cpu matrix` job and one in `build + test`, all on 2026-09-20 and 09-21. Each served 2 of 4
against a floor of 3. Each trace had the same shape:

```
+5066 ms: read-failed after 4665 ms, 0 bytes
+20146 ms: read-failed after 14979 ms, 0 bytes
+49888 ms: answered after 29641 ms, 9 bytes
+49895 ms: answered after 7 ms, 9 bytes
+58486 ms: reset after 8486 ms, 0 bytes
```

Two held connections ended in `read-failed`, which is the read loop's catch-all for an error it does
not name. notes/net/the-inbound-half.md already had the mechanism for what follows. Dropping a connection does not
take back the payload already written, because slirp delivers it when the guest next polls. The
guest then serves a round into a socket nobody holds.

## Fixed, or rare?

Rare, and the QEMU pin was not the fix. The reds stopped when `.qemu-version` moved from 11.0.2 to
11.1.1 (#1086, merged 2026-09-22 00:22), so this lane checked the loss itself rather than the reds.
It sampled 150 green `build + test` logs on each side of the pin and read the prober traces in them:

| QEMU | riscv64 traces with `read-failed` | riscv64 traces that lost a round (3 of 4) | aarch64 lost a round |
|---|---|---|---|
| 11.0.2 | 16 of 150 | 13 of 150 | 2 of 150 |
| 11.1.1 | 26 of 150 | 9 of 150 | 2 of 150 |

The loss is as common after the pin as before. The floor of three absorbs one lost round, so a red
needs two in a boot. The reds also came in a burst: none in about 2,360 riscv64 boots from 09-14 to
09-19, five in the next two days. After the pin, 0 reds in about 2,370 boots bounds the red rate
below 0.13% per boot (rule of three). That is about the old average, so the silence proves nothing.

## The errno

The first commit here kept the error on each trace event instead of only in the red-run message. One
dispatched run (36059579060) with the matrix logs uploaded caught it on `rva23s64`:

```
+35390 ms: read-failed after 35089 ms, 0 bytes (Interrupted, os error Some(4))
```

`EINTR`. A signal landed on the blocked `receive`, and the prober treated that as the connection's end.

Every `read-failed` in two weeks of CI traces, 60 of them, landed 16 to 454 ms after a multiple of
five seconds from the prober's start. That is `HostLoad`'s sampling grid: it spawns `uptime` every
five seconds from the same loop. The offset grows with each sample, as that loop's 100 ms drift
predicts. Which signal it is was not identified. On cordoba (Linux 7.0), neither a C nor a Rust
program spawning `uptime` beside a blocked read produced an `EINTR`. Nor did a Rust prober against a
real QEMU 8.2 `hostfwd` port. So the source needs something this lane did not reproduce.

## The fix

`read_error_is_not_yet` in `xtask/src/inbound.rs` now counts `Interrupted` with the read timeout's
two kinds. A signal means read again, never "this connection is over". This is standard for any
blocking read in Rust. Two host tests pin it. `an_interrupted_read_keeps_the_connection` fails with
`Interrupted` removed. `a_reset_still_ends_it` checks the fix cannot turn a reset into an endless
wait. No other read loop in `xtask` sets a read timeout, so nothing else has the same exposure.

The fix does not depend on naming the signal. Whatever sends it, the connection it interrupts is
healthy.

## BUGS

The rate after the fix is not measured. An `EINTR` is now retried silently, so the trace no longer
shows it. The evidence is the errno and the host tests, not a before-and-after count. A lost round
after this change would be a different mechanism, and its errno will now be on the trace.

The signal's source is unknown. If it is the sampler's child, any other blocking syscall in `xtask`
could see it. The ones in `std` that matter, such as `connect_timeout` and `write_all`, already
retry `EINTR`.
