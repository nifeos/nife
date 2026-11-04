---
status: BUILT
raised: 2026-09-16
built: 2026-09-16
---
# 305. The six kernel confinement rows get a falsification a machine can replay

## The finding this started from is that the blocker had already been removed

`design/fatal-risks/README.md`'s risk 7, the claim that the confinement claim is false, says in the entry a
reader meets it in: *"Six kernel confinement rows still have no mechanism at all."*
`notes/confinement-claims.md`'s `BUGS` explained why, and the explanation was stale on the day it
was read:

> Automating it needs a way to run one kernel test by name, which does not exist:
> `kernel/src/testing.rs`'s runner takes no filter, `cargo xtask test` parses only `--arch`, `--cpu`
> and `--hvf`...

Milestone 210 (no kernel test can be run by name) built that on 2026-08-31. `cargo xtask test --test <substring>` filters the kernel
suite; the filter is baked into the test binary by `kernel/build.rs` and read by the runner. 210's
own `BUGS` even names the three things a kernel sweep would still need and says it did not build
them. Nobody connected the two records for a fortnight, and `script/lint` reported the consequence
out loud on every run without anybody acting on it: *"`kernel/falsifications/user.c_seam_tests...`
falsifies something that is not a Kani harness; nothing sweeps it."*

This was not a gap in anybody's knowledge. Both facts were in-tree, in files that cite each
other. What was missing was the one edit that makes a
tool read both.

## What was built

`script/falsifications` learns a second thing to read a `Falsification:` block above (a kernel
`#[test_case]`, not only a `#[kani::proof]`) and a second verb to replay it with (`cargo xtask test
--arch <a> --test <fn>`, not only `cargo kani --harness`). Nothing about §134 changes: the three
states, the block's spelling and the patch path `<package>/falsifications/<module.path>.<fn>.patch`
all carry over. The evidence that they carry over cleanly is that milestone 202's existing patch
filename already obeys the convention with no special case.

Kernel tests are opt-in where harnesses are mandatory. `--check` fails a harness with no block,
because all 145 of them should say which of the three states they are in. There are 312
`#[test_case]`s and almost none is a confinement claim, so the same rule there would buy a wall of
`unfalsified`. The both-directions patch check is what keeps opt-in honest: a patch under a package's
`falsifications/` must be claimed by a harness or by a test, and an unclaimed one is still rot.

Correction, 2026-10-04 (UTC), calef's ruling: a `#[test_case]` newly added under
`system_tests/src/user/` must carry a block (`unfalsified` allowed). Existing tests stay opt-in.
Milestone 749 (a new confinement test carries a falsification record) built it.

Ten kernel tests now carry a record and nine are `replayable`, up from one that nothing could
replay. `notes/confinement-claims.md`'s rows 21 to 26 are the six the risk-7 entry named; they are
nine tests between them, because two of those rows cite more than one. Eight of the nine carry a
patch. The ninth is row 26 and it is `unfalsified` for a reason given below.

## What the sweep found, which is the part worth reading

### A confinement test that could not fail, and had not been able to since milestone 41

The first sweep came back with one SURVIVOR, and it is the result this milestone is for.

Row 21's RISC-V twin, `the_page_tables_say_u_mode_cannot_read_the_kernels_memory`, was patched to
remove the `U`-bit check from `mmu::user_can_read` entirely, which is as complete a break of that
test's stated property as can be written. The test ran and passed.

The defect was not weak. `user_can_read` walked through `translate_user`, and `translate_at` builds
its `Mapper` with `Half::Low`, always, so the high-half kernel address the test asks about came back
`None` before any leaf was read. `assert!(!mmu::user_can_read(kernel_va), "the page tables say
U-mode could read the kernel's own memory")` answered "no" by refusing to look. It could not fail.

The tree had already written down the cause and nothing connected it.
`is_mapped_in_current_space`, forty lines away in the same file, exists for exactly this case and
says so in its own doc comment: *"a user thread reaching for the kernel's memory names a high-half
address ... `translate_user` alone would say 'not mapped' for it and turn the most interesting case
into the wrong answer."* `user_can_read` went on calling `translate_user`.

