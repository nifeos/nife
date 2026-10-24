# The NTP client's tests: the test server, the witness, and the six kernel tests

*Name: provisional, minted 2026-10-11 (UTC) by lane/seventh-ten-longest-notes, for the tests of the
client half.*

An appendix to [`notes/ntp.md`](../ntp.md). It holds what the client's tests substitute and why
that is honest, and the witness with the false argument once made for it. It also holds the six
kernel tests, the mutations each was watched fail on, and why one exchange test used to hang alone.

## The test server, and what it does and does not prove

The client's whole network authority is one endpoint capability, so the tests substitute the peer
at that boundary. `fixtures/src/network_time_test_server.rs` holds `READ` on the endpoint the client
holds `WRITE` on. It speaks the same socket contract (`crates/socket_contract/src/lib.rs`, the same
file `net_stack` compiles) while being an NTP server on the other side of it. The client cannot
tell, and there is no test-only branch anywhere in the client. That is the shape a capability
system makes available, and it is why this is the honest choice rather than a compromise.

The substitution never depended on the two sharing a binary. Milestone 290 (`components/src/ntp.rs`
is three programs wearing one name) settled that by splitting them. What the client talks to is
decided by which endpoint is in slot 1, and the peer's ELF has nothing to do with it. Before the
split this section had a second problem. "No test-only branch anywhere in the client" sat on the
same page as `ROLE_PROBE_CLOCK`, a test-only branch in the client's binary. It is unqualified now.

What it proves:

- the socket-contract glue: minting a frame, delegating it, the destination header, and
  `SENDTO`/`RECEIVE` framing;
- that the 48 bytes are a well-formed NTPv4 client packet addressed to port 123;
- that the nonce is 64 unpredictable bits rather than a clock reading;
- that a reply failing `Query::accept` moves nothing;
- that an accepted sample reaches the clock as a proposal (the page publishes `SYNCED`, which
  nothing but the service can write);
- and that a proposal outside the policy is refused by the service while the client behaves
  perfectly.

What it does not prove, stated plainly because a test server is exactly the kind of thing that gets
overclaimed:

- Not that smoltcp, UDP, IPv4 and the NIC carry the bytes. Milestone 30 (the network stack as a
  confined component)'s socket-contract tests prove that path with a real datagram over the
  confined NIC, and this lane deliberately does not re-prove it.
- Not anything about a real time server. Nothing in QEMU's slirp answers UDP 123: its built-in
  servers are TFTP, DHCP and a DNS NAT, and `guestfwd` is TCP-only. So there is no offline peer to
  point at. Pointing the gate at a public server would make it depend on somebody else's network,
  the way the DNS check already has to be non-gating for exactly that reason.
- Not that the client and the real `net_stack` agree, beyond both compiling `netproto.rs`. The
  stub implements the server side of that contract, and a misreading of it would be a misreading on
  both sides at once. Compiling the same file rather than a copy is what keeps that risk to the
  semantics rather than the layout.

The honest summary: the client is proven against a server we wrote, over a network we wrote, and
the parts we did not write are proven elsewhere.

## The witness, and the argument about it that turned out to be false

`fixtures/src/unwritable_clock_witness.rs` is handed the client's five slots plus the address at
which a process holding the *set* authority maps the clock page. It reports, writes there, and dies
of a translation fault, because its address space has no mapping of that frame at any address. The
boundary is the mapping, not the layout, so knowing where to look buys nothing. That is the claim
Unix cannot make: `ntpd` runs as root, and there is no address in a Unix system its
`settimeofday` cannot reach.

Until milestone 290 the witness was a role of the client's binary. The reason given was that the
proof needed it to be the *same binary*. That reason is false and is worth writing down, because it
was believed for six weeks and it is the sort of argument that survives review by sounding like
rigor. The fault is caused by the capability set. Any process holding that endowment faults at that
address whatever code it runs, and no amount of shared machine code would make a stale capability
list fault.

