---
status: BUILT
raised: 2026-09-02
built: 2026-09-02
---
# 230. `script/shell-check` is red on `main`, on both architectures, and nothing says so

Minted the same day by the maintainer, from milestone 192's (a
keyboard on real silicon) lane, which found it while proving an unrelated boot path. *(Number
provisional until the merge queue lands it.)*

It was minted with no gate, on the grounds that the defect was reproducible on patagonia and needed
nothing this project does not have. That held: it was reproduced, root-caused, bisected to a merge,
fixed, and `script/shell-check` is green on both architectures.

The script is `script/swish-check` since 2026-09-23, and this block keeps the old name
throughout, including in its title and its filename, because it is an account of what happened under
the name it happened under. The rename's reasons are in `script/swish-check`'s own `Name:` block:
one hyphen from `shellcheck`, and `shell` no longer naming any program here.

In brief. With a virtio-rng attached, which both plain legs set unconditionally via `NIFE_RNG=1`,
the interactive boot traps in init at `user_rt::trap` with no message. The same build with the
device absent reaches a prompt normally. The cause is capability-slot exhaustion in
`crates/system_initializer`, and the reason nobody knew is that `script/shell-check` ran in neither
`script/test` nor CI.

## What was actually wrong

The reported hypothesis was the sixteen-slot wall that this file's own entropy-block comment
describes. The wall held; the location did not. Init does not run out of slots building the
entropy service. It runs out four blocks later, at
`let verify_page = must(retype_page_frame(ut))` in the login stack, building `credentialer`. The
entropy service is only the reason that block executes at all: `have_login_stack` requires
`entropy_client.is_some()`, and nothing else in the boot sets it.

Established by instrumentation rather than by reading, in four boots:

1. The kernel's fault line names `user_rt::trap`, which is one address shared by every failure in a
   program, so it identifies nothing. Making `system_initializer::fail` take a *data* abort at
   `return address + 0x1_0000_0000` put the caller in `far`, which is printed. That named
   `crates/system_initializer/src/lib.rs:1430`.
2. `MemoryRegion::RETYPE` answers `OutOfMemory` both when the region is out of pages and when the
   capability table is full. Splitting the two with a temporary `println!` said capability table
   full.
3. Dumping init's table at that moment showed all seventeen slots occupied.
4. Raising `CAPABILITY_TABLE_SLOTS` to 40 and printing the highest slot ever granted measured the
   boot's real high-water mark: **21**, in init, while `build_child` lays `credentialer` down.

## How far back it goes, and why the commit that broke it looked correct

Bisected over the 105 first-parent commits between milestone 49's login-stack wiring and `main`,
seven boots. The first red merge is **PR #556** (`a731b4ce`, milestone 49's boot terminal
attribution), landed 2026-08-28. `main` was red for five days.

The commit inside it that did the damage is `d1c81062`, *"cap: `CAPABILITY_TABLE_SLOTS` is 17, not
the 28 the bisection left behind"*. Milestone 49's lane had set the constant to
`28 // TEMP: generous bisection value` while chasing an unrelated login-suite flake, and then built
and shipped the login stack against it. The cleanup restored 17 on two true observations: the doc
comment carefully justifies 17, and a full `script/test` on all three architectures is green at 17.

Both are true and neither could see this. No suite in `script/test` boots the real init; every
test that runs a shell has the kernel play that role. The only gate that runs
`crates/system_initializer` is `script/shell-check`, and it ran nowhere. So a lane verified the
change with the most thorough thing available to it, was right about what it checked, and shipped a
system that could not boot.

## What was done

- `kernel::cap::CAPABILITY_TABLE_SLOTS` 17 -> 24, and `abi::CAPABILITY_TABLE_SLOTS` with it.
  Measured 21 plus three of headroom: at 32 bytes a slot that is 96 bytes a thread and 24 KiB across
  `MAX_THREADS`. Every previous raise took the number to exactly what that day's boot needed and
  every time the next addition hit the wall in the same silence, so the margin is the point rather
  than a rounding.
