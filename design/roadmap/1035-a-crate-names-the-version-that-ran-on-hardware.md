---
status: PARTIAL
promoted_from: a-crate-names-the-version-that-ran-on-hardware
raised: 2026-11-05
milestone_dependencies: 159, 818, 857
decision_dependencies: none
machine_requirements: riscv64 silicon, a StarFive VisionFive 2 (JH7110)
specific_machine: none
needs_person: yes
---
# 1035. A crate names the version that ran on hardware

*(Number provisional until the integrator mints it. The title and slug are drafts.)*

Raised by lane/hardware-claim-names-a-crate-version.

Reuse: the bench procedure of milestone 159 (a real hardware entropy source: the JH7110's TRNG),
[`bench-procedure.md`](0159-jh7110-trng-driver/bench-procedure.md), the boot tour's `trng` line,
`script/board-image`, `script/board-console` and the network boot of milestone 257 (boot radon over
the network). One new helper, `helpers/crate_hardware_run.py`, reads the transcripts against the
definition below, because no existing tool knows the definition.

## Background

`jh7110_entropy` lives in its own repository, `nifeos/jh7110_entropy`. Its tag `v0.2.0` names
commit `cfdbeb7`, and nife pins exactly that commit, with `version = "0.2.0"`, in three manifests:
`kernel/Cargo.toml`, `components/Cargo.toml` and `crates/machine_discovery/Cargo.toml`. The crate's
`main` has moved past the tag with breaking changes while its `Cargo.toml` still says 0.2.0, and
milestone 1049 (`jh7110_entropy` is ready to publish) rules that 0.3.0 is the first version
published to crates.io. So a version string alone does not name a tree here; a commit does.

The crate's README has a "Hardware evidence" section. It records two board runs of crate commit
`ee0bde8`, made by milestone 1049's bare-metal payload, and says that no other commit has run. It
names no version: `ee0bde8` sits between the tags, and the payload is not nife's driver. Milestone
159 drove an earlier tree, under nife, on radon.

No milestone runs a tagged version of the crate in nife's driver. Milestone 159 has the status BUILT. Milestone 818 (proven JH7110
TRNG logic, released on its own) is the release and names no bench step. Milestone 225 (run the soak
on radon, argon and xenon) soaks the scheduler, not this driver. Milestone 239 (radon's device tree
does not describe the TRNG, so a working driver never runs) concerns the device tree. Milestone 224
(nothing can power-cycle radon) has the status REFUSED, and milestone 257 holds the reason: the
maintainers chose manual power, because network boot removed the card and left remote power buying
only the hung-board case. So a run of a crate version is a bench session with a person present, as
milestone 192 (a keyboard on real silicon) is.

The question: how does the crate earn the right to say that its current version ran on a VisionFive
2, and how does every crate with a hardware claim do the same?

## What a hardware run of a crate version is

A hardware run of `jh7110_entropy` version V is all of these:

1. A build of nife comes from a commit whose three manifests pin the crate at the commit of tag `vV`
   and state `version = "V"`. Its own pull request bumps a pin and cites
   `nifeos/jh7110_entropy@<sha>`, so the commit exists on `main` before the run.
2. That build boots on a VisionFive 2 by the procedure in `bench-procedure.md`, twice.
3. On each boot the tour's `trng` self-test line is `ok`: the entropy service served both 32-byte
   draws to a client, and the second draw differs from the first. The line ends with the crate
   version and commit the build resolved, which `components/build.rs` reads from `Cargo.lock`.
4. The line's mode note says 256-bit.
5. No lockup: the line is not a `FAILED` line, which is where a persistent `LFSR_LOCKUP` lands.
6. The first draws of the two boots differ.

It shows that this version's decisions, wired into nife's driver, bring the block up and serve
nonzero, changing bytes on that board class. It does not show randomness quality: a TRNG has no
known answer, and there is no health test and no NIST SP 800-90B measurement. It does not show any
other board, or any version but V.

## Who runs it

