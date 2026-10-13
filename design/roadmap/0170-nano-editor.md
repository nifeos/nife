---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: 809, 801
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 170. `nano`: a real, full-featured screen editor on the primitive milestone 169 builds

This is the follow-on to [milestone 169 (`kilo`)](0169-kilo-editor.md), from the same dependency
review, sequenced after it rather than in parallel. calef, 2026-08-25: "mint kilo instead, nano as a
follow-on."

Sequenced behind milestone 169 in practice, not by a hard gate: nano needs the
identical raw-keystroke input primitive 169 exists to build, and starting nano before that primitive
is designed would mean designing it twice, once against a 1,000-line program and once against a
25,000-line one, with no guarantee the two designs agree.

## What nano adds beyond `kilo`, once the terminal gap is closed

Checked against nano's own source shape (`src/files.c`, `src/text.c`, `src/prompt.c`,
`src/rcfile.c`), not folklore:

- **The same raw-terminal input and ANSI-passthrough output milestone 169 builds**, at a larger
  feature surface (status line, help bar, search-and-replace prompts, multiple buffers) but no new
  class of primitive.
- More file-persistence surface: swap files for crash recovery, an optional backup-on-save copy,
  and (in some configurations) lock files. Each is ordinary file I/O against whatever capability the
  program holds for its target directory, not a new mechanism, just more of milestone 169's file work.
- An optional, skippable subprocess dependency: the external spell-checker and `execute command`
  (`^T`) shell out to another program. Core editing does not need this; a first cut can refuse or omit
  the feature and note the gap, the same way milestone 164 named the `aes`/SSE gap rather than
  silently working around it.
- UTF-8 and multibyte handling. Believed containable as byte/codepoint arithmetic without a full
  system locale subsystem, but not verified in depth; whoever builds this should check nano's actual
  locale calls before assuming.

## Why a separate milestone rather than folding into 169

`kilo` and `nano` solve different problems: 169 is about proving the raw-input primitive exists and
works at all, at the smallest honest scale. 170 is about whether that primitive, once built, actually
carries a real, widely-used program's full feature set, including the parts (subprocess, richer
persistence) that 169 never has to answer. Keeping them separate means 169 can ship and be useful
(a working, small editor) without waiting on 170's larger surface, matching this tree's own
sequencing convention for the milestone-158 renames: finish one thing completely before starting the
next, rather than partially touching both.

## What this does not decide

Whether the optional spell-check/`execute command` subprocess feature ships at all in a first cut, or
is recorded as a `BUGS`-section limitation the way milestone 164 recorded `fs_server`'s x86_64 gap.
Left for whoever picks this up, once milestone 169's primitive exists to build against.

## Done when

calef, 2026-10-12 (UTC), under §297 (software reaches a nife system as a package `jig` installs
from basalt's index, the kernel included): "They should all be installed via jig through basalt."

`nano` must run on nife installed by `jig install` from basalt's package index. basalt builds the
package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). No work toward that starts
until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block start
on packaging.").

## Index row

Sequenced as milestone 169's follow-on rather than run in parallel: nano needs the identical
raw-input primitive at roughly 25x the code size, plus an optional, skippable subprocess
dependency (spell-check, `execute command`) `kilo` never has to answer.
