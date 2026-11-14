# Done when

An appendix to [the milestone](../1089-environment-contract-outside-review.md).

- `read_field` bounds every length by its field's cap, three tests cover a length one past the cap
  and 255, and the doc comments state the bound.
- An accessor returns only a member of its key's domain, with a test for a recognized page
  carrying a non-member. Or the maintainer records the decision to keep the raw reader and the
  README says callers check the domain.
- `PAGE_CONTENT_BYTES`, `ConfigurationPage` and `virtual_address` are the names in the crate, the
  overlay and the callers, and both naming baselines lose their rows.
- The crate records the identity values the architect rules, pinned by a host test and a `const`
  assertion, and the accessor names the milestone decides applied with their aliases.
- A `verification` module with three harnesses and three falsification patches passes `cargo kani`
  and `proofs / falsify` in the new repository.
- `script/crate-standalone environment_contract` passes every row, and `cargo package --list`
  ships the README and both license texts.
- A compile-time check holds the domain lists to their caps, `write_field` no longer
  truncates, and `helpers/vendored_name_gates.py` carries no exemption for the crate.
- `Refused` implements `Display` and `core::error::Error`.
- Item 10's five documentation sentences carry their corrections.
- The template's `check`, `proofs` and `weekly` workflows are green on `main` in the new repository
  for the crate's own reasons, and `fuzz.yml` passes with its "no opt-in" note.
