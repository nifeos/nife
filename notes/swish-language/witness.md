# The shell's witness scripts

[`components/src/swish/witness.rs`](../../components/src/swish/witness.rs) holds the test roles
`system_tests` spawns the shell in. Milestone 885 (the shell's test roles become scripts the shipping
shell runs) replaces them. This page carries the design each script's doc comment links to.

Every script drives the real shell. Its lines go through `dispatch` or `builtin`, the functions the
prompt itself calls, so what is under test is the shell and not a reimplementation of it. The lines
are chosen so the answers check each other rather than constants.

## The piping script

Everything it prints goes over the terminal contract to whoever holds the other end, which in the
guest test is the kernel. The transcript is the assertion. `date` and `date | wc` are the same binary
sent to two destinations. The second one's byte count has to be the first one's length, or the claim
that a program cannot tell its destinations apart is false.

## The redirecting script

It has the piping wiring plus one filesystem capability. That one capability is the whole difference
between a `>` that is refused and a `>` that writes a file.

- `ls > out.txt` writes a listing into a file. The `ls` after it prints the same listing, since
  nothing between them changes the directory, and `wc < out.txt` has to agree with what was printed.
  One builtin, two destinations.
- `wc out.txt` is the line above it with the `<` left out, and it has to give the same three
  numbers. Naming a file to a program that declares an input is the operator, unwritten.
- `wc out.txt | wc` is the same designation inside a pipeline. The planner resolves the operand, and
  the pipeline wires the head's input off the plan. A head wired off the line would get an empty
  input slot, where a `receive` answers `NoSuchSlot` instead of blocking, and count an empty stream.
- `date` and `date > date.txt` are one unmodified program sent two places, and `wc < date.txt` has
  to report the length of what `date` printed.
- The truncate and append files are the same two commands with one operator changed. So what `>>`
  does is measured against what `>` does, not against a constant. `>>` on a missing name creates it,
  because there is nothing to open and seek.

The quoting lines write a name with a space in it and read it back. Without quoting, the `>` would
write to a file called `"my`.

### One witness, not a seventh

Milestone 67's grammar lines (quoting, sequencing and exit status) run on this same shell rather than
a new role. That is a memory decision, not a filing one. Every scripted shell in the suite is a live
process whose frames are never reclaimed. One more put `time_tests` over the frame pool
intermittently, failing with `refused to load a user program: Unmappable(OutOfPageFrames)`. The lines
need exactly this wiring, so a second copy of it buys nothing but that failure.

## The timing script

It has the piping wiring plus at most one capability, the clock page. That page is the whole
difference between a `time` that reports a duration and one that refuses. The same script runs
against three clock states: a published page, a blank page and no page at all. So the refusals come
from a capability table, not from a branch anybody wrote for the test.

- `least_authority_demo 3` and `time least_authority_demo 3` are the same command run twice, so the
  answer has to be the same. That is the claim "what you time is what you run", made where it can
  fail. `least_authority_demo` declares no clock and is handed none: the thing timed needs no
  authority to be timed.
- `time echo hello` times a builtin, which spawns nothing. The duration is real and no process is
  in it.
- `time` alone is the third refusal, which is about the line and not about a clock.

## The navigating script

The shell is told nothing about which subtree it was rooted in, beyond a run index. The index keeps
the names it creates distinct across runs that share one image. It tries to name one file that
exists only in `sub` and one that exists only in `other`, and reports which it reached. So "two
shells with different roots cannot name each other's files" is read off the pair of reports, not
claimed by either. The two runs are each other's control.

Some steps carry a reason the code cannot show.

- `cd /` after `cd <made>` must be a directory this shell made. The two subtrees are not the same
  shape: only one ships a child directory, so a shipped one would prove nothing for the other run.
- The unlink-while-held step sends the verb `rm` sends, at the far end of the same caretaker chain,
  rather than typing a command. `rm` is a program, and this witness holds no spawn channel: nothing
  in its capability table names a progenitor. `components/src/rm.rs`'s own guest test covers the
  program.
- The removal steps hold up one safety property: no single call on the contract takes a subtree
  away. Emptying one is a deliberate second step, bottom-up, as `rm -r` does it.
- The `bind` step names a position the shell already reached, then reaches the real files through
  the bound name: a listing, an ascent to the real parent, and a refusal at the true root.

## The two-tree script

The wiring is `fs_service::start_granted_two_dirs`. The first tree is the fixture's `sub` at slot 0,
the second its sibling `other` at slot 1, and a report rendezvous sits at slot 2. The mount point,
`fixture::twotrees::MOUNT`, is the witness's own, as the slot order is. Nothing tells an interactive
shell where a second tree is mounted yet; that transport arrives with the first real second
filesystem. The `<` probe goes through `grant_plan::redirect_target` and `open_source`, the
pipeline's own path. What is under test is the shell's resolver, walk and grant plumbing.

## The globbing script

It shows that `echo *.txt` prints exactly the authority `rm *.txt` would transfer. Both halves run in
a shell holding a real directory capability behind a real `fs_subtree_caretaker`. They share the
expander, which is the point of there being one, and do not share the plumbing. `echo` goes from
text to words, expansion and print. The grant goes from parse to positionals, expansion,
`grant_plan::plan`, the grant's names and a rendering. So a planner that narrowed, reordered or added
to a set shows up as a disagreement.

Three refusals stop the agreement from being vacuous the other way. One is a pattern that matched
nothing. Another is a pattern where one cannot mean anything, in a leading path component. The last
is a line with no pattern in it at all, which must come back byte for byte.
