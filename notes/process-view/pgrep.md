# pgrep, and the pkill that cannot exist

An appendix to [notes/process-view.md](../process-view.md), which states the conclusion. This is the
argument, the four answers, and why the pattern is not a name.

On Unix these are one lookup with two endings. `pgrep` prints the pids that match and `pkill`
signals them. The only difference is what each does with the answer. `kill(pid)` turns a name into
an action, so on Unix anything that can find a process can end it. That symmetry was going
to be milestone 126's headline demonstration, `caps pgrep` beside `caps pkill`, one narrow and one
wide.

A domain names its members and does not act on them (calef, 2026-08-17). That ruling arrived
before the signaling stratum was built, and mostly abolished it. The reason is one the ABI had
already made without anybody reading it back:

- a survey returns a tid, which is a name and not a capability;
- `abi::tcb` has no `DESTROY`, so there is no method that takes a tid and ends the thread;
- killing a live child is `abi::untyped::DESTROY` on the region it was built from (DECISIONS §24's
  forcible `^C`), held by whoever spawned it, which is the shell and not a monitor.

So `pkill` cannot be assembled out of a view here. Making a domain confer control would be the one
place this system copied the thing it exists to refuse.

What replaces the demonstration, said as a trade rather than a win. `caps pgrep` prints a scope
and there is no `caps pkill` to print beside it, because that program cannot exist. That is a
weaker side-by-side than was promised, and a stronger claim. The write-up owes both halves. The
cost, plainly: `procps` gets ported without its signaling stratum. A reader who expects `kill` to be
a program will not find one. Killing stays with the shell that spawned the
thing, which already holds the region.

The claim is asserted rather than argued. `kernel::user::survey_tests` builds a domain, filters it
down to the corpse with `pgrep dead`, and then asserts that the same capability that printed that
tid is refused the reap. Before `Rights::ENUMERATE` existed that assertion would have failed. The
only thing standing between a `ps` and a reap was the program's own source code.

## Two halves, again

- `crates/pgrep` is the filter: a selector, the match, the four answers, the output format. It
  does not walk, because `ps::collect` already does. A second implementation of `SURVEY`'s
  resume protocol is a second thing that can be wrong. Twelve host tests, no emulator.
- `components/src/pgrep.rs` is the syscall and two sinks. Its capability contract is `ps`'s three
  exactly: the output sink, the domain with `ENUMERATE`, the diagnostics sink. The two manifests
  in `grant_plan` are identical field for field. That sameness is the readable form of the ruling,
  not a coincidence worth deduplicating.

## The fourth answer

`ps` distinguishes three outcomes; a filter adds one, and it is the one upstream loses:

| what happened | output | diagnostics |
|---|---|---|
| members matched | the tids, one per line | silent |
| the domain has members, none matched | nothing | `pgrep: none of the 3 processes in this domain are ready` |
| the domain is empty | nothing | `pgrep: this domain holds no processes` |
| the walk was refused | nothing | `pgrep: this endpoint may be sent to, but not looked at ...` |

Upstream's `pgrep` prints nothing and exits 1 for all three of the bottom rows. So a monitor cannot
tell an idle machine from a closed door. The refusal wording is `ps::Survey::complaint`'s, reused
rather than restated. Two programs describing one refusal in two ways is a drift waiting to happen.
Splitting `write_diagnostics` at the sentence was the whole change needed to share it.

All four are asserted twice: in `crates/pgrep`'s host tests, and on a real kernel domain on both
ISAs in `kernel::user::survey_tests::a_filter_names_members_and_tells_its_four_answers_apart`.

## What the pattern matches, and why it is not a name

Upstream `pgrep`'s pattern matches a process name, and this system has none (see
[limitations.md](limitations.md): there is `arg0` in `Spawn` and no display name). So the pattern
matches the one other thing a domain honestly knows about a member, which is its run state. It is
spelled with the same four words `ps` prints in its `STATE` column. `pgrep dead` names the corpses,
`pgrep 'r*'` the ready and the running, `pgrep '*'` everything.

`crates/glob` does the matching, unchanged and unextended. It was the right tool rather than the
nearest one. It is on the verification path already, it has a `cost_bound` nothing can blow past,
and four short words is exactly the shape it is good at. There are two deliberate departures from
its defaults, both recorded at the call site. The first is `Dot::Ordinary`, because a state name
never begins with a dot and the leading-dot rule is a filename-listing convention. The second is
glob rather than upstream's extended regular expression. No regex engine exists in this tree, and
taking a dependency to match one of four words is what DECISIONS §46 (thin primitives or whole subsystems) declines.

The pattern never crosses a process boundary. It resolves to a small bitmask where the person
typed it, and the mask is what the program is handed. That is the same move `rm -r` makes, where the
recursion flag becomes *rights* on the granted capability rather than text in an argument. It is
the shape this system keeps arriving at: the boundary carries authority, not strings.

## The limitation that shaped it, which is a property of the boundary

Nothing in this system delivers bytes from a command line to a program. `Endowment::arg` is one
`u64`, and `spawnproto` carries three words. Every string-shaped designation a person types (a file,
a directory) arrives at the child as a capability. That is what `rm logs/old` and `doc
notes/glob.md` both are, and it is why `rm` learns nothing about the name it removes.

So a *pattern* is not something the prompt can hand over today, and the shipped `pgrep` is `pgrep
'*'`. It names every member, one tid per line, which is `ps` without the columns and is what pipes.
This is `Program::Date`'s deliberate under-declaration verbatim: an `ArgSpec::Forbidden` over a program
that reads registers the shell cannot set. It lifts when `ArgSpec` grows the positional arity
milestone 47 (navigation and naming) deferred. The filter itself is not waiting on that. The kernel test hands it real
selectors over a real domain, which is the only place in the tree that can.

Worth noticing rather than fixing: this is a capability system's shape showing through a Unix
program's interface, not a missing feature. A command line here is a list of designations, and a
regular expression is not a designation of anything.
