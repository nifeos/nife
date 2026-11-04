---
status: BUILT
raised: 2026-09-21
built: 2026-09-25
promoted_from: a-seal-check-that-reads-bytes-cannot-see-a-check-that-was-dropped
---
# 563. A seal check that reads bytes cannot see a check that was dropped

The number is **provisional**: the integrator mints it at merge.
Promoted from the proposal `a-seal-check-that-reads-bytes-cannot-see-a-check-that-was-dropped` on
2026-09-22, filed 2026-09-21. Raised by the lane of milestone 523 (moving the job-mix supervisor
into userspace, and the five permissions it turns out to need). That lane was launched because
`script/board-image --job-mix --tftp` reported `NOT SEALED` on a card whose kernel and archive were
packed by one command. The cause is not the pair and not the seal's arithmetic. The kernel image
genuinely does not carry the digest, because the build that diverts the boot tour never calls the
code that would check it, and the linker removed it.

Built as option B, below, on 2026-09-25, when the same gap blocked xenon. The watchdog soak's lane
(pull request #1301, a wedged kernel resets itself) could not put a soak kernel on xenon's stick,
because `uefi_loader`'s build refuses an unsealed pair outright. It filed the proposal
`a-soak-kernel-cannot-reach-xenons-stick` on its branch. That proposal is this milestone arriving a
second time, so it was folded in here rather than given a number of its own. What was built and
measured is in Results.

## What was measured

In a lane worktree, alternating one cargo feature and counting the refusal message in the resulting
riscv64 release kernel:

```text
board          -> 3 occurrences of `MEASURED BOOT REFUSED`
board,job_mix  -> 0
board          -> 3
```

Since milestone 268 (every architecture boots the same way: describe the machine, test yourself, hand over) the
measured-boot refusal lives in `user::boot_progenitor`. `kernel/src/main.rs` reaches it only under
`#[cfg(not(any(feature = "soak_test", feature = "job_mix")))]` (`:1640`, `:2018`, with
`riscv_hand_over`'s own `cfg_attr` at `:2195`), and `bench` diverts at the same sites. So a `soak`,
a `job_mix` or a `bench` card **verifies nothing**, the trust root is unreferenced, and it is dead
code.

## Why the tool then says the wrong thing

`sealed_pair` decides whether a kernel vouches for an archive by scanning the image for the
archive's digest: `haystack.windows(needle.len()).any(|w| w == needle)`,
`contains` in `crates/sealed_pair/src/lib.rs`. That is a question about **bytes present**, and the question a
reader thinks they are asking is **will this kernel check**. The two agree on every default build
and come apart on every diverted one.

The message it then prints (`:257`) asserts two things that are both false for such a card: *"they
are from different builds"*, and *"this pair halts at MEASURED BOOT REFUSED after the power cycle"*.
The pair is from one build, and the kernel halts at nothing.

**The cost is real and was paid.** An hour before a bench evening on radon, a maintainer rebuilt the
card, ran `cargo clean -p kernel`, deleted the build directory, and got the same refusal each time,
because nothing in the message pointed at the feature.

**And the inverse is the security-shaped case.** The tool reads "digest present" as "will refuse".
Any future build that carries the digest for some other reason while not reaching the check would
read as SEALED while verifying nothing. That is the direction worth fixing even though nothing
in the tree does it today.

## Three options

A. Say what is actually true, in the message. When the image carries no trust root at all,
distinguish *"this kernel and this archive are from different builds"* from *"this kernel contains
no measured-boot check; it will accept any archive"* and name the features that do that. Cheapest,
forecloses nothing, and turns an hour of rebuilding into a sentence. This is the recommendation.

B. Have the diverted builds keep the check. Call `boot_progenitor`'s verification before the
workload replaces the tour, so a soak, bench or job-mix card refuses a mismatched archive like any
other. It is the honest position (a card left running unattended for hours is the last one that
should skip verification). It is also a change to what three kernel features do, which is a larger
claim than a tool fix.

C. Make the seal ask the real question. The scan cannot know whether a symbol is reachable, so
answering *"will this kernel check"* means the build emitting a fact about itself (a section, or a
line in the manifest `board-image` already writes). More machinery, and it is the only option that
closes the inverse case in general rather than by inspection.

## Results

The cause, confirmed with a symbol dump rather than inferred (x86_64, debug, 2026-09-25). A
kernel built `--features soak_test` beside an archive packed seconds earlier: `llvm-nm` finds no
`trust::TRUST_ROOT`, and none of the three digests in `target/init-measure-x86_64.txt` occurs in the
image's bytes, so `uefi_loader/build.rs` panics `NOT SEALED` naming `progenitor`, `hello` and
`program_measurements`. After the change the same build has `TRUST_ROOT` in `.rodata`, all three
digests present, and the loader builds.

What changed: `kernel::trust::require_program(name)` runs the chain an ordinary boot runs, two links
long: `require_program_measurements` vouches for the archive's table against the kernel image, and
the table vouches for `name`'s bytes (`measured_boot::verify_in_manifest`, the check the progenitor
makes on every load). A missing entry returns `None`, which the caller reports in its own words;
bytes the table will not vouch for halt at `MEASURED BOOT REFUSED`. The soak's `soaker`, the job
mix's `job_mix_task` and every program the bench enters now load through it. `disk_throughput`'s
private copy of the same chain (milestone 261 (the NVMe driver leaves the kernel, on the machine
that can finally confine it)) became a call to it. The trust root stays in the image because it is
used, which is the answer to the inverse case this block warned about: no `#[used]` keeps a digest
alive for a check nobody makes.

Proved on patagonia under QEMU:

- `cargo xtask uefi-image --features soak_test` builds (it could not before), and the resulting
  `target/esp` boots under OVMF through `helpers/qemu-stick.sh x86_64` into a soak that beats
  (`soak-test: t=31s beat=6 ... refused=0 mismatch=0 stalled=0`, two cores).
- `script/board-image --soak` for radon ends `SEALED: ... (3 measured entries ...)`, and
  `script/card-check` exits 0 on the payload. The radon lane of milestone
  225 (run the soak on radon, argon and xenon) recorded `NOT SEALED` for it on 2026-09-25.
- `script/soak-test --for 30s` passes on aarch64, riscv64 and x86_64.
- `script/job-mix` measures and enters `job_mix_task` and runs its sweep on aarch64 (debug) and
  x86_64 (`--release`), with no refusal. Both ran out of the default ten minutes before `job-mix:
  done` (exit 3), part-way through the 16-task point. That is the sweep's length under TCG, not
  the check, which runs once before the first point; whether `main` finishes inside the bound was
  not measured.
- Falsified: the x86_64 soak kernel pointed at riscv64's archive halts at `MEASURED BOOT REFUSED:
  'program_measurements' is not what this kernel image was built against`. Before this change the
  same pairing would have started the soak. The second link, a table that will not vouch for
  `soaker`, is `measured_boot::verify_in_manifest`, whose own tests cover both refusals. It was not
  falsified in a booted kernel: that needs an archive whose table is right and whose `soaker` is
  wrong, and the tree has no packer that makes one.
- The default `uefi-image` is unchanged in behavior: it still seals and still boots to the
  hand-over, which is where its own check already lived.

`uefi-image --features <list>` is the watchdog soak lane's patch, saved by its lane rather than
landed because nothing it built would seal; it lands here, where it does.

## What is already recorded, so this is findable without promotion

`notes/job-mix.md`'s `BUGS` and `crates/sealed_pair`'s each carry the limitation where a reader
meets it, per DECISIONS §71 (a limitation is promoted when it stops being a fact and becomes a
plan). This proposal is the plan half; the trigger for promoting it is a second person losing time
to the same message, or anyone wanting option B's guarantee for an unattended card.

## Index row

A `soak`, `job_mix` or `bench` kernel replaced the hand-over, so it never called the measured-boot refusal and the linker dropped its trust root. Every such card read `NOT SEALED` though its pair came from one build, and ran whatever archive sat beside it. xenon's stick, whose loader refuses an unsealed pair at build time, could not carry a soak kernel at all. Those boots now measure each program they enter through the progenitor's own chain (`trust::require_program`), so the trust root is in the image because it is used. A soak stick for xenon builds and boots under OVMF, and radon's soak card reads `SEALED`.

## Follow-on

- **Refused.** Option A, rewording the `NOT SEALED` message for a kernel with no trust root. With
  option B built, no build in the tree produces that case, so the sentence it would add describes
  nothing that can happen.
- **Recorded.** Option C, a seal that asks whether the kernel checks rather than whether the digest
  is present. Still the only general close of the inverse case; `crates/sealed_pair`'s `BUGS`
  carries it where a reader of the scan meets it.
- **Recorded.** Programs the boot tour itself enters before any hand-over (`outlaw`,
  `memory_region_depleter`, `entropy`, `block_driver`, loaded with `user::program` in
  `kernel/src/main.rs`) are still unmeasured on every build. The progenitor's check comes later and
  does not cover them. Recorded here rather than fixed because they are test fixtures of the tour,
  not workloads a card runs, and moving them onto `require_program` is a separate change to what
  the default boot does.
