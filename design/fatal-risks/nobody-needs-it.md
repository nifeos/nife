---
risk: 8
color: none
updated: 2026-08-31
---
# Appendix to risk 8: Nobody needs it

*An appendix to [`8-nobody-needs-it.md`](8-nobody-needs-it.md) (risk 8). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim

Everything works and no one has a reason to run it.

### The verdict of record

CANNOT-RUN, 2026-09-23. Untestable by this project's own policy, and no verdict. That is the finding
rather than an apology for not having one. The other eight entries can come back red. This one
cannot come back at all, because the observation that would answer it is gated behind a precondition
calef set and milestone 530 (name a customer, or admit the ranking function has nothing to rank)
ruled on. A fatal risk that cannot be tested is the most dangerous state a fatal risk can be in, and
this file's own rule 1 says why: an entry that cannot fail is indistinguishable from an entry that
passed. Risks 3 and 7 each found tests of exactly that shape inside the kernel, three sweeps
running. This is the same defect one level out, in the file that judges the project.

This one already fired, which is the most useful thing about it. AGENTS.md's principle 1 ranks work
by the shortest path to a system a customer runs. And in August 2026 the customer had a real
deadline, nife could not meet it. And the customer solved the problem with Linux: borg over SSH on
cordoba, Immich for images. Milestone 55 (Time Machine over SMB3) is `REMOVED` and journey 2 went
with it. That is the principle working exactly as designed, and it is evidence rather than failure.

### What it changed

The first customer was a family backup server, which is one of the largest things a home system can
be asked to be. A first customer should be something nife can plausibly be adequate at within a
milestone or two. The customer path is vacant, and it is recorded as vacant rather than implied by a
roadmap that still names one.

The evidence today, counted rather than characterised, and it points one way.

- One user, who left. That is the entire user history of this project, and the workload he left for
  is the workload that motivated it.
- Zero others, and zero is not the damning part. Nobody has been offered this system, so a count of
  no users measures no demand and no supply at the same time. The absence of evidence is the
  problem, not the evidence of absence: an untested claim cannot be quoted in either direction, and
  this entry is worth nothing if it is read as "probably fine, nobody has complained."
- Nothing installs, so there is nothing to count. Milestone 576 (how many systems are out there, and
  what do they run) is `NOT-STARTED`. It is gated on milestone 801 (packages over the internet), which was rung 3c of milestone 198
  (a package manager) until 198 was split on 2026-10-06. Its gate read `DECISION` until
  2026-09-24; the three forks were ruled by 2026-09-23: §195 (a reviewed recipe vouches for a
  package), §197 (a package is one archive file) and §208. It reads `DECISION` again, on §219,
  since the fetch-and-verify lane of the same day found the next fork.
  The project has no instrument that could observe a user if one appeared.
- The one published argument that addresses this says it goes badly, and it is cited below.
- What the green results buy is narrower than it reads. Risk 1 is green on three architectures
  (unmodified `ripgrep`, zero patches) and risk 9 is green on three silicons. Those answer *could
  somebody run this*. This entry asks *does somebody want to*, and no amount of the first answers
  the second. Treating capability as demand is the specific error this entry exists to prevent.

Why no verdict can be rendered, stated as the loop it is. Nobody can be asked to run nife until it
installs (calef's precondition: no third parties until a package manager and a trivial install
exist). It will not install until milestone 198 lands. Since §208 (installing is granting) ruled
the last of its forks on 2026-09-23, 198 waited on work for a day: rung 3a's consumer half, which
fetches, verifies, installs and removes a package on a running system. The lane that built it got
through fetch and verify (2026-09-24) and stopped at a narrower fork, §219 (how the shell names an
installed program to the spawner). It is calef's because the shell and the progenitor must agree
on the answer. Milestone 530 ruled on 2026-09-21 that the customer path stays vacant, and is blocked rather
than empty. It refused the closest candidate, a measurement appliance for this project's own
benchmarking, because that would be the architect in a different hat, letting the ranking resume
without resolving what made the path vacant. That ruling is correct and it is also what seals this
entry: the strongest available reading of principle 1 while the path is blocked is that 198 inherits
the ranking function's top slot. And until it lands this risk accrues in silence while the roadmap
grows.

