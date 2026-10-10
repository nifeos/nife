---
status: NOT-STARTED
raised: 2026-10-10
milestone_dependencies: 835
decision_dependencies: 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 868. relibc's seed comes under the unsafe gates

*(Minted 2026-10-10 (UTC) by lane `milestone/835-a-c-library-stage-one-files-clock-and-memory`
from calef's ruling on #1896 the same day; the number is provisional until the merge queue lands
it. The title and slug are drafts. Debt paydown.)*

calef ruled Q4 on #1896 (2026-10-10 UTC) as option G1. The C library's seed in `vendor/relibc/`
comes under the unsafe census and the `unsafe fn` contract check, in a lane of its own. #1896
merged with a dated exception in `vendor/README.md` naming this milestone.

## The debt

Milestone 835 (a C library, stage 1: files, clock and memory) seeded relibc's OS-neutral code into
`vendor/relibc/`. Every gate in `script/lint` and `helpers/rust_source.py` skips that directory as
somebody else's code. That was right for upstream's bytes. It is wrong for code nife now owns
(§265 (a C library started from relibc, whose Rust platform layer holds the capabilities)). About
580 seeded `unsafe fn`s carry no `# Safety` section, and the unsafe census does not count the
seed's blocks.

## What it builds

- The seed's `unsafe fn`s each gain a `# Safety` section stating what the caller must hold. Most
  are C entry points whose contract is POSIX's ("`s` is a NUL-terminated string"), which is the
  text to write.
- The `unsafe fn` contract check and the unsafe census read `vendor/relibc/` (the exclusions in
  `script/lint` and `helpers/rust_source.py` narrow from `vendor` to `vendor/redoxfs`), with the
  census's ceilings raised once, in the same commit, by the measured count and its reason.
- The `vendor/README.md` exception is removed.

Reuse: the existing checks; nothing new is written. The `# Safety` sections are prose.

## Done when

`script/lint` passes with `vendor/relibc/` inside both checks, and `vendor/README.md` carries no
exception for it.

## Index row

The C library's relibc seed under the unsafe census and the `unsafe fn` contract check: about 580
`# Safety` sections, and the exception milestone 835 recorded comes out.
