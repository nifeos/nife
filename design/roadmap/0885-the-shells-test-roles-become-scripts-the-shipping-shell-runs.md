---
status: NOT-STARTED
raised: 2026-10-11
milestone_dependencies: 844
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 885. The shell's test roles become scripts the shipping shell runs

*(The title and slug are provisional.)*

calef ruled on 2026-10-11 (UTC), on pull request #1914 (milestone 844's cut), "Proceed with both".
The second of the two was this, in his words: "It seems like we shouldn't need the test roles in
swish. Couldn't we design a better implementation that the witnesses are individual programs and we
run them all with scripts rather than a specific build?"

Classification: new work. The lane claims with `--new-work`, because what it leads with is a user
feature the shell does not have: running a script from a file. Deleting the roles is what the
feature pays for, not the purpose.

## What calef ruled (2026-10-11, UTC)

Relayed one question at a time by the maintainer, each with its prior art and alternatives.

- **Where a script comes from: (d), a fourth option this block did not list.** The interactive
  shell already reads every line from its terminal endpoint (`line_editor::proto`'s
  `OPERATION_READLINE`), a userspace protocol the kernel only routes. So the harness serves that
  endpoint from a script: each `READLINE` gets the next line, the end is end-of-file, and what the
  shell writes is the transcript. The shell gains no script mode and no probe; the witnesses run
  the shipping binary at its own prompt, as `script/swish-check` already does over the serial
  console. The user feature is a `.` builtin (POSIX's spelling, name provisional): the running shell
  reads a file through its own directory capability and hands each line to `dispatch_line`, with no
  new process and no new authority. Refused:
  - (a) the input slot. For tests it is a probe only the harness fills. For users, `swish build.sw`
    starts a second shell that must spawn. The spawn endpoint stays at the boot prompt under
    §221 (the boot prompt is the owner's console) and §270 (a package manager holds an installer
    endpoint, not the spawn endpoint).
  - (b) `-c`, for the same reason, and it caps a script at one argument page.
  - (c) the shell opening a path. That is the `.` builtin under another name, and the fixture trees
    hold no script.

  A child shell that runs a file waits for its own spawn-authority decision.
- **Two trees: A, wait for milestone 660.** Role 6 and `fixture::twotrees` stay until 660 deletes
  them, about 250 lines. §234 (the prompt shows one tree) lands the transport "with the first real
  second filesystem". A probe only the harness fills would be a test hook by another name, and it
  would fix a one-slot shape before the deferred mount-name rules shape the real one.
- **Exit status** needs no ruling: under (d) the shell never exits, and the harness judges the
  transcript and its `echo $?` lines. No status word, so no syscall surface change.
- **Removing `navscape` and `globscape`** is cleanup, not a ruling: their only readers are the
  witnesses and the three test files that move off them.
- **Names** wait for calef's naming ratification pass, marked provisional until then.

Reuse: the shell's own `dispatch_line`, which is the thing under test and is reused unchanged.
`system_tests/src/user/pipeline_service.rs` already wires a scripted shell and reads its transcript
(`start_with`, `transcript`, `answer`, `counts`); every witness moves onto that wiring.
`xtask/src/swish_check.rs` is the shape for the assertions: a table of lines and the substrings each
must answer. `fs_service::start_granted_dir` and `start_granted_two_dirs` keep serving the trees.
The prior art for the mode is POSIX `sh`, which reads commands from a file operand or from a
standard input that is not a terminal and stops at its end. New code is needed only for the mode
itself, the few hundred lines that read a script instead of a keyboard, because nothing that exists
can run swish's own dispatch except swish.

## What is there today

`components/src/swish.rs` (5,920 lines on main at `9532eeb53`) carries six roles that no user can
reach. `_start(role, arg, clock)` dispatches on `role`:

| role | name | launched by | wiring | how it reports |
|---|---|---|---|---|
| 1 | `ROLE_NAVIGATE` | `shell_navigation_tests` (2 tests) | a subtree at slot 0, a report rendezvous at 1, no terminal, no budget | a bitmap, `fixture::navscape` |
| 2 | `ROLE_GLOB` | `glob_grant_tests` (1 test) | the same, over `globset` | a bitmap, `fixture::globscape` |
| 3 | `ROLE_PIPELINE` | `pipeline_tests` (5) | terminal, spawn, result, budget | a transcript ending `== pipelines done` |
| 4 | `ROLE_REDIRECT` | `redirection_tests` (6), `language_tests` (6) | role 3's plus a directory | a transcript ending `== redirections done` |
| 5 | `ROLE_TIMING` | `time_tests` (2) | role 3's plus a clock page or none | a transcript ending `== timings done` |
| 6 | `ROLE_TWO_TREES` | `multi_dir_namespace_tests` (1) | two trees at slots 0 and 1, a report at 2, no terminal | a bitmap, `fixture::twotrees` |

Role 0, and any number not in the table, is the interactive shell. `crates/system_initializer`
starts it as `(0, fs_rights, clock_slot)`. The witness code is about 1,090 lines, measured by
pull request #1914's proposed decision (the shell program becomes a directory of modules), which
moves it into a temporary `witness.rs`.

Milestone 844 (the shell program is split along its seams) said the witnesses "cannot move to a
fixture binary" because they need the shell's own `Nav` and spawn path. That is right, and it is
why this milestone moves them into scripts rather than into a second program: a script is run by
the shipping binary, through the shipping dispatch.

## What gets built

1. A `.` builtin in swish that reads a file through the shell's directory capability and runs each
   line through `dispatch_line`, as if typed. This is the user feature.
2. A terminal server in the kernel harness that answers the shell's `READLINE` from a script and
   records what it writes. The lane first checks whether `edit_line` reads raw keystrokes; if so,
   the harness feeds the real line discipline as a keyboard driver would, rather than answering
   `READLINE` itself. Each witness becomes a script file in the tree (location and extension
   provisional, for example `system_tests/scripts/navigate.sw`), included in the test build.
3. Each test judges the transcript, the way `pipeline_tests` already does, plus whatever the harness
   can read from the tree afterwards through `fs_service`.
4. Five roles (all but role 6, which waits for 660), their helpers (`run_line`, `pwd_is`, `Text`, `opened`, `opened_token`, `created`,
   `mtime_of`, `open_dir`, the `touch` probes, `run_name`, `line`, `bind_line`, `listing_is`), the
   witness-only `Nav` constructors, and `witness.rs` itself are deleted.
5. The verdict bitmaps `fixture::navscape` and `globscape`, with their host test rows, go once
   nothing reads them. `twotrees` stays with role 6.

## How each witness keeps its property

The property every witness holds is the same: what is under test is the shell's own resolver and
dispatch, not a copy. A script keeps that by construction, because the only path a script line has
is the prompt's. What changes is who judges, and for three witnesses, what they run in.

**Pipeline, redirection and timing (roles 3, 4, 5).** These are scripts already: fixed arrays of
lines through `dispatch_line`, printed with `$ ` and judged from the transcript. The arrays move
verbatim into script files, and each ends with an `echo` of its done line. The three clock states
stay three capability tables, which is what makes `time`'s refusals assertions. Nothing about the
witnessed property changes.

**Navigation (role 1).** The shell-level steps become lines: `pwd`, `cd ..`, `cd /..`, `ls`,
`cd deeper`, `mkdir`, `cd <made>` then `cd /`, `touch`, `touch -t`, `bind`, and `cd` through the
bound name. A refusal is read off its sentence and off `echo $?`, which prints 2 for a refusal
(notes/swish-language.md). Whether a name was reached becomes `wc < inner` and `wc < /inner`, which
resolve through the shell's own `redirect_target`, so the wiring gains the interactive endowment
(terminal, spawn, result, budget) beside its subtree. The two runs with different roots stay
each other's control, and the harness puts the run index into the names it hands each script.

Several steps never were the shell. `CREATE` with a body, `UNLINK` while a handle is held,
`UNLINK` of a directory, `RMDIR` of a non-empty directory, `GETMTIME`, and `MKDIR` narrowed to drop
`SETTIME` all go straight down the contract through `Nav::name_call`. The code says why: `rm` is a
program, and the witness holds no spawn channel. Those assertions move to
`fs_test_client`, which already carries `ROLE_DIR_ATTACKER` for exactly this kind of probe behind
the same caretaker, or to the harness reading the tree after the script. The mtime checks become
the harness reading `GETMTIME` after the `touch` lines. The `../motd` probe becomes a line the
shell's resolver refuses.

**Globbing (role 2), the known hard case.** The witness compares `echo gl-*.txt` with the planner's
verdict for `rm gl-*.txt`, rendered through `swish::write_set`. The planner's verdict is already
printable: `caps rm gl-*.txt` plans through `grant_plan::plan_stage` (which `plan` calls) and
prints the grant's names with the same `write_set`, in the row "(the directory holding ...)". The
crate's own comment there says `caps rm *.txt` prints exactly what `echo *.txt` prints. So the
script is `echo gl-*.txt`, `caps rm gl-*.txt`, the no-match pattern and `echo $?`, `caps rm
*/gone`, and the plain line, and the harness compares the two renderings. The lane checks two
things first. One is that `caps` resolves a bare `rm` without a spawn channel; it falls through to
the archive's manifest when `bare` finds no installed program. The other is that the row's wording
is stable enough to parse. The second phase, running the real `rm` with the set grant, is the harness's already and is
unchanged.

**Two trees (role 6).** The script is short. It moves with `pwd`, `ls`, `ls /`, `cd /second`,
`cd ..`, `cd ../..` and `cd /..`. It reads with `wc < secret`, `wc < /inner`, `wc < /secret` and
`wc < /second/secret`. It ends with `rm /second/secret`, both `bind` lines, `ls /bee`, `cd` and
`pwd`. Which tree
answered becomes which body `wc` counts, since the two fixture files differ in length. The
blocker is not the script. The shipping shell cannot hold a second tree: the witness builds one
with `Nav::rooted_twice` from a mount constant only it knows. Milestone 660 (a second
filesystem mounts in the boot shell) chose the transport: a named slot probed at `_start`, and a
mount path in a shared crate. calef said it is built "with the first real second filesystem, not
before".
This is open question 2.

## What `_start` keeps

`_start(role, arg, clock)` keeps `arg` (the directory rights the shell is told) and `clock` (the
clock slot), because both are told for reasons the code records. `role` is no longer read. The
system initializer passes 0 there and can keep doing so; changing its three-word spawn convention
is outside this milestone. Every wiring now grants a budget, so the branch that skips `heap_init`
for roles 1 and 2 goes. Until 660 deletes role 6, `_start` reads `role` for that one branch and
nothing else. There is no script mode to choose: a scripted shell is the interactive shell with the
harness as its terminal, so no role dispatch comes back under another name.

## What has to be decided first

Nothing. calef answered the open questions on 2026-10-11 (UTC); see "What calef ruled" above. The
lane records the `.` builtin's semantics in `notes/swish-language.md` and marks its names
provisional.

## Risks the lane measures

- Memory. Every scripted shell is a live process the kernel never reclaims, and one more once put
  `time_tests` over the frame pool (`pipeline_service.rs`, the `TRANSCRIPT` comment). Three
  witnesses gaining a 128-page budget and a terminal costs frames. Measure the pool before and
  after. A scripted shell does not exit at end of input (it says it has nowhere to exit to and
waits), so the remedy is the harness ending it, not a bigger pool.
- Stack. Roles 1 and 2 run on 4 and 7 measured pages. The script wirings take the interactive
  `SHELL_EXTRA_STACK` of 11, which must stay level with the boot's.
- A transcript is prose. A reworded sentence fails a test. That is the intent, and the failure
  names the line.

## Done when

1. `witness.rs` holds only role 6 and what it alone uses, and no other role constant or witness
   helper remains in swish. Milestone 660 deletes the rest.
2. `_start` has no dispatch beyond the interactive shell and role 6.
3. A user can run a file with `.` at the prompt, with `EXAMPLES` and a `BUGS` section in
   notes/swish-language.md.
4. Every assertion the five moved witnesses made still runs on aarch64, riscv64 and x86_64, DECISIONS §19
   (architectural parity is a tenet). Each one moved to `fs_test_client` or the harness is listed in
   the pull request beside the bit it replaced, so a reader can check none was dropped.
5. `user.shell_navigation_tests.two_shells_with_different_roots_cannot_name_each_others_files.patch`
   still fails the test, rerun on the new script, and its record says where it now fails. It is the
   only falsification patch on these tests.
6. `script/test` passes on all three architectures, and so does `script/swish-check`.
7. Every new name carries a `Name:` line marked provisional.

## Index row

The shipping shell carries six test-only roles, about 1,090 lines that only tests reach. Five
become scripts the harness types at the shipping shell by serving its terminal, so tests exercise
the real prompt with no hook compiled in, and users get a `.` builtin that runs a file. The glob
witness reads the planner through `caps`. The two-tree witness stays until milestone 660, per
calef's ruling. Comes after 844.
