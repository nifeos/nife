---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# `script/board-console` types a named command at the shell prompt

Raised by lane `maintainer/radon-bench-2026-11-17-tree-findings`, from the 2026-11-17 (UTC) radon
bench session on nifeos/nife#3509. Name: provisional.

Reuse: `--stop`, the one named mode that already sends a byte, and its rule that every byte sent is
printed into the log.

## The problem

`script/board-console` sends exactly one byte, in `--stop`, and says it is not a keyboard on
purpose. Step 5 of milestone 1062 (the device control planes leave the kernel) needs `reboot`
typed at the shell prompt of a booted board, twice. The bench session used a small script
outside the tree, so nobody can repeat the procedure from the tree, and a hand-rolled
sender risks the hazard the narrowness exists to prevent. A byte sent during U-Boot's autoboot
countdown stops the boot.

## The work

- A named mode, for example `--type <command>`, that waits until the kernel has printed its shell
  prompt, sends the command and a newline, and logs every byte it sent, as `--stop` does.
- It refuses to send before the prompt has been seen, so the autoboot countdown is never hit, and
  it refuses a command containing a control character.
- The standing rule that the capture shows every byte sent holds (milestone 324 (the bench console
  cannot speak to any of the three boards), part 4). The header's "this is not a keyboard" sentence
  then says what each mode sends. That wording changes a stated invariant, so an architect approves it.
- A host test over a recorded capture and a simulated port, in the style of the existing replay
  tests.

## Done when

`script/board-console --type reboot` on a booted board sends `reboot` once, after the prompt, and
the log shows the bytes sent. A run before the prompt sends nothing and exits 3.

## Index row

`script/board-console` types a named command at the shell prompt.
