---
status: BUILT
raised: 2026-09-09
built: 2026-09-14
promoted_from: two-boot-mode-features-for-one-decision
---
# 296. The `initboot` feature was `shell`, and one of them went

**calef's ruling of 2026-09-14: delete `initboot`.**

The question arrived sideways. `script/initboot` was the **only one of fifty-five `script/` entry
points that ran two words together** (fifty-four now that it is gone), against the hyphen rule for
shell commands. Milestone 63 (one spelling per thing) swept the tree on 2026-08-01. This one survived that sweep
rather than being exempted by it, and
`design/roadmap/proposals/what-the-boot-path-is-called.md` had been holding a rename since
2026-09-08. Asked to rule on `progenitor-boot` or `handoff`, calef asked what the thing does, and the
answer was *nothing its sibling does not*. A naming question was answered by deletion.

## The premise was checked before it was acted on, and it held

The brief that minted this said to overturn the milestone if `initboot` differed from `shell`
anywhere. It does not, and the evidence is stronger than the `cfg` grep that raised it.

Six `cfg` sites named `initboot` (`kernel/src/arch/aarch64/timer.rs`, `kernel/src/sched.rs`,
`kernel/src/user.rs`, `kernel/src/memory.rs`, and two in `kernel/src/main.rs`; a seventh occurrence
is a doc comment quoting a pre-milestone-267 `cfg`). Every one of them read
`any(feature = "shell", ..., feature = "initboot")`. There was no site anywhere in the tree at
which `initboot` appeared without `shell` beside it.

The kernels were then built and compared, which is what makes this a measurement rather than a
reading:

| | `--features shell` | `--features initboot` |
|---|---|---|
| defined symbols | 3,109 | 3,109 |
| symbol names, normalized | identical | identical |
| total symbol bytes | 1,273,614 | 1,273,614 |
| `.text` section size | 462,848 | 466,944 |

The symbol tables differ in exactly one way: the crate disambiguator hash (`Cs9iDuAnI0Lr2_` against
`CselbPZ3Od6Uz_`), which is the feature set's own fingerprint, so a difference there is the *feature
name* and not the code. Normalize that one substring and `diff` is empty across all 3,109 names. The
4,096-byte `.text` difference is one page of link padding: the disambiguator changes an object
filename, which changes link order, which moves symbols across a 4 KiB boundary. Total symbol bytes
are equal to the byte.

**On riscv64 the relationship is not symmetric, and that is the one thing the grep would have
missed.** `shell` selects three things `initboot` never did: `riscv_hand_over()` in
`kernel/src/main.rs`, and two `cfg_attr` arms in `kernel/src/cap.rs`. So `initboot` was a strict
**subset** of `shell` on riscv64 and equal to it on aarch64. That is an argument for deleting
`initboot` rather than against it. The feature that selected less was the one nothing ran, since
`cargo xtask initboot` only ever built for `TARGET` (aarch64), and the only riscv64 `initboot` build
in the tree was `script/lint`'s own per-feature loop.

## `script/initboot` was deleted rather than kept as an alias

The two `xtask` arms were not identical, and what the difference was decided this.

`shell` sets `NIFE_RNG=1` (DECISIONS §120's 2026-08-26 amendment: *grant the QEMU-only virtio-rng
stopgap*, on the ground that a person booting interactively should have one), builds the RedoxFS
server, and rebuilds the RedoxFS fixture the prompt's `>` and `<` read. `initboot` did none of that.
Its difference from `shell` was entirely a matter of having less, so an alias would have
*added* capability to anyone who typed the old name. Keeping the arm as it stood would have kept
the copy this milestone exists to remove.

**And the omission was staleness, not design.** `helpers/qemu-runner-aarch64.sh` attaches the
fixture with `if [ -f "$REDOXFS_DISK" ]`, and the image lives at a fixed path under `target/`. So
what an `initboot` boot got depended on whether somebody had run `script/console` or `script/test`
in that checkout earlier: an environment that is a function of invocation history is not a design
anybody chose. `script/console` is the command, it has been the command since 2026-07-22, and two
names for one boot was the defect.

## How two features came to mean one thing

This is the part worth a paragraph, because nothing about it was a mistake at the time.

