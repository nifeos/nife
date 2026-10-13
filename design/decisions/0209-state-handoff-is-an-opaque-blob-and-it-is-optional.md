---
status: DECIDED
raised: 2026-09-23
decided: 2026-09-23
ratified_by: calef
---

# 209. State handoff is an opaque blob over a granted frame, and it is optional

Ruled by calef on 2026-09-23, on the maintainer's recommendation, reopening
§116 (live component state handoff is declined, for want of a customer).

## The ruling, in three parts

1. The transport is the sketch §116 (live component state handoff is declined, for want of a customer) already left. State moves as an opaque blob over a
   granted shared `Frame`. Capabilities move by `GRANT`. The kernel does not know what the
   bytes mean and neither does the supervisor; the shape of the state is the component's own
   business, exactly as §116 argued when it refused to design one wire format for fifty different
   kinds of live state.

2. Handoff is optional, declared in the component manifest beside `depends_on`. A component that
   declares nothing is kill-and-replaced, which is what the console already is under §41 (the endpoint is the broker, and a device is revoked by taking it back):
   the endpoint is the stable name, and a device is revoked by taking it back. Nothing about the
   existing swap changes for a component that has no state to move.

3. **Failure does not commit.** If the incoming instance cannot absorb the blob, the swap does not
   complete. Under §208 (installing a package is granting it) an activation is a grant, so not completing means revoking the new grant, and
   revoking a grant is §16 (object revocation)'s machinery pointed at one more kind of object. There is no new
   rollback mechanism here and there should not be one.

## The finding that matters more than the ruling

§116 (live component state handoff is declined, for want of a customer)'s premise stopped being true one day after it was written, and nobody noticed for a
month.

§116 declined on 2026-08-23, on the ground that *"no component with meaningful live state is
built or being built"*. The filesystem server's first commit is dated 2026-08-24:

    $ git log --diff-filter=A --format=%ad --date=short -1 -- redoxfs_server
    2026-08-24

And `notes/fs-server.md` records that milestone 23 (a capability-routed component OS with live replacement)'s *"hardest state-handoff case"* is a filesystem
server with open handles. So the component §116 said did not exist landed the next day, and the
record that turned on its absence sat unchallenged until a package ruling happened to walk past it.

**So calef's package ruling is the second customer, not the first.** Saying that plainly is the
point. A decision whose premise is a fact about the tree has a shelf life, and this one expired in
twenty-four hours while reading as current for a month. That is the same shape as the retraction
that reached thirteen records before anyone swept it: not a thing anybody got wrong, a thing that
stopped being right.

## Why optional rather than mandatory

`components/src/` holds 52 components. Mandatory handoff makes all fifty-two answer a question
that roughly fifty of them do not have. An audit sink, a clock, a block roster and a compositor are
either stateless or hold state that is cheaper to rebuild than to move, and a manifest field they
must fill in with "nothing" is a field that teaches nobody anything.

§92 (a caretaker is supervised by the client it serves)'s test, answered out loud: optional is also the cheaper option, and that is effort. It is
less to build, less to migrate, and less to keep working. Saying so is the rule, so it is said.

The non-effort case is the one that decides it, and it is a result rather than an argument.
Erlang/OTP has run this experiment for thirty years. `code_change` is an optional callback: a module
that does not export it is replaced outright, and the overwhelming majority never export it. Thirty
years of production hot code loading across telecom switches did not turn that default into a
mistake. A mechanism that most modules never need, offered to the few that do, is what the longest
running live-upgrade system in the field converged on. We would choose it at equal cost.

## What the tree already does, so the transport reads as idiom

The granted shared `Frame` is not an invention for this decision. It is how this tree already moves
data between two processes when IPC's word-limited messages will not carry it:

- the clock page, read by every program that wants the time;
- §111 (inert configuration is a read-only page)'s env-config page, a validated read-only page of declared keys;
- `block_roster`, which hands a device inventory across;
- `system_initializer`, which hands a boot-time table across.

Four existing cases, one shape. A fifth that carries a component's own state is the same mechanism
with different bytes in it, which is the strongest thing that can be said for a transport: nobody
has to learn it.

## What remains an architect's

The manifest field's name. Names are an architect's, and this decision does not take one. A lane
implementing this ships `handoff` as a **provisional** name on `component_plan`'s requirements,
beside `depends_on`, and says so in its report. Do not ratify it here. `script/names --unratified`
is where it belongs until an architect rules.

Nothing else in this section is a naming decision: `Frame`, `GRANT` and the manifest itself are
existing ratified names being used rather than coined.

## What this does not decide

The content of any component's blob, which is that component's business and always was. And the
hung case: a component that will not cooperate cannot be asked to serialize, so handoff recovers a
planned swap and not the failure it is most wanted for. §116 (live component state handoff is declined, for want of a customer) did not claim otherwise and neither does
this. Milestone 23 (a capability-routed component OS with live replacement)'s block carries that gap and keeps it.

## What it unblocks

- Milestone 198 (a package manager, and the trivial install that makes a second customer possible)'s rung 3a, the client half: installing a package onto a running system can now
  replace a stateful component rather than only a stateless one, which is what §208 (installing a package is granting it)'s second
  argument was for.
- Milestone 23 (a capability-routed component OS with live replacement)'s last residual, which has been the only thing holding that block short of done since
  the other three parts landed.
