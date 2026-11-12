# Done when

An appendix to [the milestone](../1086-domain-name-resolution-contract-outside-review.md).

- `helpers/lint_externalized_crates.py` reports 0 hits for the crate and its waiver line is gone.
  `script/crate-move --dry-run --into <dir> domain_name_resolution_contract` (the build half
  included) exits 0 against the template release the move will pin.
- `fixture` lives in a crate that stays in nife, with its three users following it. No item of the
  contract crate names a host name, an address or a report code of a test.
- `Cargo.toml` stands alone (`rust-version`, `description`, `readme`, `keywords`, `categories`, an
  explicit license, explicit lints). Its dependency on `domain_name_system` is a pinned `git`
  dependency. The README carries `EXAMPLES` and `BUGS`, the three-party sequence, the page layout,
  the reply words and the two tiers of zone spelling.
- The architect has ruled decisions 1 to 3. The names, the control reply word and the identity apply
  in nife, in the std farm's copy and in `domain_name_system`. The template's `spelled_out.py`,
  `register_names.py`, `unit_symbols.py` and `question_shape.py` report 0 over the staged tree.
- `Outcome::from_word` refuses a count over the limit and an undefined status, with tests. The
  layout assertions compile. One expression bounds the name's length in the crate, the resolver and
  the std client.
- One Kani harness over `Grants::control` proves and its falsification patch turns it red. One
  fuzz target of the crate alone builds. `.github/mutants-exempts` lists the six equivalent mutants
  with the reason.
- The rendered crate root shows the layout table whole, and `cargo doc` with the template's rustdoc
  lints denied stays clean.
