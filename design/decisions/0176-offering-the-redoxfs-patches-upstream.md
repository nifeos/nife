---
status: SUPERSEDED
raised: 2026-09-19
superseded_by: 46
---

# 176. Offering the two RedoxFS patches upstream, and under whose name

§46 (Thin primitives or whole subsystems; we write everything in between), in [`0046-dependency-rule.md`](0046-dependency-rule.md), rules question 1. §46 says, in "Who rules": *"Redox is a source we consume and never contribute to."* It adds that Redox's CONTRIBUTING refuses LLM-generated contributions, so no code goes back. A patch nife needs is carried here under §34 (RedoxFS is the primary filesystem, on three conditions). That is option C below, and it makes question 2 (under whose name) moot, since no merge request is written. The position calef ruled on #1815 (2026-10-07): bug reports to Redox are allowed, each disclosed as agent-written and approved by calef first, and a security finding goes to Redox privately before anything is public. No code and no patches are offered, because Redox refuses AI-generated contributions (`notes/releasable-crates-2026-10-07.md`, pull request #1806) and nife sends code only to projects that accept it. Milestone 347 (offer the two RedoxFS patches upstream) is REFUSED on the same grounds. The text below is the question.

## What is being decided

Two questions, and the second is the one nobody has asked:

1. Are the two RedoxFS patches offered upstream at all?
2. Under what identity, given that both were written by an agent and both carry
   `From: Chris Alef <chris@crickertech.com>` in their `git format-patch` headers.

## Why this is calef's rather than a lane's

A merge request is a fact that leaves the machine. AGENTS.md puts that in the irreversible
column with names, dependencies and the syscall surface, and gives the reason: *"Nobody can
un-publish a decision."* [§79](0079-password-equivalent-material.md) is the worked precedent, where an
hour of argument was worth spending because the decision *"cannot be unmade by deleting the code"*.

A merge request under this project's name is smaller than §79's case and the same shape. It is also
mechanically outside a lane: it needs an account on gitlab.redox-os.org, a fork and a push.

## The tree as it stands, read rather than recalled

`patches/` holds exactly two files beside its README, checked 2026-09-19. The README's opening
sentence is the standing commitment: *"Each exists to be upstreamed; an entry leaves this directory
when the pin that needed it advances past a release containing the fix."* The submission route is
written out for both in the future tense, and no merge request is recorded anywhere in the tree.

| patch | against | state |
|---|---|---|
| `redoxfs-no-std-vec-import.patch` | master @ `99bc185` (2026-07-27) | four `E0425` sites across `filesystem.rs` and `record.rs`, plus a `--no-default-features` CI job so the configuration cannot rot again. Verified on the host and cross-compiled for `riscv64imac-unknown-none-elf` and `aarch64-unknown-none-softfloat`. |
| `redoxfs-no-std-create-uuid.patch` | the published 0.9.1, deliberately | `Header::new_with_uuid`, `FileSystem::create_reserved_with_uuid`; the `std` entry points keep their signatures and no existing caller changes. Applies to the pin with zero fuzz; **rebasing it onto master is the submitter's first step**, and the README already says so. |

## What each option costs

| | shape | cost |
|---|---|---|
| **A** | **Offer both.** Fork, `git am`, push, one merge request each, rebase `create-uuid` onto master first. | An account and an evening, then waiting and answering whatever upstream asks, which has no schedule. The `no_std` build of RedoxFS is broken upstream for everyone and this tree has the fix in a file, so A is the option that costs somebody else nothing. |
| **B** | **Offer only `vec-import`.** | It is the pure bug fix, is written against master, and needs no rebase. `create-uuid` adds API surface, which is a larger thing to ask a maintainer for and a larger thing to defend. |
| **C** | **Offer neither, and say so in `patches/README.md`.** | Free, and honest in a way the current state is not: the directory's own opening sentence promises upstreaming, so silence reads as intent. The recurring cost stays, since milestone 203's machinery reports every upstream move and each report then re-applies both divergences by hand, with `create-uuid`'s rebase getting more expensive the longer nobody does it. |

No recommendation on question 1, deliberately, for the reason above: it is a fact that leaves the
machine, and AGENTS.md's own limit says those arrive as options.

## Question 2, which is the part this block never asked

Both patches carry calef's legal name and address in their `From:` header, which is exactly right by
AGENTS.md's rule that legal names belong in *"legal and authorship strings ... patch `From:`
headers"*. Both were written by an agent.

In this repository that gap is covered by a stated convention: every pull request and comment an
agent writes opens by saying so, because until milestone 128 gives the automation a real identity
*"every artifact in this repository carries calef's name whether he wrote it or not, and a reader
cannot tell the architect's voice from a lane's"*. That convention is local. A merge request on
gitlab.redox-os.org is read by people who have not read AGENTS.md and who are being asked to take
code into their project on the strength of who appears to have written it.

So the options are: say so in the merge request description, say nothing, or wait for milestone
128's identity. **This one does want a recommendation, because it is reversible in the way question
1 is not**: a sentence in a description can be written differently next time, and the honest version
costs one line. Recommendation: say so, in the same words the in-tree convention uses.

## What is blocked until this is answered

Milestone 347, and nothing else. The mechanical part is small; the rest is waiting.
