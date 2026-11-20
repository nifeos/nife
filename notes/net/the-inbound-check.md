# The network stack: the inbound check's intermittent red, as it was investigated

*An appendix to [`notes/net.md`](../net.md), which is the page to read. This file holds the history
of the inbound check's intermittent failure, up to the day before its cause was identified. It
exists to verify or challenge the main page. A reader who only needs to use the socket contract
should not have to open it. The directory `notes/net/` and this file's stem are provisional names;
naming is calef's.*

The state of it. `inbound check (riscv64)` has gone red twice with nothing wrong that anybody
could name, and the floor has already been lowered once (from all four to three) to absorb it. It
must not be lowered again: at three it still proves each of the two listeners answered, and at two it
proves nothing that one listener could not fake. A second lane went hunting on 2026-08-19 and did not
find the mechanism either. What follows is what that lane ruled out and what it measured, so the next
person starts where it stopped rather than where it started.

The two observations. Run 32195227733's riscv64 leg served 3 of 4 with all 279 guest tests
passing, on a runner the load instrument called not oversubscribed. On 2026-08-19, on pull request
348, a leg served 2 of 4, below the floor, with the load average at 1.05 on four cores. So
contention is not the explanation, and the second one is not the same shape as the first.

Ruled out: the teardown race. The prober's `report()` sets `stop` after the child exits, and a
connection still waiting at that moment is lost. It cannot be this: the four rounds are answered
around 30 s and 39 s into a boot that runs about 180 s, so the last one has well over two minutes
of slack before QEMU goes away. Measured over five boots, all four rounds every time.

Ruled out: the guest quietly serving fewer than four. Every guest path that serves fewer than two
rounds is loud. `socket_test_client::serve_one_inbound` calls `done(0xE060)`/`done(0xE070)` when
`ACCEPT` returns `REP_ERR`, and `std_exerciser::serve_one_inbound` panics, because the overlay's
`accept` turns the bounded wait's expiry into `WouldBlock` and the exerciser unwraps it. There is
exactly one silent path, and it is worth knowing: `virtual_io_service::start_net_std` and its SMB
sibling begin with `find_net_device()?`, so a leg that cannot find the NIC prints
`(no virtio-net device attached; skipping)` and the test passes having offered nothing. That would
produce 2 of 4 on the nose, which is the second observation's number.

How to tell those apart in one look, which is what the prober now prints. The four rounds come
from two listeners in two windows about eight seconds apart, two rounds each, so the answers cluster.
The prober records every connection's outcome and the timestamp of every answer, and prints the
summary on a green run too. That is the half that was missing both times this went red: nobody
had a known-good shape to read the red one against. A green riscv boot looks like this, and it is
stable to within a few hundred milliseconds across five of them:

```console
inbound prober (port 55788): answered x4, closed-empty x1, connect-failed x2, reset x15
    +30179 ms: answered after 29973 ms, 9 bytes
    +30190 ms: answered after 11 ms, 9 bytes
    +38671 ms: answered after 5634 ms, 9 bytes
    +38674 ms: answered after 2 ms, 9 bytes
```

Two clusters with a round missing means the host lost one the guest served, and the outcome
recorded beside it says how (`reset`, `closed-partial`, `stopped-while-waiting`, `wrong-bytes`). One
cluster means a whole listener never ran, and the `skipping` line above is the thing to grep for.

**On the HVF leg the same timeline carries the guest's side** (milestone 514 (the HVF std listener
hangs in some full runs), 2026-10-12 UTC). That leg relays the transcript line by line, so it stamps
the two listener tests' own lines on the prober's clock. They are the hand-written test's census
line (its `net_stack` is up), the std program's output as its test echoes it, and each test's
verdict. `std listen| in use refused` is the last line before the std program's first `accept`. Each connection line also says
when it was opened. A green HVF run, abridged:

```console
    +21363 ms: guest: test ...::a_host_process_connects_to_the_guest_and_is_answered ...     (E2 thread census: ...
    +31756 ms: answered after 31548 ms (opened +208 ms), 9 bytes
    +31756 ms: answered after 0 ms (opened +31756 ms), 9 bytes
    +33141 ms: guest: std listen| in use refused
    +38011 ms: answered after 6255 ms (opened +31756 ms), 9 bytes
    +38013 ms: guest: std listen| served 2
```

The TCG legs (`xtask/src/scanout.rs`) do not hand their lines to the prober yet, so their
timelines have connections and no guest lines.

What that transcript also shows, and it is the most alarming number in this section. Look at the
hold times. The connection that serves round one is opened at boot and answered 29973 ms
later; the one that serves round three is held 5.6 s. The prober offers the guest exactly one
connection at a time and never abandons it, for the good reason [the-inbound-half.md](the-inbound-half.md) gives. So throughout
those 30 seconds slirp is retransmitting a single SYN into a guest with no stack yet, backing off
as it goes: roughly 1, 2, 4, 8, 16, 32 seconds. The guest's `ACCEPT` waits `service_until`'s
15 s and then gives up.