- `script/shell-check` now runs in `script/gates` and in CI, and the reasoning for each is in
  the file that carries it. It is 30 seconds for both architectures against a warm target directory,
  measured, which is why it belongs in the command a person runs before pushing. In CI it is
  appended to `script/ci-build` rather than given a job of its own. A separate job would repeat that
  job's ten-minute build and claim another runner to do thirty seconds of work, and runner
  concurrency is a real ceiling here.
- **The `BUGS` sections that described this as a mystery now describe it as a fact**, in
  `crates/system_initializer` and beside the constant in `kernel/src/cap.rs`.

## What the gate caught on its first CI run, which is the second half of this milestone

Adding `script/shell-check` to `script/ci-build` made it fail immediately on the riscv64 leg (run
33702132439), on a tree whose local runs were green on both architectures. The divergence is
real and is a better argument for the CI step than the cost argument above.

The check reported that init never dropped its construction budget. The transcript it printed
contained the line it said was missing:

```text
  user thread 17 killed: scause 0x3 (code 3)
    pc 0x0000000000406aa0   stval 0x000000000i0406aa0   usern sp 0x0000000000500da0
it:   cthe kernel is fine.
onstruction budget dropped; retype answers NoSuchSlot
```

Nothing was truncated and no byte was lost. `init: construction` is spliced through the
kernel's register line one and two characters at a time, and every byte of both writers is present
and in order. The kernel prints a user fault report with its own UART driver; the userspace
`console` server drives the same UART from its own address space; nothing arbitrates between two
processes writing one device. So a thread dying while init prints shuffles the two lines together.

This is not the partial-line problem `crates/serial_console` solved for milestone 216 (nothing in
this tree can read a board, so every hardware milestone waits on a person). That crate's
`observe_partial` is about a line the reader has not finished receiving, and its answer, `contains`
on a completed line, is exactly what fails here.
Its distinction is worth keeping and does not apply; the reader in `shell_check_leg` already
accumulates, and accumulating more would not have helped.

It also happens on every local riscv64 boot. The interleaving simply landed somewhere harmless
there, which is why local was green: whether the shuffle lands on a line the gate reads is timing,
and a runner is not this laptop.

## Two matchers lost, and what replaced betting on a matcher

The first fix tolerated interleaving under a character budget. It accepted
`construction budget NOT dropped` as `construction budget dropped`, caught by a test. The second
required the skipped text to carry the kernel's own fault-report signature. CI defeated it on
run 33707574930, where the shuffle was worse:

```text
  the kernel iis fnit: constiner.
uction budget dropped; retype answers NoSuchSlot
```

`construction budget dropped` survives with a longest run of `const`, and `the kernel is fine.` is
destroyed in the same breath, so the signature had nothing left to key on. The same commit had
passed run 33705237435 half an hour earlier, and PR #663 carrying this branch passed on the same
code. The severity is nondeterministic and no matcher wins: any string a matcher keys on can
itself be split.

So the third version does not put the safety in the matching. It puts it in which question is
asked first, on the one asymmetry a shuffle cannot break:

> Interleaving can **destroy** a string. It cannot create one.

An exact search for the sentence init prints when the answer is *no* therefore has no false
positives. `boot_claim` asks, in order:

1. `construction budget NOT dropped` present, **exactly**: fail. init printed it.
2. The affirmative present, exactly or shuffled: pass.
3. Neither, and the kernel printed a fault report during the boot phase: pass, and say on
   stderr that the line could not be read and why.
4. Neither, and nothing else was writing: fail. With one writer there is nothing to shuffle, so the
   absence is real.

The tolerance survives but is no longer load-bearing, and the signature requirement is deleted:
dropping it is what lets run 33707574930's transcript read again.

The false-red rate, which is what a CI gate has to answer for. It is not a rate, it is
structural: under this ordering an interleaving artifact cannot produce a red. A red needs either
the negative sentence found exactly (which a shuffle cannot manufacture) or a missing line with no
concurrent writer (which means nothing shuffled it). Against the four riscv64 CI legs observed, all
four read correctly, two of them through the tolerance, and none reached case 3. The residual is
that case 3's own test is six exact substring searches for short kernel strings, and a boot that
shredded all six would give a false red.