What keeps the witness honest is that one function endows both.
`kernel/src/user/ntp_service.rs`'s `spawn_with_client_endowment` takes the image as a parameter, so
`start_client` and `start_witness` are the same five `grants` with a different ELF. A sixth slot
given to the client is a sixth slot given to the witness, with nothing to remember and no second
list to update. A separate binary passed through that one function proves exactly what a shared
binary did.

The failure this guards against is a second capability list, not a second binary. Milestone 117
(the stranger test)'s run found `swish` carrying a hand-maintained copy of a fact its manifest
already held. That is the same shape: a copy that decays silently rather than breaking a build. A
future change may need the witness wired differently from the client. If so, the honest move is to
say so in the test's own words, not to grow a second `grants` array beside the first.

## The six kernel tests

Six kernel tests (`system_tests/src/user/ntp_tests.rs`), not arch-gated: three portable binaries,
so aarch64 and riscv64 run literally the same assertions. The suite went from 181 to 187 tests on
aarch64 and 152 to 158 on riscv64.

| test | what would have to be broken for it to pass anyway |
|---|---|
| `an_ntp_exchange_reaches_the_clock_as_a_proposal` | the request is NTPv4, mode 3, port 123; the correction arrives; the page says `SYNCED`, which only the service writes; the clock moved by about what the server claimed |
| `a_reply_that_fails_validation_never_becomes_a_proposal` | a bad origin, a kiss-o'-death and a 20-byte datagram each leave the page's generation untouched, and the kiss is not retried |
| `a_proposal_outside_the_policy_is_refused_by_the_service` | two hours forward and ten seconds back are both refused, by the service, with the client behaving perfectly |
| `an_ntp_client_holds_no_writable_clock_page` | the write faults, at that exact address on aarch64, and the process does not survive it |
| `the_nonce_on_the_wire_is_random_and_is_not_the_clock` | two exchanges differ, and neither transmit field is within an hour of the clock |
| `without_entropy_the_client_refuses_rather_than_guessing` | the refusal names `NoSuchSlot`, and no request was ever sent |

Every one of those assertions was watched fail before it was trusted. The seven mutations:

1. the write removed from the witness;
2. `with_nonce` swapped for the plain form;
3. a proposal made despite a rejection;
4. a fallback inserted where the entropy refusal is;
5. the kiss-o'-death `break` deleted;
6. the offset dropped from the correction;
7. the destination port hard-coded past the wiring.

Seven mutations, seven failures, each naming the right thing.

Two of them found a real defect in the *tests* rather than in the client. It is worth recording
because it is the shape a test-server design invites. With the client mutated to carry on, an
unbounded `ipc_receive` on a report nobody would ever send became a sixty-second watchdog hang
instead of an assertion. Both waits are now bounded, so "the client never got there" fails in two
seconds with a sentence.

## Running one of these on its own used to hang, and why

If you reach for `script/test --arch aarch64 --test an_ntp_exchange`, it works now. Before
milestone 290 it did not. Every exchange test failed in isolation with *"the test server never saw a
request: the client failed before it reached the network"*, while the whole suite was green. The
cause was three steps from the message. That is why it is written down here rather than left to the
next person's afternoon.

`machine_has_no_entropy()` called `entropy_service::ensure` and threw the `Wiring` away. The first
caller of `ensure` is handed the entropy service's `ready` endpoint, and the service announces
itself with a blocking send. So discarding that wiring parks the service inside its own startup,
before its request loop. Every later `ensure` gets `ready: None` and cannot rescue it. The client
then blocks forever in `call(ENTROPY, ...)`, and what the test reports is the *server's* silence.

It never showed in a whole-suite run because `entropy_tests` sorts before `ntp_tests` and drains the
report first: this file was correct only because of another file's name.
`machine_has_no_entropy()` is `entropy().is_none()` now, which asks the same question through the
helper that drains properly.

The trap itself is still there for anyone else calling `ensure`, and the remedy belongs in `ensure`
rather than in its callers. Milestone 402 (a service report nobody is obliged to drain),
`design/roadmap/0402-a-service-report-nobody-is-obliged-to-drain.md`, prices it.
