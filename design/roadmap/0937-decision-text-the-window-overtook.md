---
status: NOT-STARTED
promoted_from: decision-text-the-window-overtook
raised: 2026-10-19
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 937. Decision text the window's own rulings overtook

Raised by the 2026-10-19 documentation sweep
([decisions read forward](../audit-reports/2026-10-19-decisions-read-forward.md), finding 13).
A lane does not edit `design/decisions/`, so these go to the integrator. Each is a statement about
the tree that was true, or nearly, when its section was ruled, and that a later commit or ruling
moved. None changes a ruling.

- [§303 (the retention declaration says nothing about the region)](../decisions/0303-the-retention-declaration-says-nothing-about-the-region.md)
  says `Retention` keeps exactly two variants and that the TCB is inert after `START`. §309 (a
  supervised job's `ThreadControlBlock` goes to the shell that asked for it) added
  `Retention::Requester`, and the `SUSPEND` and `RESUME` of §300 (the paused state, a TCB verb, silent) act on a started thread.
- [§308 (the installer asks the terminal to swap)](../decisions/0308-the-installer-asks-the-terminal-to-swap.md)
  sizes the endpoints against a measured capability peak of 34 and 30 free slots.
  `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` is 37, so 27 are free; the conclusion holds.
- [§307 (how long a name may be)](../decisions/0307-how-long-a-name-may-be.md) cites
  `xtask/basalt-pins.index`, which is gone; the pin is `xtask/basalt-pin`. It also names
  `filesystem_protocol` and `system_log_protocol`, now `filesystem_contract` and
  `system_log_contract`, with the constants it quotes unchanged.
- [§306 (the web's authority list is a package and a capability)](../decisions/0306-the-web-authority-list-is-a-package-and-a-capability.md)
  points at an "open question 1" of milestone 809 (the package client becomes a program); 809 has
  no such list, and the gap it means is in
  809's follow-on.
- [§304 (who may turn a device off)](../decisions/0304-who-may-turn-a-device-off.md) quotes the
  clock-and-reset crate in American spelling; the crate has the British one. Fixing the crate to
  the house spelling makes the quote exact.
- §303 and §304 carry `decided: 2026-10-16` in their frontmatter, while milestones 356 and 376 and
  the caveat of §40 (a supervisor's death is its subtree's death) date the rulings 2026-10-15. Which UTC date is right wants the ruling's timestamp
  before anyone edits either side.

Reuse: considered `script/decisions`, which checks citations resolve but not that a section's claims about code are true; an edit to six records needs no code.

## Index row

Six statements in §303 to §308 about the tree were true when ruled and later commits moved them: a variant count, a capability peak, renamed crates and paths, a missing citation, a spelling and two dates. The edits are the integrator's, and none changes a ruling.
