---
status: IN-PROGRESS
raised: 2026-10-20
branch: milestone/946-granted-file-name-longer-than-sixteen-bytes
promoted_from: a-granted-file-name-longer-than-sixteen-bytes
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 946. A granted file name can be longer than sixteen bytes

*(Title and slug provisional.)*

Raised 2026-10-20 (UTC) by `maintainer/record-2026-10-20`, recording calef's ruling of that day on
milestone 922 (the demos are packaged in basalt). He kept the ratified `least_authority_demo` rather
than shortening it: *"B"*. The fix is to lift the limit that refuses the name. calef asked on
2026-10-24 (UTC) for it to be built; the number 946 was assigned that day at promotion.

## The limit

`filesystem_contract::grant::MAX_NAME` is 16 bytes. A per-file grant carries its name by value, packed
into the two argument words the file's caretaker is started with (`grant::pack_name`), so a grant
costs no extra page and the caretaker needs nothing mapped before it runs. The constant's own
comment calls it a demonstrator's limit, not a filesystem's, and says lifting it means giving the
caretaker a frame.

The shell refuses anything longer at the prompt. `grant_plan::MAX_FILE_NAME` mirrors the constant,
because the shell's parser must not depend on the filesystem contract. `grant_plan`'s tests pin the
pair (`tests.rs`, and `expand::MAX_NAME` in `planning/tests/operands.rs`). The refusal reads *"that is
not a name: one component, at most 16 bytes"*.

## Who meets it

Anyone with a file whose name is over 16 bytes. A customer's files are not named for this limit:
`quarterly-report.txt` is 20 bytes and cannot be granted to a program at the prompt. Milestone 922 met
it with its own program: an installed `least_authority_demo` (20 bytes) cannot be run by its package
path.

## What moves with it

- The caretaker's start: the name moves from two argument words to a frame the caretaker is given,
  which is a change to the wiring and not to the contract's verbs.
- `filesystem_contract::nameset::BYTES` is `MAX_NAMES * (1 + grant::MAX_NAME) + 1`, and a set travels
  by value. `MAX_NAMES` is 8 because sixteen names of 16 bytes overflowed the shell's stack while
  planning one grant, through four frames a debug build does not collapse. A longer name changes that
  measurement, so the set's size has to be measured again rather than scaled.
- `grant_plan::MAX_FILE_NAME` and `expand::MAX_NAME`, with the tests that pin them.
- The new bound itself: 255 bytes would match `xattr::MAX_NAME` and most filesystems. That number is
  an architect's call, because the filesystem and the loader share it.

Reuse: none exists; this is nife's own grant wiring between the shell and a caretaker, the crates
named above, and no library carries a capability's name by argument words or frame.

## Exit

On aarch64, riscv64 and x86_64: a file named with more than 16 bytes is granted at the prompt and read
by the program it was granted to, and an installed `least_authority_demo` runs by its package path.

## Index row

A customer's files are not named for a demonstrator's limit: a name like `quarterly-report.txt`
can be granted at the prompt and read by the program it was granted to, and a package's program
named over sixteen bytes runs by its path.
