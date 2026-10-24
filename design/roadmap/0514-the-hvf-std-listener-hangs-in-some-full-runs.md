---
status: BUILT
raised: 2026-09-19
built: 2026-10-12
promoted_from: the-hvf-std-listener-hangs-in-some-full-runs
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 514. The HVF leg's std listener test hangs in some full runs and passes alone

Promoted from the proposal `the-hvf-std-listener-hangs-in-some-full-runs`, filed 2026-09-19, on
calef's instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the
proposal's own, unedited except for this paragraph: the argument is its author's and promotion is
not the moment to improve it. Found by milestone 227 (a GICv3 driver, because GICv2 boots and)'s
lane (a GICv3 driver), on the first two full `script/test --hvf` runs this machine could make since
milestone 222 (the one command a person runs before).

It needs an Apple Silicon Mac with HVF, and nothing else.

## In brief

`kernel::user::tests::a_std_program_serves_a_granted_listening_port` hung in two of the three full
`script/test --hvf` runs that reached it (the third was green end to end), and passes when run alone under HVF
(`cargo xtask test --hvf --test a_std_program_serves_a_granted_listening_port`) and in every TCG run,
GICv2 and GICv3 alike. It is the one flaky test left on the HVF leg, and `script/ci-build` names it when the
leg fails so a contributor can tell it is not theirs. That sentence should go when this lands.

What the transcript shows, in order:

```
test kernel::user::tests::a_std_program_serves_a_granted_listening_port ...
  user thread 4294967418 killed: BRK instruction
    pc 0x000000000040e7dc   far 0x0000000000a50000   user sp 0x0000000000500b10   esr 0xf2000000
  the kernel is fine.

WATCHDOG: no progress for ~60 s. Every core idle, every thread blocked: a lost-wakeup hang.
```

and after the run, from the host's inbound prober:

```
inbound check (aarch64) FAILED: the guest served 2 of the 4 inbound connections ...
    +18822 ms: answered after 18613 ms, 9 bytes
    +18823 ms: answered after 1 ms, 9 bytes
    +109147 ms: stopped-while-waiting after 90324 ms, 0 bytes
```

`0x40e7dc` in `std_exerciser` is `__rust_abort`, so the std program panicked, and its message
went nowhere a transcript reads.

## The likeliest reading, which is not established

`serve_one_inbound` (`std_exerciser/src/main.rs`) panics with "round N: nobody connected" when
`accept` returns an error, and `std-nife`'s `accept` returns `WouldBlock` when `net_stack`'s
bounded wait expires. `probe_inbound` (`xtask/src/main.rs`) holds each connection it opens until it
is answered or the run ends. Suppose the prober opened a connection in the gap between the
hand-written listener's window and the std one, and that connection is never delivered to the std
listener. The prober waits on it for the rest of the run (the 90-second `stopped-while-waiting`
above), and the std listener's bounded wait expires with nobody connecting. HVF runs the suite about
three times faster than TCG, which reshapes that gap. None of this was confirmed by
instrumenting it.

## Two defects may be here, and the second is independent of HVF

1. **The timing**, as above: whatever it is, it is HVF-shaped.
2. **A kernel test hangs rather than fails when the program it waits on dies.** The test waits for
   a report on a rendezvous; the std program is dead; the watchdog fires 60 seconds later and calls
   it a lost wakeup, which it is not. A test that saw its reporter killed and failed at once, saying
   so, would have turned this into a one-line red.

## What would settle it

Capture the std program's panic message (its stderr reaches no transcript today), and log the
prober's connection opens against the guest's listen and accept times on one clock. If the reading
above holds, the fix is on the host side of the prober or in the listener's retry, not in the
kernel.

## What the evidence showed (2026-10-12 UTC)

Both instruments the block asked for were built first, and the evidence then picked the fix.

- The panic message. It was never lost: std's stderr is the same sink as stdout, and the kernel
  test had already received the message into its buffer. It only blocked forever waiting for an end
  of stream that a dead program never sends. Fixing defect 2 (below) prints it.
- One clock. The HVF leg now stamps the listener tests' own lines on the prober's clock, and
  each connection line says when it was opened (notes/net/the-inbound-check.md has a green run).
