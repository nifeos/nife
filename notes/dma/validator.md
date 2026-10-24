# The validator crate, and what its harnesses cover

An appendix to [notes/dma.md](../dma.md), whose "The validator" section says what
`crates/direct_memory_access_validator` does and why it owns the ring layout. This file holds the
reasoning behind its predicate and the coverage argument of each Kani harness, which the crate's doc
comments link here for.

## One predicate for both directions

`is_in_region` is the whole confinement as one predicate, and it does not know which way the data
flows. The device reads a transmit buffer and writes a receive buffer, and either way the address it
touches must be in the driver's region. A receive buffer aimed at the kernel would let the device
overwrite it. A transmit buffer aimed there would let it put the kernel on the wire. One check closes
both, which is why there is only one.

The predicate is total. Both additions are checked, so a length that overflows `addr`, or a region
whose own `base + size` wraps `u64`, answers "refuse" rather than panicking. A wrapping region can
only over-reject: the run's end is a huge in-region address, above any wrapped limit. Real DMA
regions are bounded by RAM and never wrap, so the hardening changes nothing the kernel sees.

## What each harness covers

`validate_and_shadow_confines_every_chain` is the main theorem. The driver's descriptor table is
fully symbolic (every descriptor's address, length, flags and next link) and so is the region. The
harness walks a chain from any valid head with the real `shadow_one_head`, and the shadow write
closure asserts the confinement invariant on every descriptor as it lands. A run that trips nothing
proves the device only ever reads in-region, non-indirect descriptors. The array indexing is
bounded on a symbolic index, so the same run proves the walk never reads or writes past the table.

It walks one head, not a whole symbolic batch, and loses no coverage. One head's chain already writes
up to `qsize` fully symbolic descriptors, and the per-write check quantifies over arbitrary content
and position. The outer loop of `validate_and_shadow` only repeats that validated single-head step
for each further head, so it reaches no new descriptor state. Its bound is
`an_oversized_batch_is_refused`, whose memory closures panic if called: a batch claiming more than
`qsize` entries is refused having read and written nothing.

`a_descriptor_mutated_after_validation_cannot_reach_the_device` has two halves. During the walk the
driver's table is read through `ChainMem::racing`, so every load returns a fresh symbolic value, as
a driver rewriting its descriptors from another core would. The defect it catches is the double
fetch: validating one read of a descriptor and copying a second read into the shadow. The second read
is unconstrained, so the invariant fires the moment it lands. Code that reads each descriptor once
into a local, validates the local and copies the local holds. After a successful walk, the harness
snapshots what the device will read from the shadow and aims the driver's own copy at an arbitrary
address. The shadow is unchanged. A model with a fixed table returns the same value however often a
load is made, so it cannot see a double fetch at all.

`the_outer_walk_stays_inside_the_rings_and_terminates` proves the loop that feeds the chain walk.
`avail.idx` is a `u16` that wraps, and the walk runs `from_idx .. to_idx` through the wrap, so
`from = 0xffff, to = 0x0002` is an ordinary three-entry batch. Its region and descriptor are
concrete on purpose, since descriptor content is the main theorem's job. Making both symbolic would
put `qsize` heads times `qsize` descriptors of symbolic reads in one formula, the batching blowup
the main theorem factors out. `#[kani::unwind(11)]` is one more than the walk can need: the batch
guard caps the outer loop at eight iterations, and each chain there is one descriptor. Kani's
unwinding assertion fails if a loop could run longer, so termination is proved, not assumed.
