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

Raised 2026-10-24 (UTC) by lane `milestone/946-granted-file-name-longer-than-sixteen-bytes`, which
let a granted file's name be 255 bytes and left every name that rides two `START` words at 16.

## The limit

§307 (how long a name may be) rule 5 holds `jig install` to a package name of at most 16 bytes, so
nothing is installed that `jig remove` cannot remove. `jig remove <package>` and the shell's
`vouch <file>` send their name to the progenitor in one message, packed into two words by
`filesystem_contract::grant::pack_name`, whose bound is `grant::WORDS_NAME`. Milestone 946 made
`pack_name` refuse a longer name rather than truncate it, and did not widen the message. §307's rule
1 bounds a package or program name at 252 bytes, so the activation messages are the last place a
16-byte bound stands between a package and its install.

## What moves with it

- `spawnproto`'s Remove and Vouch messages carry a name longer than two words: more messages, as
  `spawnproto::holder_from_words` already reads a 32-byte holder from two, or a frame, as an image's
  bytes travel. The shape is an architect's call, because the shell, `jig` and the progenitor read it.
- `jig remove` and rule 5's check in `jig install` read the new bound; `grant_plan::vouched_name`
  and `user_verb` stop at it.
- `system_initializer::activation`'s `named` buffer.

Reuse: none exists; this is nife's own activation protocol between `jig`, the shell and the
progenitor.

## Exit

On aarch64, riscv64 and x86_64: a package whose name is over 16 bytes installs with `jig install`
and is removed with `jig remove`, and `vouch` records a file whose name is over 16 bytes.