Fixed here (`translate_in_either_half`, `kernel/src/arch/riscv64/mmu.rs`), measured both ways on
riscv64: green with the fix, and red under the patch in `the_page_tables_say_u_mode_cannot_read_the_kernels_memory` in
`system_tests/src/user/riscv_virtio_tests.rs` with the test's
own sentence, *"the page tables say U-mode could read the kernel's own memory."*

Two things follow and both are larger than this row. A green confinement test is consistent with
its assertion being *unable* to fail, which is `notes/confinement-claims.md`'s opening sentence
arriving from a direction nobody had checked. The note says a passing test is consistent with the
component never reaching the address. This is a third world where the *test* never reaches the
question. And nothing but a falsification could have found it: every gate in this tree was green
throughout, for four weeks, because a vacuous assertion is a passing assertion.

### The §31 shape recurs, and the second instance is row 24

Milestone 202's headline finding was that §31's leading sentence, the witness-page equality, is
reachable only by an escape that faults anyway, so the assertion the claim is quoted for is not the
one that catches a broken confinement. Row 24 is the same shape in a different subsystem.
`two_shells_with_different_roots_cannot_name_each_others_files` states its property twice: once as an
exact verdict bitmap per shell, and once as the crossing, `assert_eq!((a & nb::REACHED_SECRET, b &
nb::REACHED_INNER), (0, 0), "a shell named a file in the other shell's root")`. The second is the
sentence the milestone makes and the one a reader would quote. It is below both per-shell checks
and it cannot run: any defect that causes a crossing sets a forbidden bit in one of the reports,
and `assert_report`'s first direction catches that one call earlier. No patch tried here made the
crossing fire, and none can. The quotable sentence is documentation; the bitmap equalities are the
mechanism.

Here the hazard costs nothing: `assert_report`'s messages name the offending or missing bit, so a reader learns as much as the
crossing would have told them. §31's instance cost the reader a 234-second watchdog timeout. Same
shape, benign outcome, and the thing to carry is the rule rather than the alarm: in a test that
states its property twice, the readable statement is usually the unreachable one. Two instances,
found the same way, neither visible without breaking the claim.

Row 24's own record is weaker than the row looks, and the patch says so. The recorded defect
turns the test red through `assert_report`'s *second* direction at `system_tests/src/user/shell_navigation_tests.rs`,
the vacuity guard: with the caretaker serving the filesystem root, the shell could no longer reach
its own files, so *"every refusal it reported proves nothing"*. Nothing crossed. It proves the test
is wired to the real root handle; it does not demonstrate a crossing.

### Row 26 cannot be falsified as written, and the reason is the syscall rather than the test

`a_client_of_the_stable_rendezvous_cannot_become_its_server` got the honest defect: delete the
kernel's `Rights::READ` check on `RECV_CAP`, so a client really can receive on the stable
rendezvous. The run came back as a 60-second watchdog reading "no progress ... a lost-wakeup
hang", with a thread dump and not one word about impersonation.

The reason is structural. `RECV_CAP` is a blocking receive, so an attacker the kernel fails to
refuse does not come back and report an escape; it takes the message the honest server was waiting
for and everything blocks. The assertion that states the claim is reachable only when the kernel
*does* refuse.

So the row is `unfalsified`, on purpose. The easier defect, changing which error the refusal
returns, does fire the assertion but leaves the claim intact. That is evidence of nothing, and
would put a false claim in the record. Row 17's disposition,
one rung over.

Proposed milestone (provisional, the integrator mints the number): give the usurper a bounded
wait, so an escape is reported rather than waited out. It is the same move milestone 202 (every confinement test is a ritual until somebody breaks the confinement) made for
§31 (the foreign-language seam) when its break surfaced as a 234-second timeout. It needs a non-blocking or timed receive (see
Follow-on). Until it exists, row 26 has a test and no
evidence that the test can fail.

### A red for the wrong reason, caught and swapped rather than recorded

Row 23's first patch was the faithful one: the claim is the kernel's, so break
`CapabilityTable::delete` and watch the progenitor rebuild. It turns the test red, and it turns it
red in `system_tests/src/user/authority_tests.rs`, inside the `run_tree` helper, on *"the supervision tree could not be
built: stage 3"*, because a `delete` that deletes nothing breaks the tree's construction long before
the progenitor gets to the part where it drops anything. Right answer, useless diagnostic; milestone
202's hazard exactly.

