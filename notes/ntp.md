# NTP: the wire format, and the client that carries it

Two halves, built a day apart. The wire format is `crates/network_time_protocol` (milestone 51 (wall-clock time,) lane C): the
48 bytes of RFC 5905, the 1900-epoch fixed-point timestamp, the offset arithmetic, and the handful of
checks that are the whole of unauthenticated NTP's spoofing resistance. Pure computation, no socket,
no clock, no service, and its tests run in milliseconds on the host. The client is
`components/src/network_time_client.rs` (milestone 51 lane D), the process that turns those bytes
into a clock correction, and it is the second half of this file.

Three programs, not one (milestone 290 (`components/src/ntp.rs` three), 2026-09-14). Until then the client, the test server that
answers it and the witness that proves the clock page is unreachable were one binary with three roles
dispatched on `arg0`, packed as `ntp`. calef ruled the split and named all three:
`network_time_client` in `components/`, `network_time_test_server` and `unwritable_clock_witness` in
`fixtures/`, because only the first is a program a distribution would ship. Where this note says
"role", read "program"; the sections below are written in the present tense and say which is which.

Milestone 51's other lanes own the RTC drivers and the clock service, and `date` plus the calendar
crate. The authority argument in full, that reading the clock is harmless and setting it is a real
capability, belongs with the service and is recorded in notes/clock.md. Proofs, era arithmetic and
client tests are in [`ntp/`](ntp/README.md).

## Why the protocol is a crate and not part of a component

The same reason `filesystem_contract` and `graphics_contract` are crates. A wire format is arithmetic and byte layout,
cheap to get wrong and expensive to debug from inside a QEMU boot against a live server. Here it is 21 host tests and 7 Kani harnesses, all done in
under a second with no emulator.

It also puts the boundary in the right place. The client holds a network capability and a
capability to *propose* a time, not the clock. Keeping the
protocol in a crate with no I/O means that client cannot accidentally grow the ability to set
anything, because the code that knows what a time is has nothing to set.

## Scope: unauthenticated NTPv4, and what that is worth

The crate implements plain NTPv4. It implements neither NTS (RFC 8915) nor the RFC 5905
symmetric-key MAC. That is a decision, and this is its accounting.

What it buys: correct time from a reachable server on a path where nobody is injecting packets,
which is the ordinary case and is why plain NTP is what most machines still run.

What it does not buy: anything against an attacker who can see the request and beat the real
server's reply back. Everything between this crate and that attacker is the check list below plus an
unpredictable transmit timestamp. An off-path attacker, who cannot see the request, has to guess
a 64-bit nonce. An on-path attacker reads it and passes every check we make. Plain NTP has no
answer to that, and the crate's own documentation says so.

The consequence for the system is that an NTP-derived time is untrusted input, and milestone 51's
design already treats it that way. The client proposes and the service applies bounds. A compromised
client can lie only inside those bounds.

### Why NTS is a separate decision and not a stretch goal

NTS-KE is TLS 1.3. TLS needs certificate validation. Certificate validation needs a roughly correct
clock, which is the thing being obtained. The standard escape is a build-time "not before" timestamp
plus whatever the RTC says. That is a real design choice, not a detail to settle mid-implementation: a machine whose image is older than its certificates fails to boot into a
usable state. The roadmap records it as a fork. What the crate deliberately does not do is
half of it: an extension-field parser with no cryptography behind it would put the letters NTS in the
tree while authenticating nothing, which is worse than the honest absence.

## The 2036 problem, and the pivot we chose

An NTP timestamp is 64 bits: 32 of seconds since 1 January 1900, 32 of binary fraction. The epoch is
2,208,988,800 seconds before Unix's, and the seconds field wraps on 7 February 2036 at 06:28:16 UTC.
The field alone cannot say which 136 years it means.

We take RFC 5905 §6 (SMP)'s convention as a fixed pivot:

| seconds field | era | covers |
|---|---|---|
| high bit set (≥ 2^31) | era 0, counted from 1900 | 1968-01-20 to 2036-02-07 |
| high bit clear | era 1, counted from the 2036 rollover | 2036-02-07 to 2104-02-26 |

