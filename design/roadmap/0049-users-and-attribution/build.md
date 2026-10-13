# Milestone 49: how each piece was built and proven

This appendix belongs to
[milestone 49 (users, login, and attribution)](../0049-users-and-attribution.md), under
[§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is
complete without it; this is how each piece is built, and its proof.

## The login service

`login` (provisional name) is `components/src/login.rs`, with its wire contract `crates/login_protocol`
(provisional). It holds the credential service's verify endpoint, the file service's root directory
capability, and a construction budget. The verify endpoint is milestone 56 (secrets and entropy)'s,
unmodified: neither `cred_proto` nor `credentialer.rs` changed.

A client presents an identity and a secret over `login_protocol`. On a match, `login` builds a fresh
`fs_subtree_caretaker`. That is the same construction `crates/system_initializer` performs for a
directory-granted spawn. It also splits a fresh budget, then delegates both to the caller over
`abi::endpoint::SEND_CAP`. Two different successful logins are therefore two different endpoint
*objects*, not two views of one shared endpoint. That is the channel-shaped attribution DECISIONS
§109 (attribution is a channel property) decided on, rather than the badge mechanism it refused.

`login` also sends one attribution record per successful login (identity, sequence number) on its
own audit endpoint. That is the "establishes a channel and can say who it belongs to" half of §109's
property, made checkable rather than merely claimed.

Proven end to end by `system_tests/src/user/login_tests.rs` on both ISAs, with `login_test_client.rs`:

- A correct identity and secret produce a directory capability that answers a real `READDIR`. The
  budget retypes a real page. The test checks that the capabilities work, not merely that they
  arrived.
- A wrong secret is refused and nothing follows the refusal. The client checks this by never calling
  `RECV_CAP` on that path, rather than by asserting a negative.
- Two different identities each get an independently working channel. Each is named correctly in
  the audit trail, in the order they were established.
- A fourth test proves the service survives more than eight logins, the ceiling a leaked
  capability-table slot per login would impose (see [bugs-found.md](bugs-found.md)). The test takes
  the shared instance's login count to nine and checks that each one's directory and budget work.

## Per-identity subtrees

Landed 2026-08-23, under DECISIONS §117 (a principal's subtree is named by its identity string). `login` used to
attenuate every principal to the same fixed subtree, `filesystem_protocol::fixture::tree::SUB`, with the same
rights. It now attenuates each authenticated identity to a subtree named by the identity string
itself. The string is used directly, with no lookup table. That matches exactly what milestone 155
(a provisioning tool)'s `identity_provisioner` creates at provisioning time; nothing is auto-vivified
at login.

Proven by `login_scopes_each_identity_to_its_own_provisioned_subtree`. `alice` and `bob` each
write a self-naming marker into the directory `login` delegated. Each confirms that the old shared
fixture subtree's own file is absent from it. That is proof, not assertion, that neither landed
there. `alice` then logs in a second, independent time and reads back `alice`'s own marker, not
`bob`'s. That is the isolation property stated positively.

A companion test, `login_denies_an_authenticated_identity_with_no_provisioned_subtree`, checks the
considered fold for a real credential with no provisioned subtree. It is refused, indistinguishably
from a wrong password. `components/src/login.rs`'s own BUGS gives the reasoning.

A third bound surfaced here and is now recorded rather than left implicit. An identity longer than
sixteen bytes cannot get a per-identity subtree in this slice at all. The grant name travels in two
argument words to the caretaker (`filesystem_protocol::grant::MAX_NAME`). That is narrower than
`login_protocol::MAX_IDENTITY`'s sixty-four. `login` refuses rather than silently truncating.

## The channel-per-client front door

A single `REQUEST`/`RESULT` endpoint pair would share one staging page across every client the
process ever serves, which is a structural "one client at a time" limit. `credentialer.rs`
documents that limit for its own verify page.

The front door accepts exactly one word, `login_protocol::CONNECT`. It mints a fresh, private
request/result pair and staging page per caller, before any identity or secret is staged. That is
`filesystem_proto`'s own "a fresh object per client" answer, copied here. Two callers reaching the
front door together contend only for service order. They can never observe or corrupt each
other's secret. The cost is that a channel, answered or not, is never reclaimed in this slice.

`components/src/login.rs`'s own BUGS has the full design. The proof is
`kernel::user::login_tests::two_clients_connecting_together_get_independent_channels_and_neither_observes_the_others_secret`.

Two resource leaks in this piece were fixed before it landed; [bugs-found.md](bugs-found.md) has
both.

## Logout reclaims a session

Resolved 2026-08-23, on `milestone/49-caretaker-teardown`. A caretaker's construction memory used to
be spent forever, with no logout that gave it back.

`mint()` used to drop its own copy of the caretaker's construction region once the caretaker
confirmed descent. An earlier lane had already landed a narrower fix for that drop's
capability-table slot. `mint()` now delegates the region to the authenticated client instead, as a
fourth capability narrowed to `WRITE`: a "logout ticket". The region has nothing left to `SPLIT` or
`RETYPE`. Its only remaining use is `MemoryRegion::DESTROY`. Calling it reclaims the caretaker's
TCB, address space and endpoints. The pages return to `CONSTRUCTION_UT` under §13 (frame
revocation)'s region ownership.

The client's own budget, the third capability, needed no new mechanism. It was always delegated with
`WRITE`, which is what `DESTROY` needs. So a full logout destroys both, in a specific order. `mint()`
splits the region first and the budget second, from the same `CONSTRUCTION_UT`. `crates/memory_regions`'
LIFO reclaim is already accepted elsewhere in this tree, by DECISIONS §92 (a caretaker is
supervised by the client it serves). It returns a freed child's pages to reusable capacity only
when it is freed at the top of its parent's watermark. A client that destroys them in the wrong
order still tears the caretaker down correctly, but strands the region's pages.

That was caught empirically, not reasoned about. An earlier version of this fix's own test destroyed
them in the wrong order. Every assertion in it passed. It silently starved a later, unrelated test
in the same suite of real login attempts.

The block's earlier text named two candidate shapes. One was a principal's supervision endpoint
reaching `login`; the other was a caretaker `DESTROY`ed by name. They collapsed into the second once
checked against `notes/hung-component.md`'s documented gap for what `Untyped::DESTROY` can and cannot
reclaim while a thread is still live. So this needed neither a new supervision endpoint nor any
overlap with milestone 152 (durable delegation)'s durable-session scope. `components/src/login.rs`'s
own BUGS ("Resolved, 2026-08-23") has the full design.

Proven by `kernel::user::login_tests::caretaker_teardown_reclaims_a_full_session_worth_of_memory`.
It runs ten logins against one shared, tightly budgeted service instance. Each fully logs back out
before the next begins. That could only pass if the memory came home every time.

## The caretaker load is measured

Resolved 2026-08-24. `login` used to load `fs_subtree_caretaker` by name with no check at all. That
was inconsistent with milestone 104 (the measurement continues past init)'s discipline: init refuses to
load a program whose bytes do not match the archive's measurement table.

The investigation asked how a loader outside the boot chain joins that chain. It found the premise
false. `login` maps the identical physical archive the kernel already maps for
`crates/system_initializer`, the same read-only way. So the kernel's boot already vouches for
`login`'s copy exactly as much as it vouches for init's. No new trust decision was needed.

`login`'s `_start` now reads `measured_boot::PROGRAM_MEASUREMENTS` from that same archive. It calls
the same `measured_boot::verify_in_manifest` that `system_initializer::measured` already calls. It
does so once, at startup, before any client exists.

A refusal does not crash the service. That mirrors `system_initializer`'s own rule: "an unvouched
`fs_subtree_caretaker` costs a feature, not a boot". Every login is answered `login_protocol::DENIED`
instead. `components/src/login.rs`'s own BUGS explains why this fold does not get the anti-oracle
reasoning the wrong-password and no-subtree folds get. This check varies with nothing a caller
controls, so there is nothing to probe. The test is
`kernel::user::login_tests::logins_caretaker_measurement_matches_the_real_table_and_a_tampered_one_would_be_refused`.

## The terminal

Resolved 2026-08-27. The roadmap had a recorded recommendation, the deny-cleanly shape, and that is
what got built. This executed the recommendation rather than re-deciding it.

`login` now holds a `WRITE | GRANT` view of the real interactive terminal. `boot` grants it; it is
the same capability the shell already holds `WRITE` on. `login` hands `WRITE` on to the first
successful caller. Every login after that is refused with `login_protocol::NO_TERMINAL`, a dedicated
code not folded into `DENIED`. The refusal comes before the identity or secret is even relayed to
the credential service. It lasts until `login_protocol::LOGOUT` frees the terminal. `LOGOUT` is a bare
word on the front door, since it carries no secret.

`components/src/login.rs`'s "The terminal: single-session, deny cleanly" has the full design. Its
own BUGS names what this slice does not build: an authenticated `LOGOUT`, and a liveness check on an
abandoned holder. Both are named limitations, not oversights, scoped to what today's single-tenant
boot needs.

Real multiplexing is the other shape, undecided and unbuilt on purpose. Choosing the narrow shape
commits to nothing the wider one would later have to unwind. Proven end to end by
`kernel::user::login_tests::login_hands_out_the_terminal_once_and_denies_a_concurrent_second_login_until_logout`
on both ISAs.

## Wiring the interactive boot

Resolved 2026-08-27. `credentialer`, `identity_provisioner`, `login` and a new `audit_sink` are now
built by `crates/system_initializer::boot` on both ISAs. `audit_sink` is a provisional name. It
drains `login`'s own `AUDIT` endpoint so its blocking send never parks the service. Its doc is now
`components/src/login_audit_receiver.rs`, renamed 2026-09-24. All four draw on the real
virtio-rng-backed entropy service that §120's amendment had already unblocked.

The block's three-item plan was executed in order.

1. `credentialer` and `login` are wired into `boot` through `build_child`. They hold narrowed views
   of capabilities `boot` already has: the file service pair and a fresh construction budget apiece.
   They also get a client view of the entropy service `boot` built first. Where they sit in the
   build order mattered; [bugs-found.md](bugs-found.md#the-build-order-and-a-full-capability-table)
   has why.
2. A real subtree and a real credential for whoever logs in, through `identity_provisioner`
   (milestone 155). It runs once per boot against the generated password below.
   `design/roadmap/0155-*` was updated to match.
3. The demo credential's password is generated, not baked in, executing the block's own
   recommendation. `boot` draws twelve bytes from the entropy service it just built and hex-encodes
   them. It provisions the demo identity `operator` with the result through `identity_provisioner`.
   It prints the password once, before the prompt. No secret is permanent; a later boot can do
   something else.

Milestone 159, a real hardware entropy source (the JH7110's TRNG, minted alongside §120), is
unaffected by any of this. §120's own "what this does not decide" already said so.
