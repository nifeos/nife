---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
---
# 293. A role that differs only in its credentials is an argument

calef ruled milestones 290, 291 and 292 into separate programs on *"small programs with specific
functions."* He ruled this one the other way (2026-09-14): the answer here is not eleven programs.

`fixtures/src/login_test_client.rs` had eleven roles, and its first act on entry was a lookup from
role number to a pair of byte strings. Several of the eleven ran the same code and differed only in
that pair. A role that differs only in its credentials is not a role; it is an argument.

## The eleven, classified

Classified by reading what each role's code does, not by the shape of its name. The question is: if
the identity and the secret were handed in, would anything about this role's *code* remain?

| Role | Credential it presented | What its code did that no other role's did | Verdict |
|---|---|---|---|
| `ROLE_ALICE` | alice, correct | nothing; falls through the match's `_` arm | **credential** |
| `ROLE_BOB` | bob, correct | nothing; byte-identical to `ROLE_ALICE` | **credential** |
| `ROLE_WRONG_SECRET` | alice, `not-the-password` | nothing; refused at the shared `verdict != OK` exit | **credential** |
| `ROLE_NO_SUBTREE` | carol, correct | nothing; refused at the same exit, for a different reason | **credential** |
| `ROLE_TERM_SECOND` | bob, correct | nothing; refused at the same exit with `NO_TERMINAL` | **credential** |
| `ROLE_ALICE_MARK` | alice, correct | `write_marker(dir, b"alice")` and an `absent` check | **behavior, and half of it was a credential** |
| `ROLE_BOB_MARK` | bob, correct | `write_marker(dir, b"bob")` and the same `absent` check | **the same behavior** |
| `ROLE_ALICE_CHECK` | alice, correct | `read_marker(dir)` into the report's third word | **behavior** |
| `ROLE_LOGOUT` | alice, correct | destroys budget then region, re-`READDIR`s, reports the wait | **behavior** |
| `ROLE_TERM_FIRST` | alice, correct | sends `TERM_MAGIC` on the terminal, then tears down without the wait report | **behavior** |
| `ROLE_TERM_LOGOUT` | none at all | sends `logout_word` on the front door, never calls `CONNECT` | **behavior** |

Five of eleven were pure credential. Two more were one behavior written twice, because the only
thing separating them was the identity byte string written into a marker file, which *is* the
identity. Eleven roles were six behaviors and four credentials.

