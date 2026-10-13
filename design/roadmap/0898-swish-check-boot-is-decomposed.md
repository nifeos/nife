---
status: NOT-STARTED
raised: 2026-10-12
promoted_from: swish-check-boot-is-decomposed
milestone_dependencies: 847
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 898. `swish_check_boot` is decomposed into its phases

*(The title and slug are provisional.)*

`swish_check_boot`, in `xtask/src/swish_check/boot.rs` since milestone 847 (the swish-check harness
is split along its seams), is 1,051 lines in one body.

The body already has phases a reader can see. It builds the images and the disk. It chooses the
devices each leg's runner attaches. It starts QEMU and the transcript reader. It types each line
and waits for its echo and prompt. Then it reads the transcript's verdicts. Each phase could be a
function, with the state they share made a struct.

This is a refactor, not a move. Only `script/swish-check` on all three architectures proves it,
which is why milestone 847 left it alone. Milestone 843 (the system-initializer crate root is split
along its seams) gives the same reason for its own `boot`.

Reuse: not applicable; this restructures one function.

Done when no function in `boot.rs` is over 300 lines. `script/swish-check` must pass on aarch64,
riscv64 and x86_64, and both falsification records under `boot` must still turn their lines red.

## Index row

`swish_check_boot`, 1,051 lines in one body, becomes one function per phase, with no function in
`boot.rs` over 300 lines and `script/swish-check` passing on all three architectures.
