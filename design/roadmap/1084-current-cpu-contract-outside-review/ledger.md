# The ledger

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

| item | state |
|---|---|
| 1, 2 | left: the move's (manifest and files) |
| 3 | left: the documentation rewrite is M and feeds the README; the `BUGS` part of item 11 is done |
| 4 | left: decision B |
| 5 | done (decision A) |
| 6 | done: `allowance` reads an allowance the target cannot hold as unknown (`usize::try_from`) |
| 7 | done: `word` reaches its load through `base.add(off)`, which keeps provenance |
| 8 | the alignment `debug_assert!` on `publish` and `new` is done; the two on `cpu` are left, because the crate's own tests publish `UNSCHEDULED` and an id past the bound to prove the reader refuses them |
| 9 | left: a published name and signature, decision B |
| 10 | left: two harnesses and their patches |
| 11 | done: the `BUGS` list is the five facts a stranger can act on, with no machine, pull request or section |
| 12 | the `BUGS` sentence is done; the farm check that refuses a copied package with more than one source file is left, a nife lane of its own |
| 13 | done: `#![no_std]` |
| 14 | left: recorded for the name review |
| 15 | done: the kernel's safety comment says the store stays inside the first 16 bytes |
| 16 | done: the crate's note links the kernel-tests note |