`initboot` was minted on 2026-07-25 for 2c of milestone 19d (a real workload), and it named a real difference. The
kernel stops wiring services and hands the machine to the userspace progenitor, which then builds
the console server, the input driver, the line discipline and the shell out of its own budget. The
`shell` feature at that date meant something else, the kernel-wired `shell_service`. Two boots, two
names, correctly.

Then the difference was eaten from both ends. DECISIONS §28 retired `shell_service` as a boot path
and milestone 41 deleted it, so `--features shell` started handing off to the progenitor too:
`initboot`'s distinguishing behavior became `shell`'s behavior. What was left for the pair to
select was only the milestone tour's absence, and both selected exactly that. Milestones 267 and
268 then moved the machine description and the tour apart and lifted the narrative out of the
kernel, and calef ruled the narrator deleted on 2026-09-13. Each of those steps took another thing
out of the space the two features were dividing. By 2026-09-09 milestone 267's lane measured the
pair and wrote the proposal this block was promoted from, and `kernel/src/main.rs` carried the
sentence *"Both features mean exactly this one thing"* into `main` under every gate.

No gate could have caught it, and it is worth being precise about why. A duplicate `cfg` arm is
not a warning, not dead code, and not a lint: both features compile, both boot, and the kernel that
comes out is the same one. The mechanism that found it was a reader following a naming rule to the
one script that broke it. That is rung four on AGENTS.md's ladder, and there is no higher rung
available for "two spellings select the same code" short of a check nobody has proposed.

## The measurement the proposal was written for, which still stands

`script/fastpath-footprint`'s method, release kernels, `.text` symbol bytes, taken at `8dd9dbf6`
before milestone 267 changed anything, and reproduced here unchanged because it is what makes the
remaining feature worth keeping:

| arch | tour build | `--features shell` | delta |
|---|---|---|---|
| aarch64 | 193,332 | 179,312 | 14,020 (7.3%) |
| riscv64 | 162,964 | 157,136 | 5,828 |
| x86_64 | 140,842 | 140,842 | **0** |

Two of those three are not what they look like. x86_64's boot arm is self-contained and ends in
`arch::halt()`, so the shared tour is unreachable there and LLVM had already deleted it; the feature
removes nothing on that architecture and never has. riscv64's arm halts too, and its `shell` swaps
its own arch tour for `user::boot_progenitor`, so 5,828 bytes is the difference between two RISC-V
boot paths rather than the cost of the aarch64 tour. The thing the surviving feature removes is one
architecture's 14 KB, and symbol attribution says almost none of it was ever the narrative.
`console_service` ~3.0 KB; the two never-yielding spinner closures ~3.3 KB; `virtio_service` ~1.7 KB;
`memory_region_service` ~1.3 KB; `kernel_main` itself +1,460. Each of those is a demonstration
needing a kernel privilege, which is why milestone 267 (the tour is three things wearing one name) left them where they are.

So the proposal's first question is answered and its second and third are dissolved. Is 7.3% on
one architecture worth a compile-time switch? It is kept, on that number. Should it be one feature
rather than two? It is now one. What is it called? `shell`, which is the name that was already
carrying the meaning.

## `script/lint` lost an entry without going blind

The per-feature clippy loop is `for feat in shell bench icount board fastpath_pad soak
cycle_counter_grant`, on two targets, and `initboot` came out of it.

Milestone 401, `design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md`, is the class of
failure this could have been. Milestone 265 (`_proto` is a truncation) is the worked example: a check that computed the set it judged
passed by checking zero crates on the one change that was its subject, going blind instead of
red. This loop cannot do that, and the comment above it now says so. The list is **written out, not
globbed**, so an entry leaving is visible in the diff, and every iteration echoes its own `==>` line,
so an empty list would be visible in the output. Seven features times two targets is fourteen lines,
and counting them is the check a reader can run.

The other `initboot`-shaped thing in `script/lint` was checked and is not this shape either: the
machine-description gate fails loudly with its own message when `print_machine_description` cannot be
found, rather than scanning an empty string and passing.

## `jobmix`, and why it was a repair rather than a tidy-up

Carried in the same milestone because it is the same defect one file over, and because leaving a
second squished name behind is a real cost.

calef ratified `crates/job_mix` on 2026-09-13, and had settled the stem a week earlier, choosing
`job_mix_task` over the maintainer's `mix_task` on 2026-09-05 for a reason the maintainer had not
made. From that crate's provenance block:

