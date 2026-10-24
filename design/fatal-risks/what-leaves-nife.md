---
risk: 8
color: none
updated: 2026-10-18
---
# Appendix to risk 8: Nobody else builds on nife's mechanisms

*Name: provisional, minted 2026-10-18 (UTC) by lane maintainer/risk-8-restatement for risk 8's
appendix, with the risk file's stem `8-nobody-else-builds-on-nife`, which is provisional too. The
risk stem states the claim in the claim form; the appendix stem names what risk 8 counts.*

*An appendix to [`8-nobody-else-builds-on-nife.md`](8-nobody-else-builds-on-nife.md) (risk 8). That
entry is the claim of record, and it is written so that a reader can decide what to work on next
without opening this file. This one exists to be verified or challenged: it holds the evidence, the
dates, the numbers, the corrections and the refusals behind the verdict, at the length they need
rather than the length the six-pager has. Where a study has its own home in `notes/` this page links
it rather than copying it. Name provisional (`design/fatal-risks/` and this file's stem); naming is
an architect's.*

### The claim

nife's mechanisms, released as crates or offered to existing ones, are not taken up by anyone else.
nife cannot shrink, because nothing it builds finds a home outside it.

calef, 2026-10-18 (UTC), on what winning looks like: *"Winning in Linux looks like a long road of
successful crates that Linux could leverage in their work."* And on composition: *"Dozens of OSs
composed from nife's components is a great legacy."*

### The verdict of record

None. The experiment has not been run, so `color:` above is `none`, and a color is an architect's
to render (§216 (fatal-risk facts are correctable, and verdicts are the
architect's)).

### How the four observations are weighed

Absorbed and handed off are worth more than adopted, because they reduce what calef maintains.
Composed is the legacy path, how the approach spreads beyond one tree. Adopted is the weakest of the
four and the likeliest to come first.

Absorbed is a nife OS mechanism or proof only. A contribution to the toolchain, such as
[model-checking/kani#4913](https://github.com/model-checking/kani/pull/4913)
([`notes/kani-upstream.md`](../../notes/kani-upstream.md)), is recorded under lines nife owns once
nife deletes its patched Kani, and not as absorbed.

Taught counts once a course chapter is live, and is a leading indicator until then. calef,
2026-10-18 (UTC), asked whether a class teaching from the course is people using nife's crates or
something leaving nife: *"Using the crates (B)"*. So it joins adopted and composed toward green's
three, never absorbed or handed off. Readers of a course are a leading indicator,
and an instructor using it in a class is the stronger one, because a class's students build kernels
from the crates and that feeds composed. The rust-osdev crates (`x86_64`, `bootloader`) spread
largely through a tutorial built on them, and so did Limine and smoltcp: someone showed them working
in a kernel. A crate that is only published will mostly sit there.

### When an offered proof counts as absorbed

A proof counts only if both hold:

- It guards a boundary where failure is a crash, a memory-safety violation or an isolation escape,
  not a convenience property.
- Its falsification patch reproduces a plausible bug, ideally one from that project's own issue
  history or a sibling project's. Firecracker's guest-controlled virtio queue overlapping MMIO is the
  model. A proof that only a contrived patch can show red is trivia, which is risk 2's claim.

### The bar for a crate nife would build on

A crate nife depends on, whoever maintains it, should meet the gates `nifeos/crate-workflows`
encodes: Kani with every harness falsified (`proofs / falsify`), a mutation baseline
(`weekly / mutation`), semver checks (`weekly / semver`), a pinned minimum Rust version
(`weekly / msrv`), and a stranger who has built and proved it from its README (`weekly / stranger`).
An absorbed mechanism that drops below this bar after nife leaves it is a partial failure, and the
entry should say so.

### The clock

Each honest attempt runs its own 6-month clock, and attempts run in parallel. calef, 2026-10-18
(UTC): *"It seems like the attempts might run in parallel, so clock per attempt might make more
sense. However I don't think we're successful if nothing has moved in a year."* So red is readable
early, once five attempts have each passed their 6 months with no uptake, and the year mark,
2027-10-18, is red unless something has left nife. An attempt at a counterpart that refuses AI-written
work cannot be honest under that policy, so it is not one of the five.

### What exists to attempt with

[`notes/releasable-crates-2026-10-07.md`](../../notes/releasable-crates-2026-10-07.md) counts 30
crates that could leave: 23 standalone (class A) and 7 one small cut away (class B), holding 135 of
the 222 Kani harnesses under `crates/`. The mechanism crates `capability`,
`inter_process_communication` and `direct_memory_access_validator` have no nife dependencies. What
does not travel is the glue in nife's kernel: revocation, the one-shot Reply and mailboxes.
Composition that counts double needs that glue extracted, or a clear guide to writing it.

Two candidate shapes for a first attempt. A contribution to an existing project, where rust-vmm's
virtio crates already run Kani and `direct_memory_access_validator` proves properties of virtio
descriptors. And a crate published as nife's own where no maintained counterpart exists. Which three
go first is an architect's pick.

### Caveats

- Linux is the long tail, not the test. Linux takes Rust code by bringing it into its own tree, not
  by depending on crates.io, so a nife crate reaching Linux is a years-long outcome. This entry
  measures the road there.
- Contribution policies cost attempts. Some counterparts refuse AI-written work (`time`, `jiff`,
  `globset`, rcore-os), and those get disclosed bug reports only. calef's view is that the policies
  will not matter in the long run, because delivered value wins. Within this entry's clock they
  decide which attempts can count.
- This entry pulls against Fork 1 of milestone 857 (crates released on their own), one repository
  per crate under `nifeos`, which is the model where calef maintains every crate. Under this entry
  a nifeos repository is a holding pen, and success is a crate leaving it.
- A second kernel shape built from the same crates would show they are general and not nife-shaped.
  It is not composed, because it is not an outside project, but it is a cheap check worth making
  before asking anyone else to compose from them.

### Nobody needs it: a non-fatal note

The claim: everything works and no one has a reason to run it. It is not fatal while nife is a
demonstrator (§14 (project direction)). Nobody chooses a new OS over Linux for general use, and this
note expects that to stay true. If nife turns back toward being a product, this claim returns as a
fatal risk.

It has fired once. In August 2026 the project's first customer, a family backup server, had a real
deadline, nife could not meet it, and the customer solved the problem with Linux: borg over SSH on
cordoba, Immich for images. Milestone 55 (Time Machine over SMB3) is `REMOVED` and journey 2 went
with it. That is AGENTS.md's principle 1 working as designed. A first customer should be something
nife can plausibly be adequate at within a milestone or two, and the customer path is recorded as
vacant rather than implied by a roadmap that still names one. Milestone 530 (name a customer, or
admit the ranking function has nothing to rank) ruled on 2026-09-21 that the path stays vacant,
blocked rather than empty.

As a test it cannot run yet. Nobody but calef can be asked to run nife until it installs (milestone
802, the trivial install), and milestone 576 (how many systems are out there, and what do they run)
waits on milestone 801 (packages over the internet). The observation that would answer it is
somebody who is not calef installing nife on purpose and still running it two months later. The
install is the weak half; retention is the claim. 576's counts are designed for a distribution with millions of systems, so below that scale
the answer is one named person rather than a number.

The green results on risks 1 and 9 answer *could somebody run this*. This note asks *does somebody
want to*, and capability is not demand.

The published counter-thesis is *An Incremental Path Towards a Safer OS Kernel* (Li, Miller, Zhuo,
Chen, Howell, Anderson, HotOS '21, DOI 10.1145/3458336.3465277). It argues that memory safety should
come to the kernel people already run, module by module, because clean-slate kernels *"have
significantly fewer features than Linux ... Impeding adoption"*. Its measurement is 1475 Linux CVEs
bucketed, 42% reachable by type and ownership safety and 35% more by verification. It takes no
position on capabilities or crossing cost, so no benchmark decides it.
[`notes/incremental-path.md`](../../notes/incremental-path.md) has the paper and the answer: the incremental path is open only to an actor who can move an existing
system. Risk 8's claim is where that answer meets this project, because a crate Linux could take
is the one incremental path nife can walk.
