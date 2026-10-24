# `machine_discovery` swept clean, and the gap was a harness that did not exist

*Name: provisional.*

An appendix to [`notes/falsification.md`](../falsification.md).

2026-09-23. The crate parses ACPI tables and device trees, which is the bytes a machine hands us. So
it was taken as a falsification target, on the reading that `script/falsifications --unfalsified`
would name several of its harnesses. It named none. All fifteen already carried `replayable`
records, written by milestone 319 (the crate that parses firmware). `script/falsifications --sweep
machine_discovery` replayed all fifteen on patagonia: 15 swept, 0 survivors, 0 stale or broken.
Nothing here needed repair.

Two of those fifteen were false when they were written, which is the strongest evidence in this
note against fatal risk 2's claim that the proofs prove trivia.
`acpi::the_dmar_fixed_part_decodes_without_arithmetic_overflow` found `host_address_width` computing
`body[0] + 1` in a `u8`, which panicked the x86 boot path on a firmware byte of `0xff`. And
`an_ecam_windows_size_is_total_and_counts_one_mebibyte_per_bus` found the window size wrong beside
it. Both are on a boot path, both were found by the proof rather than by a test, and both are
recorded at the harness.

### What was actually missing

Milestone 524 (the three `x86_64` boot gates) added 342 lines to `x86_64.rs` on 2026-09-21 and no
harness with them. So nothing was `unfalsified`. The code was *unproved*, which no ratio in this
note can see. That is worth stating as its own shape, because it is the cheaper failure to have and
the harder one to notice. The worklist counts harnesses, so code with no harness is invisible to
it. A crate can go to 100% falsified by adding nothing and proving nothing.

The gap closed here is `a_feature_is_never_reported_from_a_leaf_the_part_does_not_answer` (name
provisional). `CPUID` has no fault for a leaf a part does not implement: a read above the maximum
answers with some other leaf's data. So absence and a clear bit are the same bits on the wire, and
the four maximum-leaf gates in `Isa::decode` are the only thing telling them apart. Its
falsification gates an extended leaf on the standard maximum. That is one word of difference that
every machine in this building agrees with, which is why a proof and not a test is what catches it.