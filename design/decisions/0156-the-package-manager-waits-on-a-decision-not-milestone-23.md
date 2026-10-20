---
status: DECIDED
raised: 2026-09-19
decided: 2026-09-19
ratified_by: calef
---

# 156. What the package manager waits on: a decision, not milestone 23 and not the repository split

calef, 2026-09-19 (16:32 UTC), in conversation with the maintainer: *"Yes, drop
MILESTONE 23 from the gate"*, as recommended below. Milestone 198's gate now reads `DECISION` alone.
The text below is the proposal as he ruled on it.

What it changes. Milestone 198's `Gate:` line reads `DECISION` alone, naming the format,
activation and trust forks in the sibling proposals as the decisions it waits on. Milestone 39's gate
is untouched.

## What is being decided

Milestone 198 carries `Gate: DECISION, MILESTONE 23`, inherited through milestone 39
(repository structure). The maintainer's reading was that this is a loop and that 198 does not need
the repository split's timing. The brief was to check that reading rather than adopt it.

## The premise, checked against the records

The loop is real, and it is written down in three places.

| Record | What it says | Where |
|---|---|---|
| Milestone 23 | `Gate: NONE`, status PARTIAL; the one residual (state handoff) is "declined for now, for want of a customer" | `design/roadmap/0023-component-os-live-replacement.md`, first paragraph; DECISIONS §116 ("revisit when a customer needs it, not before") |
| Milestone 39 | `Gate: DECISION, MILESTONE 23`; the split is "not before milestone 23 forces it" | `design/roadmap/0039-repository-structure.md`, lines 9 and 98 |
| Milestone 198 | calef, 2026-08-30: no third party sees nife until there is a package manager and a trivial install | its own block; DECISIONS §135; AGENTS.md principle 1 |

So 198 waits on 23, 23's residual waits on a customer, and a customer waits on 198. Milestone 23
cannot "force" anything while its residual is declined, so the `MILESTONE 23` token names a
milestone that is not going to move on its own. The `DECISION` half is a different claim:
§151 (2026-09-15) took the *goal* of the split and left the *order* as "a separate ruling", and 198
inherited that open ruling.

Does anything in 198's first slice actually need the split's timing? Four candidates, each
checked:

1. Where recipes and the distribution manifest live (this repository, or `basalt`, the
   distribution repository milestone 120 reserved). §151 says `basalt` "can begin as a manifest
   repository that names what the distribution contains before it holds any code". A recipe written
   in this tree today moves with `git mv` later. Reversible, so it does not wait.
2. Wire-contract versions. §151 lists them as a precondition for the *split*.
   `design/what-a-distribution-packages.md` puts the trigger for versions at "a binary first
   distributed to someone who cannot rebuild it". **That trigger fires for a package built at one
   commit and installed onto a system built at another. It does not fire for a whole image built
   from one commit**, because every component in it compiled against the same contract crates. So a
   first slice that composes whole images (see `what-trivial-install-means.md`, now §157, whose
   ruling put runtime install on the path) does not need
   versions. Runtime install of a separately built package does, and that is a protocol question
   (the note prices it: the `fs_proto` request word is full, so a version is a connect-time
   handshake), not a question about repositories.
3. A third party building a package outside this tree. This is the one real dependency found,
   and it is on an SDK rather than on the split. `helpers/build-ripgrep.sh` is the only
   out-of-tree build that exists, and it needs this repository checked out: `cargo xtask std-src` to
   build and link the `nife-dev` toolchain, `targets/<triple>.json`, and a linker script derived
   from `crates/user_mode_runtime/link.ld`. A stranger writing a package today must clone the whole
   OS to get a compiler. That is §151's "third-party programs" goal meeting a missing artifact (a
   downloadable toolchain and target description), and it is **not** decided by when the repository
   splits: the split would move the same files, not publish them.
4. The integration gate. Milestone 39's cost of splitting is that `script/test` stops proving
   the whole system. A package built by a recipe outside the gate can break without the gate seeing
   it. That cost arrives with the first out-of-tree package, whatever the repository layout, and it
   is a gate to build (a package's recipe is built and booted by CI) rather than a timing to rule on.

Conclusion: the premise holds with one amendment. Nothing in a first slice needs the split's
timing. One thing needs an artifact the split is often confused with, a toolchain a third party can
download, and it is recorded below as identified work rather than as a gate.

## Options

| Option | What it says | Cost | Why it lost, or did not |
|---|---|---|---|
| **A. `Gate: DECISION`**, the prose naming the format, activation and trust proposals | 198 waits on calef's rulings on the forks that are actually his | One line in a roadmap block | **Recommended.** It names what really stops a lane |
| B. Keep `DECISION, MILESTONE 23` | The status quo | Nothing to change; 198 stays unstartable by construction | Loses: `MILESTONE 23` names a milestone that cannot move until 198 exists |
| C. `Gate: NONE` and let a lane build a first slice under provisional answers | Fastest | A lane would pick a package format, which AGENTS.md puts in the irreversible category | Loses: the format and the trust root are calef's, and a provisional wire format is a contradiction |
| D. Gate 198 on milestone 39 instead | Honest about where the split question lives | 39 is `SUPERSEDED`, not a milestone a lane builds, and the split is not needed (above) | Loses on the premise check |

## What the tree already does in the analogous case

Milestone 47's `PATH` investigation (2026-08-26) reached the same shape from the other side and
declined to build a spawn-protocol change because it "risks answering [what installing means] by
accident, one opcode at a time". That is the reason option C loses, recorded by a lane that faced it
first.

## Reversibility, and who has acted on it

A `Gate:` line is the cheapest record in the tree: `script/roadmap` reads it and nothing else does.
Nobody outside this repository has acted on it.

## The §92 test

Would we choose A if B cost the same? Yes; B costs nothing and still loses, because it describes a
dependency that does not exist.

## If calef says no

198 stays unstartable until either milestone 23's residual is un-declined or the split's order is
ruled. The proposals beside this one still stand as analysis; only the building waits.

## Identified work this found

- A downloadable toolchain for out-of-tree packages (the `nife-dev` std, the target
  specifications, the linker script), which is what "third-party programs" in §151 needs before any
  third party can author one. Home: milestone 198's `BUGS`, where it is recorded as a limitation of
  the first slice, and the handoff in this lane's report as a candidate milestone.