**`ROLE_TERM_SECOND` is the one worth pausing on**, because its name argues hardest that it is a
behavior and its code is `ROLE_BOB` exactly. What made it feel like a role is a fact about
*when the test runs it* (while `ROLE_TERM_FIRST`'s terminal loan is outstanding), and a precondition
the caller arranges is not a property of the callee. The kernel test still arranges it; the client
no longer has a name for it.

## Credentials become arguments, and how a program receives one

A run is now `_start(behaviour, identity, secret)`.

**The central problem was that an identity and a secret are byte strings and `START` carries
integer registers.** What a spawned process is handed is `kernel::user::Spawn`: three `u64`s, a list
of capabilities, and a list of physical mappings. That is the whole of it. The mechanisms that exist
were checked rather than recalled, and none of them carries a string:

- `grant_plan::ArgSpec` is the shell-side declaration, and it carries exactly one *integer*
  ("`least_authority_demo 9`'s `9`"); the positional arity it would need is deferred work its own
  doc names. `FileSpec` and `DirSpec` do designate by name, but they resolve the name to a
  *capability* before the program starts, which is the opposite of handing over a string.
- `environment_proto` is a page of inert configuration, and it is validated against curated
  domains precisely so that a secret cannot ride on it: its own module doc is the refusal, in
  those words. Disqualified by design rather than by preference.
- `Spawn::maps` can place a page at a chosen VA, and the login service's own wiring already uses
  a kernel-side `map_blob` to hand `login` two program images this way. It would work. It costs a
  frame the spawner allocates, fills and maps, plus a parse on the far side, for two short strings
  that are already compiled into both sides of the tree.

So the bytes live in a crate both sides already link and the register names which one:
`credential_proto::fixture` grows the roster of three people the tree's credential fixtures
authenticate, and `identity` and `secret` are indices into it. `credential_proto` was already a
dependency of both `kernel` and `fixtures`, so this is not a dependency decision.

The composition is the point. The two indices are separate arguments, so the wrong-secret run is
`(ALICE, WRONG)`: the same behavior as the honest `(ALICE, ALICE)` with one input changed.

That is what the file was already claiming and could not enforce. Its own argument for being one
binary reads *"a program that shares the honest path with an attempted-wrong-secret run is a fairer
test of a refusal than a different program failing for its own reasons."* A dispatch table delivers
that by convention: the honest arm and the refusal arm are two pieces of code that happen to
agree today. An argument delivers it by construction: there is one arm.

`fixture::WRONG` is `PEOPLE.len()`, derived rather than written as `3`, so growing the roster cannot
turn the wrong-secret index into a fourth person's real secret. `fixture::NONE` is neither an
identity nor a secret, so `FREE_TERMINAL` says out loud that it authenticates nothing, and a
behavior handed it that tries to authenticate anyway reports `MALFORMED` rather than quietly
logging in as whoever sits at index zero.

## The behaviors stayed in one binary, and the reason is not effort

Six behaviors remain: `LOGIN`, `WRITE_MARKER`, `READ_MARKER`, `LOGOUT`, `HOLD_TERMINAL`,
`FREE_TERMINAL`. calef did not pre-decide whether these should be six programs under 290/291/292's
principle or an argument. Both were priced.

**Six programs** costs six `[[bin]]` entries, six archive entries, and six copies of the
connect-and-authenticate preamble: send `CONNECT`, receive `CONNECTED`, receive three capabilities
in order, map the delegated page from a scratch region, `place` the credential, send `LOGIN`,
receive the verdict, then receive five more capabilities in `login_proto`'s fixed order. Avoiding
six copies means lifting that preamble into a crate, which is a new crate and a new name.

**One binary** costs one `match` after the preamble, at the point where the behaviors genuinely
diverge.

Would we still choose one binary if both cost the same? Yes, and the reason is the fixed order
in that last sentence. The preamble is not shared setup, it is the contract under test: five
delegated capabilities arriving in one order on one channel. Six copies of a wire order is six
places for it to drift, and a drifted copy fails as a mysterious `RECV_CAP` on the wrong object
rather than as a diff. The sibling lane splitting `fixtures/src/hello.rs` (291, unmerged as this is
written, which is why it is not cited by number) found the opposite, and it is worth stating as
evidence rather than as analogy: `hello`'s roles are a catalog (virtio, IPC call,
revocation, page frames, address-space building, four `init` variants) that share the file and
nothing else. These six share an authenticated session.

The same argument the file makes for refusals therefore extends inside the session: a teardown proof
that reaches teardown down the honest login path is a fairer test than one that gets there its own
way.

## `credentialer_test_client`: the premise was false, and checking it was the work

Milestone 293 was minted expecting `fixtures/src/credentialer_test_client.rs` to have "three roles
and the same shape". It does not, and this is recorded rather than quietly skipped.

Its three roles are `ROLE_PROVISIONER` (fills the store through a **provision** endpoint in slot 0),
`ROLE_HONEST` (asks four verify questions) and `ROLE_ATTACKER` (sends five things the contract does
not offer, then asks whether any of it installed a credential). There is no role-to-credential
lookup here at all: the credentials in this file are fixture *data* a behavior uses, not a
selector that picks one. All three are behaviors, and nothing about them dissolves into an argument.

What it did hold is the definition the other two files were copying. `PEOPLE` lived here, and:

- `login_test_client.rs` re-typed all three pairs inside its `credentials()` lookup;
- `kernel/src/user/identity_provisioning_tests.rs` re-typed two of them, under a comment saying it
  had **chosen** `alice`/`correct horse battery staple` to match this file, which is a coupling with
  nothing holding it: the next person to edit either copy had no way to know the other existed.

That is `credential_proto::fixture`'s own opening argument happening three times over ("several
programs have to agree on an account down to the byte, and a second copy of it somewhere would drift
silently into a wrong answer that looks like a bug in the code under test"). The roster moved there,
paired rather than parallel so an identity cannot drift away from its own secret, and the three
call sites now read it. `identity_provisioning_tests`' second spelling of the same name as a `&str`
is now a `const` conversion of the first rather than a second literal.