> *the family stays greppable as one string*, so `job_mix` finds this crate,
> `fixtures/src/job_mix_task.rs` and `script/job-mix`. Three members in three naming domains, each
> correct for its own, which is the domain table working rather than a coincidence.

The block counts three members and there were four. `kernel/src/jobmix.rs` is the kernel-side
supervisor of that same workload, it was squished, and a squish is precisely what a
separator-insensitive grep cannot reach. Measured at this lane's base `9b68f17e`:
`git grep -lie 'job[_-]mix'` returns 31 files and `git grep -lie jobmix` returns **25**, and
`kernel/src/user.rs` is in the second set and not the first. *(The first two commits on this branch
say 26. That was a hand count of a terminal listing, made before the numbers were re-taken against
the base commit; 25 is the measured one and those messages are left as they were written.)*
The property the ratification explicitly rests on was already false, and it was false because of
the one member nobody had counted.

So this is not the hyphen rule reaching a stray file. `job_mix` is the ratified name of this thing
and `jobmix` was a misspelling of it, so the module became `kernel/src/job_mix.rs`, the Cargo feature
became `job_mix`. That is also what every other multiword feature in this kernel already looks like:
`fastpath_pad`, `cycle_counter_grant`, `reboot_soak`. And `script/board-image`'s flag became
`--job-mix`.

**The console markers moved to `job-mix:` and `job-mix-census:`, and that half is provisional.** They
were checked for readers first, because a marker with readers is a contract. `crates/serial_console`'s
recognizer knows `soak:` and does **not** know this workload at all, which is what
`design/roadmap/proposals/a-board-console-recogniser-for-the-job-mix-sweep.md` exists to fix, so the
only reader in the tree is `xtask`'s own sweep and it moved in the same commit. The spelling is the
command's rather than the crate's, for one deciding reason: `xtask` already printed
`job-mix: QEMU ended before printing "jobmix: done"`, one sentence in two spellings. A marker is
neither a Rust identifier nor a shell command; it is a string a person reads after typing
`script/job-mix`. calef names what a reader meets, and he has not ruled on this one.

Proved by running it: `script/job-mix --arch aarch64 --smp 2` completes all six subruns, `xtask`
matches every renamed marker, and the run exits 0.

## A gate found nothing because it was not looking

`--features job_mix` failed `-D warnings` on both ISAs, and had for some time.
`user::boot_via_progenitor` is dead in a job-mix boot for the same reason it is dead in a soak boot
(both replace the progenitor handoff rather than following it), and the `allow` named only `soak`.
One line fixes it. Nothing found it, because `script/lint`'s loop does not carry this feature and
`script/job-mix`'s own `BUGS` says nothing else builds it either.

It was **not** added to the loop here. That is two more clippy builds on every pull request, and the
shape of the real answer is `design/roadmap/0373-board-only-features-nothing-compiles.md`,
which covers `reboot_soak` and `single_hart` as well. What this milestone did instead is write the
finding where a reader meets the feature, in `script/job-mix`'s `BUGS` and beside the `cfg_attr`
itself.

That same `BUGS` entry claimed CI builds neither this feature nor `--features soak`, and the second
half was false: `soak` has been in `script/lint`'s loop since the loop was written. It is corrected
rather than quietly narrowed, because a `BUGS` section that overstates is the failure AGENTS.md names
outright: a newcomer who hits a limitation the docs hid will not trust anything again.

## What kept its old spelling, and why

`design/naming.md`'s rule is that status decides what moves. Accounts keep `initboot` and say what
it means now: `design/decisions/0021-terminal-in-userspace.md`, `design/init-and-granular-spawn.md`'s
19d.2c record, `notes/progenitor-and-loading.md`'s milestone-41 narrative, and the pre-267 `cfg`
quoted in `kernel/src/main.rs` and `script/lint` (a quotation never moves, so it is annotated rather
than edited). Live documentation moved: `notes/scripts.md` lost the row,
`notes/check-inventory.md` lost the name and gained the count, `notes/line-discipline.md` and
`notes/riscv-parity-scope.md` now describe the one feature there is.

`design/roadmap/README.md`'s `BUILT` rows and the blocks for milestones 130, 264, 266, 267 and 268
keep every `initboot` and `jobmix` they carry, for the same reason: they describe what was true when
they were written.

