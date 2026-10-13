# The confinement claims' limitations, in full

An appendix to [notes/confinement-claims.md](../confinement-claims.md), whose BUGS section keeps the
short list. This is every entry in full.

- Rows 19, 22, 23, 24 and 30 carry a record on all three ISAs, row 19 on one of four kernel tests.
  Milestone 323 (the falsification record is incomplete in five ways) replayed the four older ones on
  riscv64 and `x86_64` on 2026-10-03 (UTC).
- A kernel row's evidence is re-checked far less often than a harness row's, and on one
  architecture. A Kani record costs a second, so `script/falsifications --affected-since` re-checks it
  on every pull request that can reach it. A kernel record costs a boot per leg. So it is re-checked
  only by a full `--sweep`, which nobody runs per commit, and only on the architectures its patch
  names. Both limits are in `script/falsifications`' own `BUGS`. Read the "Falsified" column
  accordingly: a yes on a kernel row is a machine-replayable fact that nothing replays on a schedule.
- This table is a floor, and its own worst failure is invisible. It cannot list the claim nobody
  made. Every row here was found by reading what this project already wrote, so the enumeration
  inherits exactly the blind spots the tests have. The `BUGS` of §31 (the foreign-language seam) and `design/fatal-risks/README.md`
  both say the decisive experiment is adversarial and by somebody else. This is not that, and calef's
  position gates outside eyes behind milestone 198 (a package manager).
- A recorded falsification proves the harness catches *that* defect, not the class. The
  `the_view_and_the_reap_have_the_same_scope` record carries a prediction that was measured false.
  Its defect was claimed to be caught by that harness alone, and it also reaches
  `reap_is_permitted_only_to_the_supervising_rendezvous`. Both the prediction and its correction are
  in the patch, which is the point of writing the prediction down.
- Nothing gates which assertion a row's evidence comes through, and milestone 307 (which assertion
  actually fires when a confinement claim is broken) is a manual sweep rather than a mechanism.
  `script/falsifications` checks that a recorded defect turns the harness red. It cannot check that
  the red came through the assertion the patch's prose predicts, which is its own `BUGS`' standing
  entry. And it has nothing at all to say about an assertion that is *unreachable while the harness
  is green*. Row 12's survivor was found by reading the encoder beside the assertion and then
  breaking a constant on purpose. Nine rows' worth of that reading is recorded in
  [which-assertion-fires.md](which-assertion-fires.md), and it will rot the moment somebody rewrites
  one of these harnesses. Read the verdicts as dated 2026-09-16.
- An assertion that is unreachable because a guard above it is correct becomes reachable the day the
  guard is wrong. So "cannot run" is not "delete it". Rows 4 and 15 are left exactly as they are for
  this reason. The unreachable assertion is the claim in the claim's own words, and it costs nothing
  but a line. Where 307 did remove such a line (rows 13, 14, 18), it was because the assertion was a
  *restatement of the guard itself*, so no defect anywhere can separate them. The distinction is worth
  keeping: one is redundancy, the other is decoration.
- The sweep read every assertion and broke exactly one. Reading is how all nine unreachable
  assertions were found, and it is cheap. Breaking is the only thing that can find a survivor, and it
  costs a solver run or a boot per defect. Row 12 was broken because the reading predicted a
  tautology, and a prediction about a proof is worth confirming. The other 25 rows' verdicts are
  reasoned from the code, not measured. Milestone 305's own headline is the standing warning about
  what that is worth: `user_can_read` had been readable for four weeks.
- Rows 27 to 29 were added by an audit, and their verdicts are dated 2026-09-17. Milestone 313
  (userspace confinement, read adversarially) found the tree's newest device object, the `x86_64`
  `PortRange` of milestone 299 (the x86 port-range capability), absent from this table. Its two tests
  were unable to go red in the direction they exist for. A wrongly permitted `out` was followed by a
  `SEND` nobody received, so the escape hung the run: row 26's shape, one object over. And a third
  property, self-deletion, was false in the tree. The fixtures were reshaped so an escape exits and
  arrives as `EVENT_EXIT` where the test wants `EVENT_FAULT`. All three rows carry a record replayed
  on `x86_64`. Row 27's record also names the defect that did not fire. The hand-off is protected
  twice (the bitmap bits and the `iomap_base` word), and only a defect that defeats both turns the
  test red. The audit is
  [design/audit-reports/2026-09-17-userspace-confinement.md](../../design/audit-reports/2026-09-17-userspace-confinement.md).
- Row 27 tested the hand-off on one `x86_64` boot only, from 2026-09-23 to 2026-10-03 (UTC). Its two
  children could run on different cores, each with its own port bitmap. Its record has the
  measurement and fix.
- The rows citing kernel tests are still not evidence at the same grade as the rows citing
  harnesses. A machine can replay a kernel falsification (milestone 305 (the six kernel confinement
  rows get a falsification a machine can replay)), so the gap is about what the replay *proves*. A Kani harness is checked by a solver over every input in its bound. A kernel test
  is one boot of one machine with one fixture attached, on one architecture. A kernel row that skipped
  for want of a disk or a device page is not evidence at all. That is why `--sweep` reports a skipped
  test as an error rather than as either color.