The recorded patch is the fixture edit instead (`root_supervisor` keeps the untyped it is supposed to
give away), which fails in `the_root_supervisor_drops_its_construction_authority_and_cannot_build_again` in
`system_tests/src/user/authority_tests.rs` on the test's own sentence. The refused patch
is in the record with its measurement, so the better-looking defect is not rediscovered.

### A defect is not always expressible on every ISA, and row 21 is where that bites

Row 21 says *"A user program cannot read a kernel address, on every ISA"* and names three tests, one
of them RISC-V's. The aarch64 falsification is one flag at one call site: map the kernel's `.rodata`
with `Flags::user_rodata()` and EL0 may read it, while the kernel goes on reading it too because
this kernel never sets `PSTATE.PAN`.

The same defect cannot be booted on RISC-V. `crates/paging`'s Sv39 encoder turns `CAP_USER` into
the `U` bit, and S-mode access to a `U` page faults unless `sstatus.SUM` is set, which this kernel
sets only inside a test helper. A kernel that marked its own memory user-readable there would not be
a kernel userspace can read; it would be a kernel that cannot read itself, dead before the first
test. So RISC-V's twin is falsified at the software walk instead (`user_can_read` stops reading the
`U` bit), which the test's own doc comment names as the thing under test on that ISA. The record
says so rather than implying parity it does not have.

DECISIONS §19 (architectural parity is a tenet) makes parity a gate for the capability; this is a gap in the evidence, not in the
capability. It is recorded on row 21 and in `script/falsifications`' `BUGS`.

## What it costs, measured on patagonia

| Run | Wall clock |
|---|---|
| One filtered kernel test, aarch64, cold worktree | 1 min 08 s |
| One filtered kernel test, riscv64, warm | 9.9 s |
| `script/falsifications --sweep`, 41 records (39 Kani + 2 kernel) | 1 min 53 s |
| `script/falsifications --sweep`, 48 records (39 Kani + 9 kernel), warm | 2 min 55 s |
| The same 48, on a tree where each patch forced a rebuild | about 22 min |

The spread between the last two rows is the number to carry, and it is not the mechanism. A kernel falsification is cheap when the tree it reverts to is already built, and
expensive when the patch touches a crate the kernel and the user programs both depend on. Then
each apply and each revert costs the archive, the standard-library exerciser and five disk
images. Milestone 210 measured QEMU start to `running N tests` at 0.50 s and a warm filtered run at
8.6 s; the riscv64 row above is that number confirmed independently. So the boot was never the
cost.

Two consequences are recorded in `script/falsifications`' `BUGS`. `--affected-since`, the per-PR
half, deliberately covers only the Kani records, because its whole argument is that a falsification
runs one harness and a kernel boot per record would spend minutes on every pull request touching
`kernel/`. And `--count` stays Kani-only so `script/lint`'s drift check against `script/metrics`
keeps comparing two derivations of one fact; that check fired on the first run here, correctly, and
`helpers/rust_source.py` was fixed rather than the check suppressed.

## BUGS

- A kernel record is swept on one architecture, the one its patch's `Architecture:` line names.
  Defaulted to `aarch64`. Row 21's riscv64 record is the only one that is not, and the reason it is
  not is in that patch rather than here.
- `Architecture:` is a provisional spelling, this lane's. It is a field in a patch's prose head
  beside `Falsifies`, which is where the sweep reads it, and an architect names things.
- The sweep checks a red's shape, not its sentence. It requires the kernel to have booted and
  selected exactly one test before a non-zero exit counts as red, which is what keeps a patch that
  does not compile from reading as a successful falsification. It cannot check that the failure came
  through the assertion the patch's prose predicts, so it prints the panic line and leaves that to a
  reader. Milestone 202's 234-second watchdog timeout is the failure this still leaves on the table.
- A skipped test is reported as an error and it is the likeliest flake here. Rows 24 and 26 skip
  when no RedoxFS disk or no device page is attached, and a skip means the falsification was never
  attempted. Calling it green would be the manufactured fact milestone 214's skip accounting exists
  to refuse.
