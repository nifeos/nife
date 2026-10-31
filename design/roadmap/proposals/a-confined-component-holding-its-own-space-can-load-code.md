---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# A confined component holding its own space can load code

The lane for milestone 969 (a `MAP_INTO` refuses a frame mapped twice with different flags) raised
this on 2026-10-31 (UTC) while it read the mapping paths for
[`the-rulings-the-alias-refusal-waits-on`](the-rulings-the-alias-refusal-waits-on.md). The lane read it and did not boot it.

Reuse: the existing rights model; no dependency. Searched `design/roadmap/` for an execute right
and found none.

`AddressSpace::MAP_INTO` with `MAP_CODE` needs `WRITE` on the space and `READ` on the frame, and
nothing else. A component holding `WRITE` on its own running space (§249 (a running address space
stays nameable)) and a budget that retypes `READ | WRITE` frames can therefore run code it wrote. It
maps a frame `MAP_RW`, writes instructions, unmaps it with `UNMAP`, and maps it `MAP_CODE`. No page is
ever writable and executable at once, so claim 11 holds, and milestone 969's refusal does not touch
this sequence.

Two shipped, confined components hold that authority today. `system_initializer::children` gives
`net_stack` a `SPEND_ONLY` budget, which is every right but `GRANT`, and `WRITE` on its own space,
so `CLOSE` can take a socket's page back. It gives `domain_name_resolver` a budget with every
right, and the same space right. A memory-safety bug in either becomes arbitrary new code,
not only a reuse of the code it shipped with.

Prior art, from memory. PaX `MPROTECT` refused executing a page that had been writable. SELinux
`execmem` refuses making anonymous memory executable. Fuchsia gates execution on a handle right,
`ZX_RIGHT_EXECUTE`, minted only through a separate resource, and Capsicum gates it on
`CAP_MMAP_X` on the descriptor. Each makes "may introduce code" an authority of its own.

The question for an architect, not answered here: whether introducing code is a right of its own in
nife, separate from `WRITE` on a space. Options a lane would cost out include an execute right on
`PageFrame` capabilities that a retype never mints from a leaf budget, or an execute right on the
address-space capability that the components above do not receive. Either one changes the rights
model, which makes it a syscall-surface ruling. The tree has no claim that states this property, so
a ruling either way also decides whether the claims table gains a row.
