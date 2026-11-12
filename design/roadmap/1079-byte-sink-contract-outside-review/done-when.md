# Done when

An appendix to [the milestone](../1079-byte-sink-contract-outside-review.md).

- The repository created from the scaffold passes `check / standalone` with no finding. It has a README in the template's section order with `EXAMPLES`, `What is proven` and `BUGS`, and both license files. It has literal `license`, `rust-version` and lints. It has no path, section, milestone or provenance reference into this tree, and no `Name:` block.
- The `VERSION` documentation states the note-declared pair as the check and names no wire operation. The architect records decision A.
- `fixture` is gone from the published crate and its four consumers build against
  `byte_sink_fixture` (or decision B rules otherwise, and the module sits behind a feature).
- The crate has no dependency of any kind. The `GONE` pin test lives in a nife crate that links both and passes.
- Five harnesses verify under the template's `proofs / kani`, each with a falsification patch that
  `proofs / falsify` replays red, and the README's "What is proven" lists them.
- `unpack` refuses a bytes message of any kind but `KIND_TEXT`, with a test. Every in-tree consumer still passes its tests.
- No British spelling remains. The template's `british_spelling` and `house_style` gates pass.
- Decision C specifies the end-of-stream words, and `rm`'s report documentation agrees with the contract.
- One rename lane applies the names ruled in decision D across every caller, with the std PAL regenerated.
- The zero-count rule, the narrow-`usize` bound, the typed `req` parameters and the four mutant exemptions are in. `cargo test`, `cargo kani`, `cargo mutants` (none missed) and the three bare-metal builds are green at the commit the move pins.
