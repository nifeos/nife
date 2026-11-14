# Ledger

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md).

| item | state | where |
|---|---|---|
| 1 manifest does not stand alone | LEFT | move work; the new repository inlines the license and lints |
| 2 README, licenses, changelog, stranger record | LEFT | move work, and the stranger run is a person's |
| 3 contract-crate gate | LEFT | the template's kept-name form `(network_time_protocol, RFC 5905)` trips clippy's `doc_markdown` here, and backticks may not satisfy the gate; the move settles which |
| 4 `PACKET_LEN` unit | LEFT | a public name; Decision 2 |
| 5 British spellings in names | LEFT | public names and the harness name; Decision 2 |
| 6 acronyms | LEFT | doc-comment sweep that belongs with the README rewrite of item 2 |
| 7 dependency on `clock_contract` | LEFT | Decision 1 |
| 8 text points back into nife | LEFT | stripped by the move |
| 9 safe Rust | PARTIAL | `#![forbid(unsafe_code)]` is in. LEFT: the repository's fuzz opt-in and its target, which belong to the new repository |
| 10 exhaustive sweep and the workspace profile | LEFT | the new manifest carries it; the new repository measures coverage under optimization |
| 11 surviving mutants | LEFT | the exemptions file lives in the new repository |
| 12 totality of `Interval` | DONE | `an_interval_no_exchange_could_make_still_reads_without_a_panic`: both accessors saturate, so no constructor argument panics. The public signature stays; narrowing `from_ticks` to `i64` stays open as a one-way choice. LEFT: a Kani harness over the accessor |
| 13 two RFC checks | DONE | `a_server_claiming_an_impossible_root_distance_is_refused` (now `>=`), `a_reference_timestamp_after_the_transmit_timestamp_is_refused`; the new `Reject::ReferenceAfterTransmit` is provisional and `network_time_client`'s `reject_code` answers 13 |
| 14 replay duty | PARTIAL | the `Query::accept` documentation states it. LEFT: the README's BUGS line |
| 15 accept ever accepts | DONE | Kani `a_consistent_exchange_is_accepted` with its falsification patch (stratum `>` to `>=`) |
| 16 `Timestamp::fraction` | DONE | public; `the_fraction_field_is_readable` |
| 17 low-entropy items | LEFT | Decision 3 |
| 18 `Short` field | LEFT | a public layout; Decision 2 |
| 19 `dest` parameter | DONE | `destination` in `Query::accept` (no caller names the parameter) |
| 20 drifted notes | DONE | `notes/ntp.md` states no count; `notes/ntp/what-is-proved.md` lists the eighth harness |
| 21 `Reject` order claim | DONE | the example's sentence now matches `accept` |
| 22 `abs_ticks` | LEFT | public name; Decision 2 |

Decisions 1 to 3 carry background, a recommendation and an example above; the architect has ruled none.
