# What was checked, and how

An appendix to [the milestone](../1098-line-editor-outside-review.md).

Verified firsthand, on a fresh clone at the head of `main`:

- `cargo test -p line_editor`: 58 unit tests and 3 documentation tests pass.
- `cargo clippy -p line_editor --all-targets -- -D warnings` and `cargo doc -p line_editor
  --no-deps` with `RUSTDOCFLAGS=-D warnings`: clean.
- `cargo build -p line_editor --target riscv64imac-unknown-none-elf`: builds; the crate is
  `#![no_std]` and has no `unsafe` block.
- The template's helpers over `src/` with the manifest: `spelled_out.py` 3 findings (ceiling 0);
  `unit_symbols.py`, `contract_names.py` and `british_spelling.py` 0; `abbreviations.py` 40
  components it does not know; `standalone.py` 12 findings, listed in item 1.
- Probe tests over the escape parser and the repaint paths (a long CSI parameter, many `;`
  separators, `ESC ESC [ A`, an X10 mouse report, a prompt past `PROMPT_MAXIMUM`). Findings that
  concern only behavior are items 5 and 10; anything else went through the private channel and
  not into this block.
- Which callers use which items, by grep over the tree: 38 files outside the crate name it. 32 of them reach only `proto` or `component`. `components/src/line_editor.rs`, `components/src/swish/prompt.rs`,
  `crates/video_terminal` and the terminal sink caretaker use the engine (`LineDisc`, `Sink`,
  `expand_output`, `Event`).
- Download figures for the prior-art crates, from the crates.io API.

Recorded, not re-run: the mutation ledger (`notes/mutation-testing/baseline-ledger.md`) has 48 survivors, 45 killed, 3 equivalent for this crate. The fourth triage appendix indexed in `notes/mutation-testing.md` records 24 missed brought to 8, with 4 equivalents and 4 recorded gaps (all `BUDGET_PAGES = 2 * INSTANCE_PAGES + 16`). The first weekly runs record the score moving from 80.1% to 98.5%. A census is outside this review's window.

Not run: a mutation census, the QEMU system tests that drive the `line_editor` process
(`system_tests/src/user/terminal_*_tests.rs`, `raw_mode_tests.rs`), Miri, a fuzz campaign (the
crate has no fuzz target), Kani (the crate has no harness).