## Follow-on

- **Done.** `design/roadmap/proposals/two-boot-mode-features-for-one-decision.md` is the file this
  block was promoted from: numbered, `git mv`d up a directory, index row added, which is the
  documented three-step promotion. Milestone 267's `Follow-on` bullet naming it moves from
  `**Proposed.**` to `**Milestone 296.**`, the same way 277's did when 288 was promoted.
- **Done.** `design/roadmap/proposals/what-the-boot-path-is-called.md`, written 2026-09-08, asked
  whether the boot path should be `progenitor-boot` or `handoff`. It is answered by neither, because
  calef deleted the thing it was naming; deletion is a third answer a naming proposal cannot
  reach on its own, which is the whole shape of this milestone. It was `git rm`d by this lane
  rather than promoted, since one milestone cannot be two files, and the bullets in milestones 266
  and 267 that cited it now cite this block.
- **Milestone 373.** `design/roadmap/0373-board-only-features-nothing-compiles.md`: `job_mix`,
  `reboot_soak` and `single_hart` are still built by nobody, and this lane demonstrated what that
  costs by finding one of them red. Not done here because the fix is a change to CI's shape.
- **Milestone 324.**: the
  board-side recognizer still cannot read this workload's markers, which is why renaming them was
  safe and is also why a board run is still read by eye.
- **Recorded.** The console markers' spelling is a lane's choice and calef has not ruled on it. It is
  written in `crates/job_mix/src/lib.rs`'s provenance block, where the rest of this family's naming
  argument already lives.
- **Recorded.** The limitations below stay limitations, and the one a reader meets away from this
  file is written where they meet it: `script/job-mix`'s `BUGS` carries the unbuilt feature.

## BUGS

- Nothing stops a second feature from meaning the same as a first. This one survived six `cfg`
  sites, two `xtask` subcommands, a `script/` entry point and a per-feature lint loop, and was found
  by a person following a naming rule to the one script that broke it. No gate in this tree compares
  what two features select, and none is proposed here, because the check is a compiler question (are
  these two `cfg` sets equivalent) rather than a text one. The cheap text version would fire on
  every legitimate `any(a, b)`.
- `--features job_mix` is still built by nobody but `script/job-mix`. Fixed once, here, and
  unprotected. `script/job-mix`'s own `BUGS` is where a reader meets this and carries the detail.
- The console markers are split from the feature's spelling on purpose and the split is real. The
  feature is `job_mix` and the markers say `job-mix:`. A reader grepping a board log for the feature
  name finds nothing. Both are found by `job.mix`, which is the property the ratification asked for,
  but a reader who does not know that has one more thing to know.
- `design/init-and-granular-spawn.md` and `notes/progenitor-and-loading.md` still tell a reader to
  run `script/initboot`. Each now carries a note saying the command is gone and what to type
  instead, which is rung three: the record is at the thing itself and nothing fires on its own. The
  alternative, editing the accounts, would make them describe a boot nobody performed on those dates.

## Index row

Promoted from the 2026-09-09 proposal milestone 267's lane wrote and was told not to act on. `script/initboot` was the only `script/` entry point running two words together; asked to rule on `progenitor-boot` or `handoff`, calef asked what the thing does and ruled it deleted, so a naming question was answered by deletion. The premise was measured rather than grepped: an aarch64 kernel built `--features initboot` and one built `--features shell` carry the same 3,109 symbols at the same 1,273,614 total bytes, differing only in the crate disambiguator hash the feature name itself feeds. On riscv64 `initboot` was a strict subset, selecting three things less. `script/initboot` was deleted rather than aliased, because its `xtask` arm differed from `shell`'s only by omitting the virtio-rng device and the RedoxFS fixture. **`jobmix` moved in the same breath and it was a repair, not a tidy-up**: calef's 2026-09-13 ratification of `crates/job_mix` rests on the family staying greppable as one string and counts three members. The fourth, `kernel/src/jobmix.rs`, is exactly what that grep misses (31 files against 25). Console markers to `job-mix:`. Found and fixed one line nobody was looking at: `--features job_mix` failed `-D warnings` on both ISAs. Merged with milestone 297 (`soak` becomes `soak-test`) and its `soak` to `soak_test` rename on 2026-09-15, carrying both renames through every conflicted site.