The alternative, picking the era nearest the time you already believe, makes decoding depend on the
clock. That makes it unprovable, and it is wrong exactly when this machine needs it, at a boot that
believes it is 1970. So the pivot is a pure function of its input, provable, and wrong only after
2104. The crate therefore has a documented expiry date, better than every implementation's undocumented
one. `Timestamp::from_unix` refuses anything outside the window rather than
wrapping. Offsets and delays are modular (`wrapping_sub`, read back as signed), so an exchange
straddling 2036 comes out as three seconds. [`the-era-pivot.md`](ntp/the-era-pivot.md) has the
epoch arithmetic, the three reasons against a nearest-era rule, and the exact window.

## The checks, which are the security

`Query::accept` is the only function in the crate that can reject anything. `Packet::parse` is total
on 48 bytes: it decodes and judges nothing. That split is deliberate, and it is what lets the
parse/serialise round trip be proved over arbitrary bytes while every judgment stays readable in one
place.

In order:

1. Exactly 48 bytes. Longer means extension fields or a MAC, and since we implement neither,
   accepting one would mean silently ignoring authentication data a server computed. Fail closed.
2. Version 4, mode 4 (server). Mode is what keeps an unsolicited broadcast out: nobody asked for
   it, so nobody should believe it.
3. The origin timestamp equals the nonce we sent. The load-bearing one. Checked before anything
   in the packet is believed, because it is the check that says the packet is a reply to *us*. It
   rejects a stale reply to an earlier request.
4. Stratum 0 is a kiss-o'-death, reported as itself rather than as a generic failure: `RATE`
   means back off and `DENY` means go away, and a client that retries on those is the abusive client
   the packet exists to stop. Stratum above 15 is not a time source. A leap indicator of 3 is the
   server saying the same thing.
5. The transmit timestamp is not zero, which on the wire means "I do not know what time it is".
6. The four timestamps agree with causality: the server did not answer before it was asked, we
   did not receive before we sent, and the round trip is not shorter than the server's own
   turnaround. Three distinct rejections, because the third is possible while both halves of the
   first two are fine.
