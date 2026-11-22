---
status: PROPOSED
raised: 2026-11-22
milestone_dependencies: none
decision_dependencies: 324
machine_requirements: none
specific_machine: none
needs_person: no
---
# A message's words are arrays and every decoded value is named by its meaning

Raised 2026-11-22 (UTC) by lane `lane/message-word-names`, after section 324 (the names of a
message's words and a system call's result words) ruled the rule.

Reuse: the trap boundary already passes `words: [u64; 6]`; three `const fn` contract decoders already
take arrays. Name: provisional. No new dependency.

## The problem

The tree still names message words `w0` to `w5` and result words `r0` to `r2`, and the system call
wrappers take and return separate words. Section 324 rules an array of words, a signed first result
word in a tuple, and names by meaning for every decoded value.

## What to build

- `send`, `call`, `receive` and `Message` take and hold `words` arrays; `call` returns
  `(i64, u64)` and `reply` takes `(i64, u64)`. About 113 `let (..) = call(` bindings and 112
  `reply(` calls in 43 files change, plus the 197 group sites and 122 single-word sites.
- The thin wrappers carry `#[inline]`. Check that a release build's assembly for a caller before and
  after is identical, on aarch64, riscv64 and x86_64, and keep that check so the attribute cannot
  be dropped unseen.
- `byte_sink_contract` changes first in its own repository and nife bumps its pin; regenerate and
  replay the 16 falsification patches; check that the Kani harnesses and mutation baselines do not
  name the renamed parameters.
- Prose and notes/abi.md use `words[i]` and `results[i]` as notation; `w0`..`w5` and `r0`..`r2`
  join the `[spelled_out]` list.

One lane, in commits by area, after the in-flight renames of the same files land.
