# Done when

An appendix to [the milestone](../1072-jh7110-clock-and-reset-outside-review.md).

- A pull request in the crate's repository runs `check / clippy`, `check / standalone`, `check /
  tests`, `check / coverage`, `proofs / kani` and `proofs / falsify` green. The proper nouns of item
  1 sit in `clippy.toml` or in backticks.
- `spelled_out.py`, `register_names.py` and `prose_acronyms.py` report 0 over the crate. The seven
  private constants, the three `len` fields and the two test names carry new names, every acronym
  has an expansion at its first use, and the public renames of decisions 2 to 4 apply as ruled.
- `parse_register_property` answers `None` for every length other than 8 and 16, or decodes by the
  parent's cell counts. A 12-byte host test covers it, and the harness chooses the length.
- The README's BUGS names the GMAC0 transmit parent as a VisionFive 2 v1.3B fact. The constant's
  first sentence says so; the ruling on decision 5 applies.
- One full `mutation / mutation` run on `main` is green with every mutant tested.
  `.github/mutants-exempts` names each survivor with a reason that describes the current body, and
  nife's triage ledger drops the crate's rows.
- The crate computes the fractional PLL2 rate as Linux does, with a host case.
  The doc of `was_already_up` names the reset it reads, `name_index` counts an empty entry, and the
  fallback asymmetry has its reason at the field or is gone.
- The README carries the scaffold's sections with the measured Kani figures and a `-p1`
  falsification example, and the badge row. It has no sentence over 40 words and at most one
  passive per thousand words. A stranger run is on record with no open finding against the
  README.
- `Cargo.toml` carries `repository`, `documentation`, `include` and the docs.rs targets; the
  repository carries the scaffold's files and `.github/accepted-words` with `jh`.