- Eight patches is not six claims. Rows 21 to 26 are six claims and nine tests, and a
  falsification is per test. A row whose tests are falsified one at a time is stronger than a row
  with one patch and weaker than a row where every assertion in every test has been shown to fire.
- Row 25 is one of its four directions. The compositor test proves the input-slot refusal, the
  write fault, the witness digests and the third client's read of the screen; the patch reaches the
  write fault, and the other three are downstream of it and never run. The row reads "yes, one of
  its four" for that reason.
- Row 23's evidence is about the test, not about the kernel. The recorded patch is a fixture
  edit, so it proves the test notices when the drop does not happen. That the *kernel* is what makes
  the drop stick is rows 4 and 5's evidence, not this row's. The refused kernel-side patch and its
  measurement are in the patch file.
- One defect was found in a test and fixed here, which is a lane doing two things.
  `user_can_read` on riscv64 was corrected in this milestone rather than left as a proposal, because
  the alternative was recording a row as `unfalsified` when the reason was a two-line bug in a
  test-only function with no production caller. It is a kernel edit in a falsification milestone and
  it is called out here so a reviewer does not have to find it in the diff.

## Follow-on

- **Milestone 417.** A bounded wait for the usurper in `kernel::user::live_swap_tests`, so that a client
  the kernel fails to refuse reports an escape instead of taking the honest server's message and
  hanging the run. Row 26 of `notes/confinement-claims.md` is `unfalsified` for exactly this reason,
  measured here as a 60-second lost-wakeup watchdog. It needs a non-blocking or timed `RECV_CAP`,
  which is the syscall surface and therefore calef's.
- **Recorded.** In `script/falsifications`' own `BUGS`: a kernel record costs about a minute rather
  than a second, so `--sweep` is no longer a thirty-second command. `--affected-since`, the per-PR
  half, covers only the Kani records. A kernel record is swept on the one architecture its patch
  names. The sweep checks a red's shape rather than the assertion its prose predicts.
- **Recorded.** In `notes/confinement-claims.md`'s `BUGS`, beside the table: a kernel row's evidence
  is re-checked far less often than a harness row's and on one architecture, so a **yes** there is a
  machine-replayable fact that nothing replays on a schedule.
- **Done.** `script/lint`'s standing complaint that
  `kernel/falsifications/user.c_seam_tests.a_c_out_of_bounds_write...patch` "falsifies something that
  is not a Kani harness; nothing sweeps it". Something sweeps it.
- **Done.** `user_can_read` and `user_can_write` on riscv64, which answered by refusing to look at a
  high-half address, so `the_page_tables_say_u_mode_cannot_read_the_kernels_memory` could not fail.
  Fixed by `translate_in_either_half`; checked both ways on riscv64 (green with the fix, red in
  `the_page_tables_say_u_mode_cannot_read_the_kernels_memory` in `system_tests/src/user/riscv_virtio_tests.rs` under the patch).
- **Recorded.** In this block's own `BUGS`, because there is nowhere nearer a reader:
  `design/fatal-risks/README.md` risk 7 says "Six kernel confinement rows still have no mechanism at all",
  which is now false. A lane may not edit `design/` outside its own block and nothing gates that
  sentence (`script/fatal-risks` checks a cited milestone's status, and risk 7 cites neither 210 nor
  this one). The edit is the integrator's and is named in the pull request too.

## Index row

Milestone 210 built `cargo xtask test --test <substring>` on 2026-08-31 and nothing connected it to the six kernel confinement rows in `notes/confinement-claims.md` that it unblocked, whose `BUGS` went on saying the mechanism "does not exist" for a fortnight while `script/lint` reported the consequence on every run. `script/falsifications` now reads a `Falsification:` block above a `#[test_case]` and replays it by booting one architecture, so rows 21 to 26 carry eight replayable patches instead of six "no"s. **The finding worth the milestone is a survivor**: row 21's RISC-V twin stayed green under a patch that removed the `U`-bit check entirely, because `user_can_read` walked through a `Half::Low` mapper and answered "U-mode cannot read the kernel" by refusing to look at a high-half address. That assertion had been unable to fail since milestone 41, every gate was green throughout, and only a falsification could have found it; the walk is fixed here and the test now goes red on its own sentence.