7. The claimed root distance is inside 16 seconds (RFC 5905's `MAXDISP`), which is deliberately
   far looser than the RFC's 1 s selection threshold. The split is whose job it is: the crate
   rejects the impossible, the service applies policy. A server 200 ms away over a bad path is a
   poor sample, and what to do with a poor sample is a decision made with the other samples in view.

### The nonce, and the free hardening

In plain NTP the client's transmit timestamp is echoed back in the origin field, so it is the only
thing an off-path attacker has to guess. RFC 5905 says to randomize its low-order bits, and how many
bits are random is set by the clock's precision: a microsecond-resolution clock leaves about
12, which is 4096 guesses. `Timestamp::randomise_low` does that and takes the bit count as a
parameter, because the caller knows its precision and the crate does not.

`Query::with_nonce` does better, and the reason it is free is worth stating: a server never
interprets the client's transmit timestamp. It copies it into the origin field and does nothing
else. So the value on the wire need not be a time at all. Send 64 random bits, keep the true send
time locally for the arithmetic, and the attacker's guess goes from a dozen bits to sixty-four. This
is what chrony does by default and it costs one extra field.

## What is proved, and one place where a solver was the wrong tool

Seven Kani harnesses, run by `script/verify`, cover the era pivot over every Unix second from 1970
to 2104 and total decoding of any wire timestamp. Three more quantify over all 2^384 48-byte
packets: the parse round trip, the nonce check, and `accept` being total and coherent. Those three are what a
model checker exists for.

The nanosecond round trip is the exception. Kani needs about four times the work per bit of operand
on two composed multiplications, and could not finish the full range. Running the
code on all 10^9 values takes 0.6 s, so `every_nanosecond_survives_the_round_trip` is exhaustive,
which is a complete verification, and a bounded harness guards the low corner. The rule it is an
instance of: a model checker is the tool for domains too big to enumerate, not a better tool for
domains that are not. Check first whether you can try all of them.
[`what-is-proved.md`](ntp/what-is-proved.md) has the harness table and the solver timings.

## What is not in the crate

- No socket, no `net_stack` client, no retry scheduling, no server selection or clock filtering.
  Those belong to the component that carries these bytes, which is the rest of this file.
- No leap-second handling beyond passing the indicator through. A pending leap second is not a
  rejection: the server's clock is fine, the day is not 86400 seconds long, and interpreting that is
  the calendar's problem.
- No `Duration`/`SystemTime` conversions. The crate is `no_std` and deals in `(u64, u32)`, so the
  clock service can hold whatever type it likes.

# The client (milestone 51 lane D)

`components/src/network_time_client.rs`. Five capability slots, and the interesting one is the
slot that is missing.

```text
  entropy ──an endpoint──►┌──────────────┐──an endpoint──► net_stack ──► the network
  (8 random bytes)        │  ntp client  │ (the socket contract, UDP 123)
                          └──────┬───────┘
                                 │ an endpoint: PROPOSE
                         ┌───────▼────────┐
                         │ clock service  │──writes──► the clock page
                         └────────────────┘   (the client holds NO mapping of it,
                                               read-only or otherwise)
```

| slot | what it is | what it lets the client do |
|---|---|---|
| 0 | a report endpoint (WRITE) | say what happened |
| 1 | the socket contract's endpoint (WRITE) | send and receive datagrams |
| 2 | an untyped budget | mint and map one shared frame |
| 3 | the clock service's **propose** endpoint (WRITE) | **ask** for a time |
| 4 | the entropy service's endpoint (WRITE) | obtain eight random bytes |

There is no clock page here in either direction, so "set the time" is not an operation this process
can express. A compromised NTP client can lie inside the service's bounds and can do nothing
else. It cannot set the clock to 2038 or move it backwards past a second, and it holds no authority
over anything but the socket it was given. Unix cannot make that claim, because `ntpd`
runs as root and `settimeofday` takes any value it is handed.

The claim is proved the way the machine proves things rather than argued.
`an_ntp_client_holds_no_writable_clock_page` gives a witness the same five slots, plus the exact
address at which a process holding the *set* authority maps the clock page
(`clock_service::CLOCK_VA`). It watches the witness write there and die of a fault, with the page's
generation unchanged on both sides of the attempt. The address is the one that would matter, so the
test cannot degenerate into poking at nothing; the boundary is the mapping, and knowing where to look
buys nothing.

## Reading the time without a mapping

The client needs T1 and T4 in the same timescale as the server's T2 and T3, and it holds no clock
page. So it uses `propose::STATE`, which the contract crate put there for exactly this caller: "a
proposer is exactly the process that may hold the endpoint and no mapping". One round trip, once, to
anchor `wall0` against the monotonic counter; everything after that is counter arithmetic, which is
ambient and costs one instruction.

The bootstrap case falls out rather than needing a branch. When the clock is `UNKNOWN` the service
answers 0, T1 and T4 are then measured from 1970, the offset comes out as the whole distance to the
server's clock, and the proposal lands on the server's time. That is the machine with no RTC, and it
is the case NTP exists for.

## The nonce comes from the entropy service, and refuses to come from anywhere else

`Query::with_nonce`'s 64 bits (above) are worth exactly as much as they are unguessable. Until
2026-07-30 the only source in this tree was splitmix64 seeded off the virtual counter, which
notes/entropy.md calls predictable to anyone who can guess boot-relative time. The entropy service
(DECISIONS §44) landed that day, so the nonce is one eight-byte draw from it, one per attempt.

A client with no entropy capability stops. It reports `RPT_NO_ENTROPY` and exits *before it
touches the network at all*: no socket, no frame, no datagram. Falling back to the weak stream would
be §42 (a filesystem must be truthful)'s silent degradation, in the one place where the entire value
of the number is that nobody can predict it. It is the same call `SystemRng` makes when it panics
rather than degrading. The two
failures stay distinguishable with no probe, because `entropy_contract::delivered` reads a byte count of
`0..=8` and every kernel `CALL` error is one of the small negatives: `NoSuchSlot` means "there is no
entropy service", 0 means "the service has none".

The test asserts what did *not* happen, which is the harder half: the server's report endpoint has no
waiting sender, so no request was ever built.

## One shot, and why that is now a choice

When the client was built (2026-07-31) the syscall surface was `EXIT`, `YIELD`, `INVOKE` and
`CAP_DELETE`, with no sleep, no timeout and no deadline anywhere in it. That was the milestone 51
block's open fork. A poll interval was a yield-spin, a thread that stays runnable for the whole
interval, and at NTP's ordinary 64-second poll that is not a service anybody should ship.

So this is a one-shot synchronizer: up to three requests a couple of milliseconds apart, one
proposal, exit. It was the honest shape available then, and adding a sleep syscall to escape it
would have settled the timed-wait fork by accident.

*Corrected 2026-10-11 (UTC), verified against `components/src/network_time_client.rs`:* milestone
106 (a wait that ends on either the interrupt or the deadline) built the `Timer` object, and
`poll_gap` sleeps on one between attempts. A long-running client is no longer blocked on a kernel
primitive. It is a design question of its own (who supervises it, and what it does between polls),
recorded in `design/roadmap/0674-the-last-yield-spinners-sleep-on-a-timer.md`.

A kiss-o'-death is not retried, and this is a property of the client rather than of the crate. A
rejected reply *is* retried, because it may have been a spoof that beat the real server back.
Stratum 0 is an instruction (`RATE` means back off, `DENY` means go away), and retrying into one is
the abusive behavior the packet exists to stop. The test counts requests: three for a bad origin,
one for a kiss.

## The tests, and what they do and do not prove

The client's whole network authority is one endpoint capability, so the tests substitute the peer
at that boundary. `fixtures/src/network_time_test_server.rs` speaks the socket contract on the other
end of the client's slot 1 and is an NTP server behind it. There is no test-only branch anywhere in
the client. `fixtures/src/unwritable_clock_witness.rs` gets the client's five slots and faults
writing the clock page, and `spawn_with_client_endowment` endows both from one `grants` list, so the
two cannot drift. Six kernel tests (`system_tests/src/user/ntp_tests.rs`) run the same assertions on
aarch64 and riscv64, and each was watched fail under a deliberate mutation first.

They do not prove that smoltcp and the NIC carry the bytes (milestone 30 (the network stack as a
confined component) does), or anything about a real time server. [`the-client-tests.md`](ntp/the-client-tests.md) has what is proved and
not, the test table, the seven mutations, and the false "same binary" argument that kept the witness inside
the client's binary until milestone 290 (`components/src/ntp.rs` is three programs wearing one
name). It also has why `script/test --test an_ntp_exchange` once hung on its own: a discarded
entropy-service wiring, a trap milestone 402 (a service report nobody is obliged to drain) prices.

Milestone 402, `design/roadmap/0402-a-service-report-nobody-is-obliged-to-drain.md`, prices it.

## What the client does not do

- No server selection, no clock filter, no combining. RFC 5905's selection and clustering
  algorithms exist because a real client polls several servers and weighs their samples. One server,
  one sample, one proposal is what a one-shot synchronizer can honestly do, and the machinery for
  more wants a long-running client, which is now a design question rather than a missing primitive.
- No slewing. Nothing here gradually corrects; a proposal is a step the service bounds. Slewing
  is a policy the *service* could adopt, and DECISIONS §43 records why it is a choice rather than a
  correctness requirement: `Instant` is never in the write path, so a step cannot perturb monotonic
  time.
- No NTS and no MAC, as the crate half of this file records at length.
- A boot whose clock is `UNKNOWN` runs the client once, from the progenitor, announced like the
  resolver (calef, 2026-10-25 UTC;
  [the proposal](../design/roadmap/proposals/radons-booted-system-runs-jig.md)). Until that is
  built, nothing in the interactive boot runs it; the tests wire it.