- **The cause.** The prober opens its third connection within a millisecond of the hand-written
  listener's second answer. `net_stack`'s `tcp_accept` re-arms before it returns, so that
  connection can complete its handshake into the re-armed socket, and the hand-written client's
  `CLOSE` then dropped the socket unpolled without a reset. The prober held an established
  connection to nobody, and offered nothing else, while the std listener's `accept` ran out. That
  is the reading above, sharpened: the gap was not between the two listeners but inside the first
  one's close. Forced on HVF by making `net_stack` poll for 300 ms before closing a listener, a
  `network_tests` run failed in exactly the recorded shape (the connection held 91 s, the std
  listener to its 90 s test budget). With the fix and the same forcing it was green twice, the
  prober reconnecting about 400 ms later.
- The std listener's retry does not cover it. Milestone 649 (each socket is its own capability)
  gives `std_exerciser` eight 15 s accept windows. The forced run above had them and still failed,
 because the prober opens nothing new while it holds a
  connection.

## What was built

1. **`net_stack` resets a peer whose handshake landed in a listener being closed**
   (`close_tcp`, in a new child `components/src/tcp_close.rs`, since §282 (when a file is split,
   the parent becomes an index) lets `net_stack.rs` only shrink). `abort` and one poll instead of
   `close`, which is what Linux does to a closed listener's unaccepted connections.
2. **A std test fails at once when its writer dies** (defect 2): `drain_sink_from` in
   `system_tests/src/user/std_tests.rs` binds a notification to the test thread, ticks it every
   100 ms, and on a tick asks whether the writer's thread still exists. A dead writer fails the test
   with everything it wrote, panic message last. `start_net_std` hands back the writer's thread, and
   the four std net tests (two per ISA) use it. Proven by making `std_exerciser` panic in round 1:
   the test failed with `INJECTED: round 1 dies on purpose` in the same second.
3. The one clock (`InboundProber::mark`, `xtask/src/inbound.rs`), and the std test echoes the
   program's lines as `std listen| ...` so they can be stamped. `std_exerciser`'s accept panic now
   says how long it waited.
4. The hand-written listener re-arms too (`ACCEPT_TRIES`, five 15 s windows, in
   `components/src/socket_test_client.rs`). Not the cause, but measured beside it. The prober's
   first connection is opened at boot and slirp backs its SYN off to 16 and then 32 s gaps, and
   every run here answered it about 10 s into a single 15 s window.
5. `script/ci-build`'s sentence naming this test as a known flake is gone, and notes/hvf-leg.md,
   notes/multicore-defect-curve.md (D13) and notes/virtualization.md (stale since milestone 227
   (a GICv3 driver, because GICv2 boots and silently loses every interrupt)) say what is true now.

## Verified

Full `script/test --hvf` on patagonia (Apple M-series, QEMU 11.1.1), one at a time, each started
only once the 1-minute load was under 4 and no other QEMU ran: thirteen runs. Ten were of the fix
before `close_tcp` moved into its child module and three of the final tree. Twelve were green end
to end (347 passed, 13 skipped, the scanout and inbound checks passing), with the std listener test
passing each time, about 90 s a run. One stopped earlier, at
`login_user_schedule_tests::suspending_a_user_ends_their_schedule_and_refuses_them_until_resumed`
(`alice was not authenticated`), before any network test ran; that test's BUGS records it.

Read the green count with its caveat. Four more full runs on the way here (one on the base commit,
three with the instruments but before the `net_stack` fix) were green too. So at this load the
unforced hang was rarer than the two in three of 2026-09-19 or the three in six of 2026-09-24 (load
14 to 25). Twelve green runs therefore say little alone. The evidence that the fix closes the cause is
the forced pair: red without it, green with it, same forcing.

## BUGS

- A writer that is alive but stuck is now reported as a livelock. The watch's ticks are wakes,
  and wakes are progress to the lost-wakeup watchdog, so such a test runs to its budget (90 s) and
  fails as over budget rather than at 60 s as a hang. `drain_sink_from`'s BUGS has it.
- The other `drain_sink` callers still hang on an aborting writer, because their starters do not
  hand back the writer's thread. Each needs only that.
- `std_exerciser`'s eight accept windows (120 s) are more than the 90 s test budget. The budget
  ends the test first, so the last two windows never run on this leg.

## Follow-on

- **None.** The BUGS above that are work (more `drain_sink` callers, the window count) are each a
  few lines for whoever next touches those tests, and the login flake is recorded at its test.

## Index row

`kernel::user::tests::a_std_program_serves_a_granted_listening_port` hung in two of the three full
`script/test --hvf` runs that reached it (the third was green end to end), and passes when run alone
under HVF (`cargo xtask test --hvf...
