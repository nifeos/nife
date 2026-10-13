# What the `swish` language's guest test proves, on both ISAs

An appendix to [`notes/swish-language.md`](../swish-language.md). It holds the wiring the test
shares with `redirection_tests`, the memory finding that made it share rather than add a role, and
the four pairs of assertions.

## The wiring

`system_tests::user::language_tests` reads the tail of the same run of the same script
`redirection_tests` reads, in `system_tests/src/user/language_tests.rs`. The script runs the real shell binary
with a terminal, a spawn channel, a result channel and a budget. Its directory is narrowed by an
`fs_subtree_caretaker` to one subtree of the real RedoxFS image. Both halves of the milestone need
that wiring. A quoted name is only worth something if it reaches a filesystem, and a `&&` needs a
command that can succeed.

## Why it shares the witness

It shares the witness rather than wiring its own, because of memory. Not a seventh role,
`ROLE_LANGUAGE`, with the identical endowment: every scripted shell in this suite is a live process
whose frames nothing reclaims, and a seventh put `time_tests` over the frame pool *intermittently*. Two consecutive runs of unchanged code differed,
one green and one dying with `refused to load a user program: Unmappable(OutOfFrames)` and then a lost-wakeup
watchdog sixty seconds later. The wiring the new lines needed was already there, so a second copy of
it buys nothing but the flake. The transcript buffer is 8 KiB to hold the longer script, which is
kernel `.bss` and costs no frames at all.

The lesson generalizes past this milestone: a scripted-shell witness is not free, and the price is
paid by whatever test runs last. A new claim about the shell should look for a witness whose
endowment already matches before it asks for a role of its own.

## The four pairs

The assertions are pairs, which is `redirection_tests`'s shape and for the same reason. A single
line proving "it printed something" would pass on a shell that ignored quoting entirely.

- `echo "*.txt"` against `echo *.txt`: the same four characters, quoted and not. The quoted one must
  print itself and the bare one must not, so a shell where quoting did nothing fails both halves.
- `least_authority_demo 3 && echo yes` against `least_authority_demo && echo yes`: one connector against a left-hand side that
  was refused. `least_authority_demo 3` runs and `least_authority_demo` alone is refused for the integer its manifest requires,
  so the condition table is covered by two real commands rather than by a branch written for a test.
- `wc "my notes.txt"` against `wc < "my notes.txt"`: the same designation said two ways, whose byte
  counts are derived from the `echo` that wrote the file rather than written down.
- `echo $?` after a command that ran and after one that was refused: `0` and `2`.