So the gate's first round lands because a SYN whose retransmit interval had grown to about 32 seconds
happened to arrive about a second after the listener came up. That is a one-second margin against a
fifteen-second cliff, on a timer nothing here controls, and it is the same shape on every ISA and
every runner. What differs between this laptop and CI is only where in the backoff the guest's
listener happens to appear. It has not been shown to be the mechanism of either observed failure. It
is a measured hazard the design has, it is enough on its own to make the leg's behavior depend on
an emulator's retransmit timer, and it is where the next lane should look first.

The proposal that follows from it, not taken here because it could not be measured. Offer the
guest several connections at once, staggered, all held and all read. Every answer is still
collected (nothing is abandoned, so the never-abandon rule in [the-inbound-half.md](the-inbound-half.md) is kept). But no window ever depends on a SYN
timer that has been backing off for half a minute: there is always a recently-opened connection with
a short one. The extras hit `net_stack`'s one-deep backlog and are reset, which costs nothing and the
prober already handles. It was left undone deliberately: with no reproduction, a green run after the
change is indistinguishable from a green run before it, and this check has been widened once already
on evidence that thin.

And the aarch64 leg is closer to the cliff than riscv is, which is worth knowing because the
flake has only ever been seen on riscv. On the same laptop its first round is held 41998 ms
before it is answered, against riscv's 29973. Its prober logs twenty `connect-failed` attempts
at the start of the boot because it is poking the forwarded port before QEMU has bound it. Nothing
about this hazard is riscv-specific; riscv is simply where it has landed so far.

Why it does not reproduce on a developer machine. Five riscv boots on macOS on a quiet Apple
Silicon laptop: 4 of 4 every time, with the timings above, plus one full two-ISA `script/test`. The lane that built the check got 0 in 5
as well. CI runs on `ubuntu-24.04-arm`, so the QEMU build, the libslirp version, the host TCP stack
and the core count all differ, and this is an emulator-timing failure. A reproduction attempt that
is not on that runner class is not a reproduction attempt; that is the single most useful thing to
know before spending an afternoon on it.

The runner class was brought to the laptop on 2026-08-19, and the result is one notch short of a
reproduction. `script/runner-container` boots the leg inside an ubuntu 24.04 aarch64 container on
the developer machine, with `.qemu-version`'s QEMU 11.0.2 built by `script/ci-qemu` (the workflow's
own script, not a package), four processors by affinity, and no induced load. Fifty boots:

- The gate never went red. 50 of 50 green, which is a real if narrow statement. At the roughly
  one-in-six rate this failure is estimated to have on CI, fifty clean boots have probability
  1.1e-4, so whatever the container is doing, it is not doing that. Read as a bound rather than as a
  refutation: 0 of 50 puts the 95% one-sided ceiling on the per-boot red rate at 5.8%, so a
  one-in-twenty flake would have sat inside this run unnoticed.
- The loss itself reproduced once, and that is the finding. Run 45 of 50 collected 3 of 4,
  which is the first CI observation's shape exactly, on a boot whose 279 guest tests all passed. It
  did not turn the leg red only because the floor is three. One in fifty is 2%, 95% CI
  [0.05%, 10.65%], which overlaps the CI estimate at its top end and is a point estimate to
  distrust: it rests on one event.

The trace, beside a green one from the same fifty for comparison:

```console
inbound prober (port 34033): answered x3, closed-empty x2, connect-failed x1, read-failed x1, reset x4
    +15119 ms: read-failed after 15019 ms, 0 bytes
    +21097 ms: answered after 5877 ms, 9 bytes
    +27105 ms: answered after 5579 ms, 9 bytes
    +27108 ms: answered after 3 ms, 9 bytes

inbound prober (port 38119): answered x4, closed-empty x1, connect-failed x1, reset x3
    +18071 ms: answered after 17969 ms, 9 bytes
    +18082 ms: answered after 11 ms, 9 bytes
    +24066 ms: answered after 5527 ms, 9 bytes
    +24070 ms: answered after 3 ms, 9 bytes
```

Read it the way the failure text says to. Two clusters, and the first one has one answer where it
should have two, so the host lost a round the guest served. The outcome beside it names how, and it
is not a timeout: `read-failed` is `probe_inbound`'s catch-all `_ =>` arm, reached only by an error
that is not `WouldBlock`, `TimedOut`, `ConnectionReset`, `ConnectionAborted` or `BrokenPipe`. The
never-abandon rule did not protect this connection, because a hard error is exactly the case that
rule allows to give up on. The paragraph in [the-inbound-half.md](the-inbound-half.md) about abandoned requests still being executed
then applies in full. The guest answered into a socket whose host end had gone.

The connection died at 15019 ms, held from 100 ms into the boot. That number is not obviously a
coincidence next to `service_until`'s 15 s, and pinning it is the next lane's first job.