And what it costs, said where a reader meets it (`script/shell-check`'s `BUGS`): the error moves
from false red to false green. If init's report were deleted *and* a thread faulted in the same
boot, this passes. The trade is deliberate. A false red taxes every lane whose change had nothing to
do with it, which is the signature this tree has deleted three lint checks for. A false green is
undone by repetition, because an init that stops dropping its budget prints the failing sentence on
every boot of both legs on every push.

The diagnostic was the other defect and is fixed independently. It used to assert two capability
states ("it still holds the kernel's root untyped, or the delete did not take") on the evidence that
a string was missing, and sent a maintainer looking for a bug that does not exist. It now says which
sentence it wanted, how much of it survived, and nothing about a kernel it cannot see.

## Proposed milestone: two address spaces drive one UART with nothing arbitrating

*(Provisional; the number is the integrator's at merge.)*

The kernel prints its boot tour and its user-fault reports with its own UART driver. The userspace
`console` server drives the same device from its own address space. Nothing arbitrates between them,
so output from the two is nondeterministically interleaved at byte granularity whenever they
write at once. This is a defect in the system, not in the gate that found it.

What depends on that stream being readable, beyond this gate:

- Every bench session on argon, radon and xenon. Each of the three target boards has one serial
  line (`notes/target-hardware.md`), so a serial log is the only window into a board, and it can
  shuffle.
- **`crates/serial_console`** (milestone 216), whose whole job is to recognize how far a boot got
  from the text alone. Its `progress` recognizer matches on complete lines, which is the assumption
  this breaks.
- Milestone 218's boot script, which parses the same stream.

The design question is where the kernel's own output should go once userspace owns the console, and
it is not obviously a patch. A fault report has to reach a person, and on a real board there is no
second port to send it to.

## `login` does not run, and this gate is why anyone knows

The thread whose death caused the shuffle is `login`, and it dies on every interactive boot on
**both** architectures. Measured rather than deduced: instrumenting `login::fail` to fault at
`0xFA11_0000 + step` gives `far 0x00000000fa110001`, which is step 1, `nifefs::Fs::parse` refusing
the archive. `_start` reads the archive from `initrd_len` in `a1`;
`crates/system_initializer` starts it with `thread_control_block_start(login_tcb, 0, 0, 0)` and
endows it with no mapping of the archive, so it gets a zero-length slice.

The boot still prints `init: login ready` with a generated password, because init measured the
identity provisioning rather than this process's survival. **So a green `script/shell-check` does
not currently mean the login service runs**, and that is recorded in `components/src/login.rs`'s own
`BUGS` where a reader meets the program.

Not fixed here. Handing `login` the archive costs `crates/system_initializer` capability slots at
the exact peak this milestone has just measured and sized the table against, so the fix and the
accounting move together and want a lane of their own.

## BUGS

- A fault line still cannot say where a program died. The kernel prints the `pc` of the
  breakpoint inside `user_rt::trap`, which is one address per program however many callers it has.
  The return address is in `x30`/`ra`/on the stack at the moment the kernel takes the fault and is
  not printed. That is what made the first day of this milestone a day rather than a boot, and it is
  recorded in `user_rt::trap`'s own `BUGS`. Fixing it is a change to the three
  `kernel/src/arch/*/exceptions.rs` fault printers, so it wants a lane of its own.
- Nothing counts init's capability slots. *(Closed by milestone 231, nothing counts how many
  capability slots a boot actually uses so the wall is always a surprise, on the same day: every
  boot now prints `capability slots: N of M at peak` and this check reads it.)* The high-water mark
  was measured here with temporary kernel `println!`s that were reverted, and three slots of
  headroom stood in for a mechanism until 231 gave it one.
- The two graphical legs remain red behind milestone 177's (attaching a GPU and keyboard to the
  real boot) display driver bug. `script/shell-check` with no arguments is green; `--graphical` and
  `--graphical-serial` are not, and neither `script/gates` nor CI runs them.
