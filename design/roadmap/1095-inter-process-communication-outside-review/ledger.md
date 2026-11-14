# Ledger

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md). Each item
reads DONE with the test or harness that holds it, or LEFT with the reason.

| item | state | where |
|---|---|---|
| 1 review package | LEFT | the package is the move lane's input to the architect's name rulings; it waits for D1 to D3 |
| 2 `intrusive_queue` leaves first | LEFT | decision D3; the order of leaving is the architect's |
| 3 stand-alone text and manifest | LEFT | done by the move commit, which strips the provenance blocks and rewrites citations |
| 4 type named `Send` | LEFT | decision D1 (a public name) |
| 5 gate names, private and prose parts | DONE | `a_canceled_or_rearmed_timer_never_fires_stale` (harness and patch renamed); test bindings spelled `pointer_a`, `pointer_b`, `pointer_c`; the British prose spellings are gone |
| 5 gate names, public parts | LEFT | `one_queue_invariant_holds` and `Key::virtual_address` are public names; they wait for the package rulings |
| 6 design record travels | LEFT | `docs/design.md` and the README belong to the move |
| 7 manifest metadata | LEFT | the move's manifest rows |
| 8 `signal` harness falsification | DONE | `falsifications/verification.signal_preserves_the_invariant.patch` replays the overflow, and the harness comment says what it guards |
| 9 mutation record | LEFT | needs a full run in the new repository |
| 10 timer and futex scope | LEFT | decision D2 |
| 11 one queue by construction | LEFT | an option for the maintainer after the move; changes the proofs the README lists |
| 12 `Futexes` assertion | DONE | `Futexes::new` refuses zero buckets at compile time (`const` assertion) |
| 12 `Futexes` names, `len`, iterator | LEFT | public names; the iterator lands in `intrusive_queue` |
| 13 `Timer::deadline` as `Option` | LEFT | a public signature, one-way at publish |
| 14 saturation test | DONE | `the_pending_count_saturates_and_a_receive_still_drains_one` |
| 15 one shared test node | LEFT | five copies differ in what they own; a shared module waits for the move so it lands once |
| 16 documentation details | DONE | `# Examples` heading, "4-aligned, which the caller checks" |
| 16 named diagnostic structs | LEFT | changes a public signature the kernel's dump reads |
| 17 Kani macro warnings | DONE | the three `assert!` match arms carry braces |
