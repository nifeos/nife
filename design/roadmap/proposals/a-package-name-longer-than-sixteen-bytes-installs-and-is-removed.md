---
status: PROPOSED
raised: 2026-10-24
milestone_dependencies: 946
decision_dependencies: 307
machine_requirements: none
specific_machine: none
needs_person: no
---
# A package name longer than sixteen bytes installs and is removed

*(Title and slug provisional.)*

Raised 2026-10-24 (UTC) by lane `milestone/946-granted-file-name-longer-than-sixteen-bytes`. That
lane let a granted file's name be 255 bytes. Every name that rides two `START` words stayed at 16.

## The limit

§307 (how long a name may be) rule 5 holds `jig install` to a package name of at most 16 bytes. So
nothing is installed that `jig remove` cannot remove. `jig remove <package>` and the shell's
`vouch <file>` send their name to the progenitor in one message. It is packed into two words by
`filesystem_contract::grant::pack_name`, whose bound is `grant::WORDS_NAME`. Milestone 946 (a
granted file name can be longer than sixteen bytes) made `pack_name` refuse a longer name. It did
not widen the message. §307's rule 1 bounds a package or program name at 252 bytes. The activation
messages are the last 16-byte bound between such a package and its install.

## What moves with it

- The Remove and Vouch messages carry a longer name. That is more messages, as
  `spawnproto::holder_from_words` reads a 32-byte holder from two, or a frame, as an image travels.
  The shape is an architect's call, because the shell, `jig` and the progenitor all read it.
- `jig remove` and rule 5's check in `jig install` read the new bound. So do
  `grant_plan::vouched_name` and `user_verb`.
- `system_initializer::activation`'s `named` buffer grows with it.

Reuse: none exists; this is nife's own activation protocol between `jig`, the shell and the
progenitor.

## Exit

On aarch64, riscv64 and x86_64: a package whose name is over 16 bytes installs with `jig install`
and is removed with `jig remove`. `vouch` records a file whose name is over 16 bytes.
