# The confined fuzzer's conductor, in detail

An appendix to [notes/system-test-modules.md](../system-test-modules.md). The module doc of
`system_tests/src/user/confined_fuzzer_tests.rs` names the conductor's oracles and its `BUGS`. This
page states each oracle's rule. The milestone is [milestone 779 (fuzz the surface a confined process
can reach)](../../design/roadmap/0779-fuzz-the-surface-a-confined-process-can-reach.md).

The generator never interprets an answer. It draws one syscall from its seed, makes it through the
real trap entry, and waits on its slot 0 for the next `GO_STEP`. Every judgment is the conductor's,
made between calls, so a wrong answer cannot steer what the generator does next.

## The oracles

- The table diff. A slot may empty freely. It may fill only four ways: `RETYPE_OBJ` (the object's
  page in the fuzzer's own region), `BADGE` (the same object, restamped), `SLICE` (a sub-run of a
  held frame at its rights, from a capability holding `GRANT`), or `RECEIVE_CAP`. A `RECEIVE_CAP`
  fill must be a delegation the conductor watched leave a sender, no wider and with the same badge,
  or the `Reply` of a parked caller. Any other fill, a plain `RECEIVE` included, is an escape.
- The delegation ledger. It records each parked `SEND_CAP`, marks it dead on revocation, and drops
  it when its sender returns. A fill naming a dead or unknown delegation is an escape (#1525).
- Register truth. After `RECEIVE_CAP`, `x1` names the slot that filled or is `NO_CAP`, and `x4`
  carries `REPLY_DELIVERED` exactly when the fill is a `Reply`.
- The badge. Everything drained off the server endpoint carries a fuzzer's badge or one it minted
  under the conductor's eye. Badge 0 is the shape of #1494.
- The witness. It holds only slot 0, which is never named, so every `SYS_INVOKE` it makes must fail.
- The page oracle. Every mapping lies in the spawned pages or the 32-page playground, and every
  frame behind them lies in a granted region or one of the two gift frames.

## Serving a call that blocks

A call that blocks is served, keyed on the parked thread's observed state and never on timing: sends
are drained, receives released, `CALL`s answered and the notification signalled. Only a call that
neither completes nor parks somewhere serviceable is a finding.

Every service is one `sched::conductor_move`, an operation that can never park the conductor. A
check-then-act service around a blocking IPC primitive races on four cores: the party the check saw
can be gone by the act. A parked conductor is a dead suite even though the kernel is fine, because
every fuzzer is parked on its GO and nobody is left to wake the judge.

Only the stepped fuzzer is ever served, which also keeps the audit tractable. At every quiet point
each fuzzer is parked on its GO and only the stepped one makes calls, so the one thread that can be
blocked mid-call is the one being served. A fuzzer retypes endpoints and notifications of its own
and parks on them; each such park is legitimate kernel behaviour, and the conductor releases every
one the same way.

## What it cannot reach

No fuzzer can bind an interrupt or hold a supervision channel, so interrupt and death deliveries
never happen. The step protocol also closes the orders in which two fuzzers' messages can arrive.
Four recorded falsifications depend on exactly those, so this module cannot turn red under them;
`receive_cap_attack_tests` keeps those records.