- **The gate does not fail on a killed user thread**, which is how `login`'s death went unnoticed
  through a passing run. *(Closed by milestone 233, `login` dies on every boot and the boot says it
  is ready, which fixed the program and added exactly that assertion.)* It was left out here because
  it would have been red on both architectures until `login` was wired.
- The interleaving is coped with rather than removed, which is why it is proposed above as its
  own milestone. Everything this milestone does about it is a reader working around a stream that
  should not be corrupt.
- A boot that both stopped reporting and faulted passes. `boot_claim`'s third case reads a
  concurrent kernel write as an explanation for an unreadable line, and cannot tell that from init
  having gone silent in the same boot where something died. Both halves have to happen together.
  Recorded in `script/shell-check`'s own `BUGS`, which is where a reader meets the check. Milestone
  233 narrows it a long way without closing it. The only kernel output during the console phase is
  a fault report, and a killed thread is now a failure in its own right, so a run that reaches case 3
  is already failing for the true reason. What this design buys there is that the *reported* reason
  is the dead thread rather than a fabricated claim about init's capabilities.
- Case 3's own evidence can be shuffled too. "Was the kernel writing during the boot" is six
  exact searches for short kernel strings, and a boot that destroyed all six would give the false
  red this design otherwise rules out structurally. Six independent chances is a better bet than
  one, not a proof.
- The bisect is over first-parent merges, so it names PR #556 rather than a commit inside it.
  `d1c81062` is identified by reading that branch, not by a boot at that commit.

## Follow-on

- **Milestone 233.** Milestone 233 (`login` dies on every boot): `login` died on every interactive
  boot on both architectures, at `nifefs::Fs::parse` refusing a zero-length archive ("`login` does not run" above). Left unfixed
  here because handing it the archive costs capability slots at the peak this milestone had just
  sized. 233 fixed the program and added the assertion that a killed user thread fails the gate,
  which is the other item this block left open.
- **Milestone 231.** Milestone 231 (nothing counts how many capability slots a boot actually uses):
  nothing counted init's, and the BUGS above say how this block measured them. 231 made every boot print `capability slots: N of M at peak` and this check
  reads it.
- **Milestone 177.** `script/shell-check --graphical` and `--graphical-serial` remain red behind
  177's display driver bug, and nothing runs them (BUGS above).
- **Recorded.** `crates/user_mode_runtime/src/lib.rs` carries it in `trap`'s own BUGS: a fault line
  cannot say where a program died, because the kernel prints the `pc` of the breakpoint inside
  `user_mode_runtime::trap`,
  which is one address per program however many callers it has. The return address is in `x30`, `ra`
  or on the stack at the moment the kernel takes the fault and is not printed. The workaround that
  worked here is to fault on a *data* address derived from it, since `far` is printed.
- **Recorded.** `script/swish-check`'s own BUGS (the script this block calls `shell-check`
  throughout) says where the error moved. A boot that both stopped
  reporting and faulted passes, because `boot_claim`'s third case reads a concurrent kernel write as
  the explanation for an unreadable line and cannot tell that from init having gone silent in the
  same boot. Why that trade is deliberate is told above, under "Two matchers lost".
- **Recorded.** `script/swish-check` also carries the residual in that third case, the six exact
  searches told in this block's `BUGS`.
- **Recorded.** `design/roadmap/0230-shell-check-is-red.md`'s BUGS: the bisect names PR #556, not a
  commit inside it.
- **Milestone 342.** Decide where the kernel's
  own output goes once userspace owns the console. Today the kernel and the `console` server drive
  the same UART from two address spaces with nothing arbitrating, so the streams interleave at byte
  granularity. It corrupts every bench session on argon, radon and xenon, and 243's BUGS points at a
  home that now exists.

## Index row

init ran out of capability slots building the login stack; the table goes 17 -> 24 and the check
now runs in `script/gates` and CI
