# What was checked, and how

An appendix to [the milestone](../1072-jh7110-clock-and-reset-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main`, with the crate
copied out of the workspace into a directory of its own, as `script/crate-standalone` does:

- `cargo +1.88 test`: 45 unit tests and 2 documentation tests pass. `cargo +1.85 test` refuses the
  manifest, so `rust-version = "1.88"` is the true floor (let chains).
- `cargo +stable build --lib` for `riscv64imac-unknown-none-elf`, `aarch64-unknown-none-softfloat`
  and `x86_64-unknown-none`: clean. `cargo doc` with broken intra-doc links and invalid HTML tags
  denied: clean.
- `cargo clippy --all-targets -- -D warnings` on stable and on 1.88: 17 errors, all
  `clippy::doc_markdown` (item 1). The same with the template's Kani shim and `--cfg kani`: the
  same 17 and nothing from the proof module.
- Kani 0.68.0, the version in the template's `.kani-version`: all nine harnesses verify; 3 minutes
  59 seconds wall, 838 MB peak resident, on one host. The two falsification patches read
  (`every_fixed_plan...`, `a_device_plan_from_specifiers...`) each invert the property their header
  names; the other seven were not replayed.
- cargo-llvm-cov: `src/lib.rs` 307 of 311 lines, 98%, over the template's per-file floor of 85.
- `cargo mutants --list`: 261 mutants. Nobody ran a census (item 5 says why).
- The template's helpers, as `check / standalone` runs them:
  - `standalone.py` gives three findings, all expected of a crate still in nife: `publish = false`,
    the `Name:` blocks, one line naming the architect inside a `Name:` block.
  - `spelled_out.py` gives 7 refusals and `register_names.py` 8, both at ceiling 0 (item 2).
  - `prose_acronyms.py` gives 28 refusals at ceiling 0 (item 2).
  - `unit_symbols.py`, `question_shape.py`, `british_spelling.py`, `service_model.py` and
    `contract_names.py` give 0.
  - `prose_figures.py`, `history_phrases.py`, `file_length.py`, `comment_blocks.py`,
    `function_length.py`, `inflated_wording.py`, `readme_sections.py` and `readme_acronyms.py` run
    clean.
  - `prose_caps.py` finds one 62-word sentence over the 40-word cap, and `passive_voice.py` finds 8
    passives in 876 words against an allowance of 1 (item 14).
  - `abbreviations.py` finds one component, `jh`.
  - `readme_badges.py` gives four findings, all wanting the repository that does not exist yet.
  - `typos` runs clean over the source.
- Against the pinned Linux commit the crate cites, the review checked the PLL2 field positions
  (`DACPD` bit 15, `DSMPD` bit 16, `FBDIV` bits 28:17 at `0x2c`, `POSTDIV1` bits 29:28 at `0x30`,
  `PREDIV` at `0x34`). It also checked the PLL rate formula and the divider rule (`DIV_ROUND_UP`
  then clamp to `max_div`). It checked `i2c5`'s `reg` and cells and `gmac0`'s clocks and resets. It
  checked the two board files' `&gmac0` blocks (item 4).
- The DesignWare count formulas in Linux's `i2c-designware-common.c` (item 8).
- The comparable crates on crates.io, through its API (prior art).

Inferred from reading, not run: the behavior of `parse_register_property` on a 12-byte property
(item 3), the `name_index` miscount (item 9) and the fallback asymmetry (item 10).

Not run: a mutation census (outside its window), `cargo semver-checks` (no release to compare
against), Miri, the stranger test, a build of pull request #3475. Not verified: the DesignWare
databook's minimum count values (item 8), OpenBSD's and FreeBSD's spellings beyond what the crate
cites, and whether a VisionFive 2 v1.2A wants the transmit parent the plan selects (item 4).