### What would falsify it, concretely

One observation, in two readings that must not be confused:

1. Somebody who is not calef installs nife on purpose and is still running it two months later. The
   install is the weak half; a person tries anything once. Retention is the claim. And milestone
   576's instrument is built to see exactly that distinction: Fedora's `countme` model attaches an
   age bucket to a repository request the system was already making. So a first check-in and a
   long-lived one are distinguishable without any identifier being sent or stored. An install curve
   that reaches a handful and an age bucket that never ages is the red result: people look, and
   nobody keeps it.
2. A package fetched by a system that is not ours. Under the same design, package popularity is the
   fetch traffic counted per package with no linkage between one system's requests, which means
   somebody chose to do something specific with this OS rather than merely booting it.

Is 576 the right instrument? Yes, and it will not work at the scale this project is at. It is the
first mechanism nife will have for learning whether anyone runs it. And the `countme` model is the
right one for the reason its milestone gives: counting installs without an identifier is the only
shape that is defensible when it is on by default. But its arithmetic is distinct addresses per
bucket per week, designed for a distribution with millions of systems. At three, the noise is the
whole signal: one person on a changing address counts as several, a household behind one NAT counts
several as one, and no number of weeks fixes either. So 576 answers this risk only at a scale nife
is a long way from, and below that scale the falsifying observation is not a number at all. It is
one named person who is not calef, which is what milestone 530 asked for and could not get. Both
instruments should be expected: 530's for the first user, 576's for the hundredth.

And 576 cannot answer the follow-on question by construction. Its counts are unlinkable on purpose,
so it can report how many and which packages, and never why anybody stayed. Nothing in this tree is
an instrument for that, and nothing is planned to be.

The counter-thesis here is published too, and it is a different argument from risk 4's. *An
Incremental Path Towards a Safer OS Kernel* (Li, Miller, Zhuo, Chen, Howell, Anderson, HotOS '21,
DOI 10.1145/3458336.3465277) argues that memory safety should be brought to the kernel people
already run, module by module. Its reason is this entry's claim: clean-slate kernels *"have
significantly fewer features than Linux ... Impeding adoption"*, and *"the cost of switching from
Linux to these clean-slate designs is prohibitive due to the established Linux and Android
ecosystems."* There is a measurement behind it. 1475 Linux CVEs bucketed, 42% reachable by type and
ownership safety and 35% more by verification, and ext4 still minting CVEs after seven years of use.
It takes no position on capabilities or on what a crossing costs, which is why it belongs here and
not in risk 4: no benchmark decides it. And a microkernel that wins every crossing number and that
nobody runs has lost this argument anyway. `notes/incremental-path.md` has the paper, the answer
(the incremental path is only available to an actor who can move an existing system, which this
project is not). And the check of whether Linux is walking it: at Linux 7.2.4, five years on, 0.166%
of the tree is first-party Rust, all of it new leaf code, with no existing C subsystem replaced and
functional correctness not begun.

What the project is doing in the meantime, and which half is a plan. The plan is real and it is
narrow: milestone 198 with §157 (a trivial install is a web page, a USB drive and packages) is the
route to being installable. And milestone 530 converted this gap into a gate by writing the
precondition down where a reader meets the empty path. That is a plan to become testable, not a plan
to be needed, and the distinction is the whole of this paragraph. Nothing in this tree is an attempt
to find out whether anybody wants this. There is no announcement, no outside reader, nobody asked.
The order being followed is *build until it is presentable, then look*, which is defensible and is
also precisely the order that lets this risk stay unanswered longest.

### The rest is a hope, and it is recorded as one

The unstated expectation carrying this project is that a system running unmodified third-party
software on three architectures, with a verified core and a confinement story, will find users once
it is installable. No evidence in this tree supports that, the one published argument that speaks to
it says otherwise, and every green result on this list makes nife more plausible to run without
making anyone more likely to.

The experiment that has not been run, and currently cannot be: milestone 576, which needs rung 3c of milestone
198 first and then needs a population this project does not have. Until then the honest entry is the
one above. This risk is the question the other eight are in service of, and it is the only one with
no answer and no scheduled way to get one.