A person, so `needs_person: yes`. The board needs its serial console attached and its power switched
by hand, and a hung board needs a person because the maintainers refused remote power. Network boot
spares the card. The maintainers keep the bench rules for the board itself (wiring, outlets)
off-tree; the public procedure is `bench-procedure.md`. An agent prepares the build at the pin, and
`helpers/crate_hardware_run.py judge` reads the transcripts against the six conditions above.
Neither the run nor the reading runs in CI, since no runner reaches the board. The steps a person
follows are in [`bench-steps.md`](1035-a-crate-names-the-version-that-ran-on-hardware/bench-steps.md).

## Where the evidence lives

nife keeps the log. The run's transcript goes under `bench/<board>-<date>/`, as milestone 159's runs
do, and the block of the milestone that ran it holds the `trng` line of each boot with the nife
commit and the crate version.

The crate states a fact, not a story. Its hardware section names the version and the board class,
and nothing about dates, people, faults found or earlier names. It does not link nife, because a
crate repository stands alone. A later release does not edit the sentence: "0.2.0 ran on a
VisionFive 2" stays true when 0.3.0 ships, so the README never claims an untested version.

## The edit the crate README then makes

The "Hardware evidence" section opens with:

> Version 0.2.0 has run on a VisionFive 2 (JH7110): it served real TRNG output on two boots with the
> chip in 256-bit mode, the draws differing within each boot and across the boots. The clock and
> reset must be ungated and released first, as the first item under BUGS says.

Its "Not established" bullet on other commits then names both trees that ran, 0.2.0 and `ee0bde8`.
The bullet on randomness quality stays. A draft pull request in `nifeos/jh7110_entropy` holds the
edit, unarmed until a run passes.

## Which version runs first

The maintainers chose 0.2.0, provisionally. It is the only tagged version nife can pin today, and
nife's `main` pins it already, so the run needs no tag, no release and no pin bump. 0.3.0, the first
version crates.io will carry, cannot run until its tag exists, and its tag waits on the publishing
hold. When it exists, the same steps run at the pull request that bumps nife's pins to it, and the
README names 0.3.0 in place of 0.2.0, as option A says.

## Options

Ruled: **A.** The others lost for the reasons given.

- **A. The README names the newest version that ran, by board class; nife keeps the log.** As above.
  A stranger reads one sentence that is true for as long as the README exists. Rejected claims cost
  nothing: a version that never ran is simply not named.
- **B. A "tested on" table in the crate repository, one row per run.** Each row holds a date and a
  transcript. A row carries history, which a document here does not hold (§298 (a document states
  what is in force now)), and a transcript from nife's tour pulls nife into a repository that stands
  alone.
- **C. No hardware sentence; host tests and proofs only.** The board is the one thing a driver user
  wants to know about, and the evidence exists.
- **D. A runner on the board, so CI attests each version.** The best evidence, and not buildable
  now: the maintainers refused remote power, and a hung board needs a person. It waits on milestone
  224's revisit.

Would A still win if all four cost the same? Over B and C, yes. D would win, which is why A lets a
CI run later fill the same sentence without changing it.

### Prior art

- Linux's `Tested-by:` tag (`Documentation/process/submitting-patches.rst`) records that a person
  tested a patch "in some environment", and a new version drops the tags when the patch changes
  substantially. This is A's rule at a finer grain: the claim binds to one version and does not
  carry forward.
- KernelCI runs the kernel on boards in contributed labs; that is D at scale.
- Zephyr's board porting guide asks that each new board target pass a minimum test suite with
  Twister before upstreaming, and keeps per-board Twister metadata. The claim binds to the board,
  not to a release.
- The `ssd1306` driver crate's README names no board; its example imports an STM32F1 HAL, so a
  reader infers the tested target rather than reading it. That is the failure A prevents.

## The rule for every crate with a hardware claim