What blocks pinning it today, and it is a two-line fix somebody should make before the next
hunt. The errno was captured. `probe_inbound` formats it into `last`, and `last` is printed only
when the check FAILS. This run passed, so the one fact that would have named the mechanism was
computed and thrown away. `InboundTrace` should keep the error string on the event rather than only
the label. Not done here on purpose: this lane was measuring, and changing the prober mid-measurement
would have made the fifty boots incomparable with each other.

The timing is bimodal, deterministically so, and nobody had looked before. Across the fifty
boots the first round is answered at either 17.95-18.04 s (24 boots) or 29.97-30.08 s (25
boots), with nothing in between and under 100 ms of spread inside each mode; run 45 is the only
outlier. The whole boot follows it, taking ~49 s or ~62 s. Twelve seconds apart is a retransmit
ladder, and the near-even split says the guest's listener comes up right on top of one of its
rungs and falls to one side or the other essentially at random. That is the previous lane's
one-second margin, measured from the other end and confirmed: the leg's behavior is decided by
which rung of an emulator's SYN backoff the listener happens to appear next to. It also means the
laptop's steady 29973 ms is one of two modes rather than the number, so a fix should be judged
against both.

What the container matches: the distribution and release, hence libslirp and the scheduler tick.
It matches the emulator, from the same `script/ci-qemu` and the same pin. It matches the provisioning, from
`script/bootstrap`. And it matches the core count, narrowed by affinity rather than by a CFS quota, which is the
difference between four processors and four processors' worth of throttled time on eight.

What it does not match, and any one of these could be where the missing rate lives. The kernel is
the podman machine's Fedora CoreOS, not the runner's Ubuntu one. The container is nested inside a VM
on a laptop rather than being the VM. Memory is the podman machine's rather than the runner's 16 GiB.
And the image is `ubuntu:24.04` rather than a `runner-images` build. A reproduction here would have
been strong evidence; a clean run is weak evidence, and the honest reading of 50 green is that this
narrows the search rather than closing it.

Where the next person should start. Keep the errno, then re-run this. If the rate stays near 2%,
the remaining gap to CI is in the four unmatched things above and the cheapest of them to test is
memory. The proposal in the paragraph below (several staggered connections at once) is now measurable
for the first time. Run the fifty boots before and after it and compare the count of lost rounds.
That is a number this instrument produces on green runs and the old evidence never had.

## The CPU matrix's reds, read against milestone 514 (the HVF std listener hangs in some full runs)

Written 2026-10-13 (UTC). The CPU matrix's riscv64 legs went red on networking twice on 2026-10-12.
A search of CI from 2026-08-17 to 2026-10-13 counted them: every failed run, and every run that
went green on a rerun, 268 failed matrix jobs in all. Three were network failures with nothing
earlier in the boot to explain them:

- 2026-09-26, `thead-c906`, on the pull request of milestone 595 (the shell runs a `std`
  program), green on rerun: 2 of 4.
- 2026-10-12, `rva23s64`, #1967's merge group (run 38222292949): 2 of 4.
- 2026-10-12, `thead-c906`, #1964, green on rerun: `std_resolve`, a different cause (below).

The first two are milestone 514's shape on TCG. The prober opened its third connection within a
millisecond of the hand-written listener's second answer and held it to the end of the run. The
std listener's test hung into the 60 s watchdog. Every other inbound red in those jobs had another
cause: an unrelated panic or hang earlier in the boot, a branch's own change, or the `read-failed`
cluster of 2026-09-20 and 21 (the EINTR fixed on 2026-09-24).

**#2029's fix covers this path.** The CPU matrix runs the same `net_stack`, listener client,
`std_exerciser` and prober as the HVF leg. It was forced the way 514 forced it, with 300 ms of
polling before a listener's close. With #2029's reset taken out, both models failed in that exact
shape (2 of 2; on `rva23s64` the connection opened at +109028 ms was held 96 s). With the reset and
the same forcing, both were green (2 of 2), and the prober reconnected about 400 ms later.

The race is routine, not rare. The prober now prints a `closed-empty` line, and all six unforced
runs with it (three per model, all green) show the race. The third connection closed empty 2 to 7
ms after the second answer, and a new one opened about 100 ms later. The reset makes it harmless.

Why it was a watchdog and not a failed accept. `net_stack`'s 15 s bound only ended a wait when a
frame or a smoltcp timer woke it. A connection held to nobody sends nothing, so the std listener's
accept never expired and its retries never ran. Fixed 2026-10-13 in `components/src/nic_wait.rs`,
held by `an_accept_nobody_answers_ends_at_the_stacks_bound`.

The rate is too low to measure locally. Roughly 1,800 CI runs ran the matrix between 2026-09-26 and
2026-10-12, five models each (estimated from a 25-run sample), for two reds: about one in 4,500
boots. That is why the forced pair is the evidence.