Every other `const ROLE_` in the tree was checked (`fixtures/`, `components/`, `crates/`,
`kernel/`: 24 files). Every one of them dispatches a role to a *function*. `login_test_client`'s
`credentials(role)` was the only role-to-data lookup in the repository; the nearest thing to a
second is `kernel/src/soak.rs`'s `role_letter`, which maps a role to the character a census prints
for it, and that is a property of the role rather than an input to it.

## What this does not claim

It does not give this system a way to pass a secret to a program. An index into a compiled-in roster
is enough for a fixture and is nothing else; a real program needs a capability to something that
holds the secret, which is DECISIONS §41's answer and is already what `login` itself uses. Both
limitations, and the fact that a spawn argument is visible wherever a spawn is recorded, are in
`login_test_client.rs`'s own `BUGS` section, where a reader meets the mechanism.

## Follow-on

- **Milestone 407.** Numbered on 2026-09-19 by milestone 433's drain of the pile.
  This milestone was briefed to grep the values rather than the constant names, and the grep found
  the other half of its own file's problem: `kernel/src/user/login_service.rs` and
  `fixtures/src/login_test_client.rs` declare fifteen of the same constants twice, held in line
  by a `// must match` comment, and the tree holds seventy-six such constants across nineteen file
  pairs. AGENTS.md rule 7 already forbids it. Not fixed here because the fix needs a crate, a crate
  needs a name, and doing one pair inside the test-wiring hotspot three other lanes were in would be
  a partial fix in the worst available place. `crates/swap_proto` publishes three role numbers that
  `kernel/src/user/live_swap_tests.rs` re-declares locally anyway, which is why the proposal asks
  for a gate and not a convention.
- **Recorded.** `fixtures/src/login_test_client.rs`'s `BUGS` section, on the two things this
  milestone does not claim: that an index into a compiled-in roster is not a way to pass a secret to
  a program (a real one needs a capability to something that holds it, DECISIONS §41), and that a
  spawn argument is visible wherever a spawn is recorded, so nothing here is a design for carrying
  real secrets.
- **Refused.** Splitting the six behaviors into six programs, which is what 290, 291 and 292 did
  with their files. The reason is in "The behaviors stayed in one binary" above and it survives the
  equal-cost test: `hello`'s thirty-one roles share a file and nothing else, while these six share
  an authenticated-session preamble that is itself the contract under test.
The behavior constants (`LOGIN`, `WRITE_MARKER`, `READ_MARKER`, `LOGOUT`, `HOLD_TERMINAL`,
`FREE_TERMINAL`) and the roster's new public names (`PEOPLE`, `ALICE`, `BOB`, `CAROL`,
`NOBODYS_SECRET`, `WRONG`, `NONE`, `identity`, `secret`) are **provisional** and go to calef with
the merge, per AGENTS.md. That is a naming backlog rather than follow-on work, and it never blocks,
which is milestone 115's own rule.

## Index row

calef ruled this apart from 290/291/292, which he ruled into separate programs (2026-09-14): the answer here is not eleven programs. `fixtures/src/login_test_client.rs` had eleven roles and its first act was a lookup from role number to a pair of byte strings. Classified by code rather than by name: five of the eleven were pure credential, byte-identical runs differing only in that pair (`ROLE_TERM_SECOND` included, whose distinguishing fact was a precondition the caller arranges), and two more were one behavior written twice because the only thing separating them was the identity written into a marker file. `_start` now takes `(behaviour, identity, secret)`, so the wrong-secret run is `LOGIN` with a different secret rather than an arm that could drift from the honest one. Credentials travel as `credential_proto::fixture` indices because nothing here hands a `no_std` program a string it was not compiled against, recorded in the file's BUGS. Six behaviors stayed in one binary and the equal-cost test says so: the preamble they share is the contract under test. 293's premise about `credentialer_test_client` was **false** and checking it was the work; what it did hold was the `PEOPLE` definition two other files hand-copied, one saying it had *chosen* to match. All 24 `const ROLE_` files swept: the only role-to-data lookup in the tree. Behavior names provisional.
