# Two proofs in `kernel/src/arch/x86_64/interrupt_controller.rs` (milestone 304)

An appendix to [`notes/kernel-proofs.md`](../kernel-proofs.md), which states what is proved. These
two came from milestone 304 (`cargo kani -p kernel` only ever compiled one architecture).

The first properties this tree has ever checked on `arch/x86_64/`, and they exist because of a
runner label rather than because of anything clever. Until milestone 304 every Kani job in this
repository ran on an aarch64 host, so `arch/x86_64/` was never compiled. Both are about numbers
firmware chose, which is the strongest case here for a bounded model checker: a vector is an index
into the IDT, so getting one wrong does not misroute an interrupt, it runs the page-fault handler.

- `no_vector_belongs_to_two_bands`. `MSI_VECTOR_BASE`'s doc claims the three vector bands are
  "disjoint by construction rather than by anybody remembering", the construction being
  `MAX_REDIRECTION_ENTRIES` clamping the entry count. This is that claim, over every entry count an
  eight-bit version register can report and every state the MSI bump counter can be in, asked
  through the two predicates the trap handler actually calls rather than restated from the
  constants.
- `an_owned_gsi_routes_inside_the_io_apic_band`. For every GSI `redirection_index` admits, the
  flat map lands inside `GSI_VECTOR_BASE..MSI_VECTOR_BASE`. Milestone 255's
  `no_stream_can_reach_another_streams_tables` one architecture over: a firmware-supplied
  identifier, one bounds check, a write that cannot be taken back.

The second one found a defect, and the harness carries the finding as an assumption. Without
`kani::assume(base == 0)` it fails: a second IO APIC owning global interrupts from 200, with the 24
entries every real part has, admits GSI 210, and `gsi_vector(210)` wraps onto vector 2, the NMI.
`MAX_REDIRECTION_ENTRIES`' doc says this cannot happen and bounds the wrong quantity. Not fixed
here, because the fix costs `gsi_vector` its `const fn` and its total signature: see `interrupt_controller.rs`'s
module `BUGS` and
`design/roadmap/proposals/the-gsi-vector-map-wraps-on-a-second-io-apic.md`.

Falsified twice, on cordoba, and the asymmetry is the honest half. Raising
`MAX_REDIRECTION_ENTRIES` by one turns both red; loosening `redirection_index`'s bound from
`index < entries` to `index <= entries` turns only the second red, correctly, because the first
never calls the guard. Both are recorded `attested` rather than `replayable`, because
`script/falsifications --sweep` runs a named harness on whatever host it is on and an x86_64 harness
does not exist on an aarch64 one. That script's `BUGS` now records the same fact from the sweep's
side.