The rule generalizes. A crate README states a hardware run only by naming a version that has one, by
the definition above adapted to the device. That means nife built at the pin that names the version,
booted on the board by its public procedure, with the device's own success line read. It is a gate
on that sentence alone, never on publication: a crate publishes on proofs and a stable API, as
milestone 818 says, and a release with no run simply makes no hardware claim.

Today's claims under it:

- `e1000e`: "The I219 port has run on real hardware once, with no cable", which names no version.
  Under the rule it names the version or becomes a limitation.
- `jh7110_clock_and_reset`: "None of the bring-up has been confirmed on silicon end to end", already
  a limitation, and correct.
- `designware_ethernet`, `designware_mobile_storage` and `non_volatile_memory_express` make no run
  claim in their READMEs today; one that they make later follows the rule.

## The work

1. **Needs a person; nobody is assigned.** nife's `main` pins the tag `v0.2.0` already. Run
   [`bench-steps.md`](1035-a-crate-names-the-version-that-ran-on-hardware/bench-steps.md) on a
   VisionFive 2, commit the transcripts under `bench/`, and record each boot's `trng` line, the nife
   commit and the crate version in this block.
2. In `nifeos/jh7110_entropy`, after step 1, make the README edit above.
3. Write the rule into milestone 857 (crates released on their own), beside the release rules, so
   every crate's review reads it.
4. Bring the `e1000e` sentence under the rule before that crate moves: it names the version that
   ran, or it becomes a limitation.

Steps 2 to 4 need no board. Step 2 waits for step 1, because the README names a version only once
that version has run. Steps 3 and 4 state the rule and do not wait. Step 1 is the only step with
`needs_person: yes`, and the milestone stays PARTIAL until a person runs the board.

## Done when

- This block holds two `trng` success lines, 256-bit, whose first draws differ, from a nife
  commit that pins `jh7110_entropy` 0.2.0, and `helpers/crate_hardware_run.py judge` prints `PASS`
  over their transcripts.
- The crate README names 0.2.0 as run on a VisionFive 2 and carries no limitation about an untested
  tree.
- Milestone 857 states the rule.

## Cost and what cannot be undone

One bench session with a person, about two boots over the network, plus two small edits. Nothing
here is one-way: anyone can reword a README sentence, and the rule only narrows what a README may
claim, so loosening it later breaks nobody.

## Follow-on

- **Outstanding.** Step 1, the bench run: two boots of nife `main` on a VisionFive 2 by
  [`bench-steps.md`](1035-a-crate-names-the-version-that-ran-on-hardware/bench-steps.md), with
  `helpers/crate_hardware_run.py judge` printing `PASS` over both transcripts. Checked against the
  tree: `main` pins `jh7110_entropy` at `cfdbeb7`, the commit of tag `v0.2.0`, and no transcript under
  `bench/` carries the `trng` line's crate identity yet.
- **Outstanding.** Step 2, the README sentence in `nifeos/jh7110_entropy`. Its draft pull request
  waits for step 1, since the README names a version only once that version has run.
- **Done.** Step 3: the rule is in milestone 857's extraction checklist,
  `design/roadmap/0857-crates-released-on-their-own/what-nife-does-at-each-extraction.md`.
- **Done.** Step 4: the `e1000e` I219 sentence becomes a limitation, in its own repository's pull
  request, since no version has run on an I219.

## BUGS

- Milestone 159's `bench-procedure.md` describes the card path and a `hw entropy` line; the tour
  prints the `trng` self-test line now, and the network boot replaces the card. `bench-steps.md`
  here is current; that page is not.
- The README's "Hardware evidence" section carries dates, the person's statements and a run
  narrative, which option A keeps out of a crate's hardware sentence. The version sentence above
  goes first; whether the run record below it stays is milestone 1049's to settle.
- The judge cannot tell a cold boot from a warm one. The person states it, and the bench README
  records their statement.

## Index row

A crate's README states a hardware run only for a version that has one. Nife built at the pin that names the version must boot on the board by its public procedure. The first case is `jh7110_entropy` 0.2.0 on a VisionFive 2.
