---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 187. One crate per kernel-test pair, or one crate for all of them?

Ruled 2026-10-28 (UTC): Option 1, one crate per kernel-test pair (e.g. login_test_contract),
following swap_protocol's precedent, keeping each contract readable in one file; nineteen names
handed to script/names as a worklist rather than decided in one sitting. Unblocks milestone 407.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 407 (one definition of the numbers a kernel test and its program agree on)'s
`DECISION` gate naming no section. Measured by milestone 293's lane, which reduced one instance of
this and then counted the rest. *(Section number provisional until the merge queue lands it.)*

## What is being decided

AGENTS.md rule 7 says anything two binaries must agree on is a crate, never a copy. Seventy-six
constants across nineteen file pairs are copies, each held in line by a `// must match` comment:
role numbers, report tags and report flag bits shared between a kernel test's wiring and the program
it spawns.

The sweep is mechanical and a lane could start it today. **What it cannot settle is the crate names,
and there is no useful half of this work that does not create one.** Two shapes:

1. One crate per pair (`login_test_contract`, `compositor_test_contract`, ...), which follows
   `swap_protocol`'s precedent, keeps each contract readable in one file, and costs nineteen names.
2. One crate for all of them, which costs one name and makes the crate a grab bag of unrelated
   numbers.

## Is the premise true

Checked 2026-09-19 in this worktree. Yes, and the tree's own counter-example is still in it:
`crates/hot_swap_contract/src/lib.rs:381` publishes `pub const ROLE_DIRECT: u64 = 0` with real doc
comments, and `kernel/src/user/live_swap_tests.rs:52` re-declares it locally anyway under a
four-word comment, alongside `ROLE_QUEUED` and `ROLE_HUNG`.

That is the strongest argument for a gate rather than a convention: even where the crate exists,
one side was written without it.

Milestone 407's block records an independent re-count on 2026-09-19 over `ROLE_*`, `RPT_*`, `F_*`
and `TERM_MAGIC` declared on both sides of a spawn, finding 25 kernel/program file pairs sharing
79 constant names. That is a similar method rather than the same one, so it is not byte-comparable
with the 19 pairs and 76 names, and what it establishes is that the shape has not shrunk.

## Why a comment is not a mechanism

It is rung four of the ladder for a fact that has a rung-one answer sitting next to it. The failure
it permits is silent and specific: a role number changed on one side spawns a *different* role, and
a flag bit changed on one side makes a test assert against a bit nothing sets. Both read as a bug
in the code under test rather than as a mismatch, which is the same misdiagnosis
`credential_proto::fixture`'s own module doc warns about for the SMB account.

## What this tree already does in the analogous case

The shape is built and works three times, all of them in this worktree:

- `crates/hot_swap_contract` publishes the role numbers its two binaries agree on.
- `crates/job_mix` does the same.
- `crates/schedule_store` publishes the file names `session_reviver` and `fs_test_client` both
  depend on, for exactly this reason and with the reason written down.

Milestone 293 did it once more for the credential values three files were copying
(`credential_proto::fixture::PEOPLE`). **So what is missing is not a design, it is the sweep.**

And §63 says where the seam is: logic that needs no capability goes in the crate; anything that
moves or exercises authority stays in the program. A role number is neither logic nor authority, it
is the vocabulary the two ends share, which is rule 7's category rather than §63's.

## Why the existing protocol crates are the wrong home, which is a lookup rather than a preference

These numbers are **not part of any shipped wire contract**. Putting `login_test_client`'s six
behavior numbers into `login_protocol` would widen the protocol a real login client is written
against with something no real client needs. `crates/hot_swap_contract` is not a counter-example: the
swap roles genuinely belong to the swap protocol.

## What each option costs

| | cost | what it gets wrong when it is wrong |
|---|---|---|
| **1, one per pair** | nineteen names, nineteen `Cargo.toml` entries, nineteen dependency edges | a reader meeting `compositor_test_contract` knows what it holds without opening it |
| **2, one crate** | one name | a grab bag of unrelated numbers, which §46 and naming.md's generic-name rule both push against; and every kernel test depends on every other test's constants |

Three crate names in milestone 407's table have moved and the work has to read the current ones.
`swap_proto` is `crates/hot_swap_contract`, `login_proto` is `crates/login_contract`, and
`components/src/ntp.rs` was split into three programs by milestone 290, so that row's program side
is now `network_time_client` and its siblings.

## Recommendation

1, and the reason is not symmetry. Option 2's real cost is the dependency edge rather than the
name. One crate for all of them makes every kernel test's wiring depend on every other test's
constants. So a number added for the compositor rebuilds the login test, and a reader opening the
crate cannot tell which half is theirs. Nineteen small crates is more entries in a manifest and
fewer things to hold in a head, which is this tree's definition of elegance.

Names: none proposed, and `*_test_contract` above is illustrative rather than a suggestion.
Nineteen names at once is the largest naming batch this tree would have taken, which is a reason to
answer question 1 first and then hand the batch to `script/names` as a worklist rather than deciding
it in one sitting.

## Would we still choose 1 if both cost the same

Yes. Option 2 is the cheaper one to build (one crate, one name) and it is still the one this section
refuses, on the dependency edge rather than on tidiness.

## How reversible, and who has acted on it

**The structure is reversible; the names are not.** Merging nineteen crates into one, or splitting
one into nineteen, is a mechanical afternoon. Nineteen names land in nineteen `Cargo.toml` files, in
`script/names`' worklist and in a reader's head, and they cannot be un-taught.

## What is blocked until this is answered

Milestone 407, entirely. The sweep cannot produce a useful half without creating a crate, which
is why the block is gated rather than ready.
