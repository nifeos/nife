---
status: NOT-STARTED
promoted_from: image-composition-records-the-grants-it-checked
raised: 2026-11-03
milestone_dependencies: 990
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1030. Image composition records the grants it checked

Raised 2026-11-03 (UTC) by the security audit
[2026-11-03-from-the-package-to-the-grant.md](../audit-reports/2026-11-03-from-the-package-to-the-grant.md),
finding 3. Severity: low (availability only). Every name below is provisional.

Reuse: `program_asks::checked_asked_grants_of`, which already exists and is what the check uses.

## The failure

`xtask/src/base_image.rs` checks each carried program's asks against the approval with
`program_asks::checked_asked_grants_of`, which reads the note from an ELF of any machine. It then
writes the first generation's grant lines with `program_asks::asked_grants`, which goes through
`elf::Elf::parse`. That parser refuses an ELF whose machine is not its own build target's, and
`asked_grants` turns the refusal into no asks. Composing an image on a host of another
architecture than the image's therefore checks the asks and then records no grants for them.

This only narrows. A program that should hold the network or a resolver zone starts without it,
and nothing reports the difference. The audit did not run a cross-architecture composition to
confirm it; the reading is from the code.

## The work

The grant lines come from the same reader as the check, so the two cannot disagree. A test
composes the first generation from a program whose machine is not the host's and asserts its grant
lines. `asked_grants` either takes the checked reader too or says in its doc that it reads only the
build host's machine.

## Index row

Composing a base image on a host of another architecture checks a program's asks and then records none. The program starts without the network or resolver zone its approval named, and nothing reports the difference.
