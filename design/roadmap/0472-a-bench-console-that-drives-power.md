---
status: REFUSED
raised: 2026-09-20
refused_by: 216, 448
---
# 472. A bench console that can power-cycle a board

Refused by milestone 216 (design/roadmap/0216-board-console.md), which records it in its Follow-on.
Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names work a
number, a status and a condition that would change it.

## The refusal, in its own words

From '216. Nothing in this tree can read a board, so every hardware milestone waits on a person',
under `## Follow-on`:

> Making the tool drive power. A tool that power-cycles is a different and more dangerous object
> than one that reads, and the outlet next to the board's on that strip feeds an external drive
> that must never be switched off. The built tool reads and never writes, to the port or to the
> outlet, so the question stayed undecided rather than being settled by an implementation.
>
> -- design/roadmap/0216-board-console.md

## Why it is here rather than only there

`board_console` reads a board and never writes, to the port or to the outlet. Power is the obvious
next verb and it is a different and more dangerous object: the outlet next to radon's on that strip
feeds an external drive that must never be switched off, and a tool that can switch an outlet can
switch that one.

## Revisit

- **Unstated.** The refusal deliberately leaves the question undecided rather than settling it by
  implementation, and it names a hazard rather than a condition. It is an architect's call, and what
  a reader needs before it can be made is not a trigger but an answer about whether a reading tool
  should ever hold a writing authority. The hazard is on the record here so that it cannot be
  rediscovered the expensive way.

## Index row

The tool reads and never writes, on purpose, and the reason is a specific outlet on a specific strip
that feeds a drive nobody can afford to switch off. The question of whether it should ever drive
power was left open rather than answered by shipping, and this keeps it open where it can be found.
