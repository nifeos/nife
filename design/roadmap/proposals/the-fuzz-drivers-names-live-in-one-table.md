---
status: PROPOSED
raised: 2026-11-22
milestone_dependencies: none
decision_dependencies: 311
machine_requirements: none
specific_machine: none
needs_person: no
---
# The fuzz drivers' names live in one table that nife and the filer both read

Raised 2026-11-22 (UTC) by lane `lane/fuzz-filer-system-call-variable`, after the continuous fuzz
host's filer refused every new kernel-thread finding.

Reuse: the `windows.tsv` row format that `helpers/continuous-fuzz.sh` writes and `fuzz-filer` reads,
which is already a table two repositories agree on; `option_env!`, which accepts a macro that
expands to a string literal; the public, credential-free GitHub reads that
`helpers/fuzz-follow-main.py` already makes. No new dependency.

Name: provisional. Every file and check name below is the lane's, not a ruling.

## The problem

Each in-guest fuzz driver has two public names: the build variable that sets its seed range and the
test filter that selects it. nife writes them in four places that nothing compares. The driver's
`option_env!` and its replay line in `system_tests/src/user/`, `driver_test` in
`helpers/continuous-fuzz.sh`, and the selftests. nifeos/fuzz-host writes them a fifth time, as the
`DRIVERS` table in `bin/fuzz-filer`, which checks a held log's replay line against it and builds each
advisory's reproducer from it.

nife's system call rename (#3855) changed the kernel-thread pair from `NIFE_SYSCALL_FUZZ_SEEDS` and
`seeded_syscalls` to `NIFE_SYSTEM_CALL_FUZZ_SEEDS` and `seeded_system_calls`. Every nife gate passed,
because nife's copies moved together. The filer's copy did not, and from the host's first window on
the new commit it refused every held kernel-thread finding as naming another driver's variable. Only
the filer's exit status showed it. This is the duplicated agreement §311 (a contract is code, not
prose) forbids, held across two repositories.

## The work

1. **One table, owned by nife.** `fuzz/drivers.tsv` holds one row per spelling: driver, seed
   variable, test filter, and a state, `current` or `retired`. Each driver has one `current` row. A
   rename turns the old row `retired` and adds a new `current` one, and the retired row stays while
   any held log may name it, because the held directory keeps logs from before a rename.
2. **nife reads it instead of repeating it.** `helpers/continuous-fuzz.sh` takes `driver_test` from
   the `current` rows. The drivers take the variable from a macro generated from the table, so
   `option_env!` and the replay line's format string cannot disagree with it. The selftests read the
   table for their fixture lines.
3. **A nife check holds the rest.** A `script/lint` section refuses a table with no `current` row
   for a driver or two for one, a `current` filter that names no test in `system_tests`, and any
   `NIFE_*_FUZZ_SEEDS` spelling in the tree that the table does not hold.
4. **The filer reads the same table.** fuzz-host keeps a byte copy at `drivers.tsv` beside a file
   naming the nife commit it came from, and `fuzz-filer` builds `DRIVERS` from it. A replay line
   whose variable matches any row of its driver passes, and the reproducer uses that row's filter. A variable
   in no row gets a refusal that names it.
5. **The order of a rename is enforced.** A nife pull-request check fetches fuzz-host's `drivers.tsv`
   from its main branch, with no credential, and refuses a nife table holding a row that copy lacks.
   A rename therefore lands in fuzz-host first, as an added row, and in nife second. fuzz-host's own
   CI runs a daily job that compares its copy with nife main's and opens an issue when nife holds a
   row it lacks, for a change that reached main some other way.

## Prior art

syzkaller writes each system call once, in its `sys/<os>/*.txt` descriptions, and `syz-sysgen`
generates both the manager's Go tables and the executor's C tables from them, so the two halves
cannot disagree on a name. Protocol Buffers do the same for a wire format across languages. nife
differs in scale: two names per driver do not earn a generator, so the shared artifact is a table
both sides parse, and a cross-repository check stands in for the single build syzkaller has.

For example, renaming the confined driver's variable to `NIFE_CONFINED_SYSTEM_CALL_FUZZ_SEEDS` would
be two pull requests. fuzz-host adds the row first. nife then marks the old row `retired`, adds the
new one, and the drivers, the host script and the filer's copy all agree before any window runs.

## Open questions

- Step 5 makes a nife check read another repository's main over the network. The stricter answer is
  that a failed fetch fails the check after retries, so a rename cannot slip through on an outage.
  That blocks unrelated pull requests during a GitHub outage, so the check could run only when the
  table or a driver changes. This lane recommends both: fail closed, and run only on those paths.
- The table could instead be a small crate that fuzz-host reads through a generated file. A
  tab-separated table is enough for five columns, and the filer is Python, so this proposal takes the
  table.

## What this does not do

It does not decide when a `retired` row may go. That waits on the filer telling the host which held
logs remain, which milestone 1016 (the clean-range table does not locate a held seed) already
touches.
