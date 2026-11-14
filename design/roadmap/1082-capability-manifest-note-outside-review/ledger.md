# The ledger

An appendix to [the milestone](../1082-capability-manifest-note-outside-review.md).

| item | state |
|---|---|
| 1, 2, 8, 9, 14 | left: the move's (the scaffold, the README, the fuzz workspace and the manifest), or the template's |
| 3 | (a) and (b) done: `write_zones` is one body for `encode_zoned` and `encode_serviced`, so both check each zone with `is_canonical_zone` (`the_serviced_encoder_refuses_a_zone_the_resolver_would_not_spell`, `the_serviced_encoder_refuses_a_zone_with_a_trailing_dot`), and `encode` asserts `minimum <= maximum` (`the_encoder_refuses_an_inverted_memory_grant`); the encoding-direction harness and its falsification patch are left, because it needs a symbolic `Manifest` built field by field |
| 4 | left: the citation rewrite is M and waits for the README it feeds; one citation (the argv paragraph) is already plain |
| 5 | partly done: `VERSION`, `Error::WrongLength` and the `verification.rs` header now say what the code does; the exact-offset wording, the cut version 3 descriptor and the stale BUGS line are left |
| 6 | left: private renames, which wait for the next template release that runs the check |
| 7 | left: the catalog round trip moves with `grant_plan`'s fate under milestone 1017 |
| 10 | done: `Display`, `core::error::Error` and `Hash` for `Error` (`an_error_prints_and_is_an_error`) |
| 11, 12, 13, 15 | left: additive methods and three more harnesses, each M or a new public name |
| decisions | left: the owner literal, the `_LEN` family, `service` and `VERSION` are the architect's |
