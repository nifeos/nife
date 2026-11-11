# What was checked, and how

An appendix to [the milestone](../1075-abi-outside-review.md).

Verified firsthand on a fresh shallow clone of `nifeos/nife` at the merge of pull request #3514. Also verified on the tree that `script/crate-move --dry-run --no-build` staged from it against template release v7:

- `cargo test` on stable 1.99.1 and on 1.88 (the manifest's `rust-version`): 20 unit tests and 4 documentation tests pass. The README's example is among them.
- `cargo doc --no-deps` with warnings denied: clean. `cargo clippy --all-targets -- -D warnings`:
  red, on both stable 1.99.1 and the nightly nife pins (item 1).
- `cargo build --lib` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`: clean. `cargo package --list` shows the manifest, both licenses, the README, the changelog and `src/`.
- The dry run's own checks: standalone, file length, comment blocks, markdown links and falsification records pass. Prose caps, orphan modules, citations and the nife-word check fail (items 1, 3, 9). The orphan finding is template v7's and not the crate's. The helper at the template's head declares every file. The `crate_move.py` patch fix in flight (#3521) did not come into play, since the crate has no falsification patch.
- The template's helpers at head over `src/`: `spelled_out.py` and `unit_symbols.py` judge 233 names and refuse none. `unsafe_scan.py` finds no `unsafe`. `question_shape.py` refuses one name (item 12). `passive_voice.py` refuses the README (item 9). `history_phrases.py` and `function_length.py` pass.
- Every public `u64` constant against the pinned-values tests: 99 defined, 99 pinned, counted by
  hand (item 11 is that nothing counts them).
- Every caller in the tree, by `grep` over `Cargo.toml` and `src/`: 16 manifests depend on the crate. `console::WRITE` and `CapabilitySlot` have no user outside the crate.
- The kernel's twins: `kernel/src/capability_object.rs` asserts `CAPABILITY_TABLE_SLOTS` and `rights::ALL` equal at compile time. `futex::admitted`, `usage::is_known`,
  `survey::record::is_known`, `reboot::from_psci` and `reboot::from_sbi` each have their one kernel
  caller.
- The recorded mutation census: the baseline row reads 67 caught, 6 missed, 0 timed out, 8 unviable. The triage notes name three of the six as equivalent (item 20). This review ran no census.
- crates.io, through its API: the name `abi` belongs to another crate (decision A), and the
  download figures in the prior-art box.

Inferred from reading, not run: the behavior of the template's `proofs` job on a crate with no Kani harness (item 21). The coverage figure against the template's 85 floor and the x86_64 register names for the fourth and fifth syscall words (item 6) come from reading too.

Not run: a mutation census (outside its window), the stranger test, `cargo semver-checks`, the
publish rehearsal, and the review packet script, which writes into `notes/`.
