# P0: blocks the move

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

## P0: blocks the move

1. The manifest is not standalone.
   Finding: `crates/boot_ladder/Cargo.toml` inherits `license` (line 9) and `lints` (lines 20
   to 21) from the nife workspace, carries no `rust-version`, no `description`, no `repository`,
   no `readme`, no `keywords` and no `categories`. The `publish = false` line (line 8) points the
   reader at `crates/publish-allowlist.txt`, a file that will not be in the repository. The header
   comment (lines 1 to 3) describes a `Name:` block the template strips. The body comment (lines 11 to 18) says the crate is "six `&'static str`s" when it holds eight and two slices. It also cites a milestone and a rule of `AGENTS.md`.
   Why it matters: `check / standalone` fails on each of the first three. `cargo publish --dry-run` refuses a manifest without `description` and `license`. A stale count is the first thing a
   stranger reads and the first thing they stop trusting.
   Change: write `license = "MIT OR Apache-2.0"`, `rust-version = "1.88"` (the value every other
   package in `notes/crate-reviews.md` states), a one-sentence `description`, `repository`,
   `readme = "README.md"`, and the template's required `[lints]` table (`unsafe_op_in_unsafe_fn`,
   `missing_docs`, `unexpected_cfgs` naming `cfg(kani)`) as literals. Replace every comment with
   one sentence a stranger can act on. The `publish` line stays until the release that publishes,
   with the comment "set to `true` only in the release that publishes".
   Door: two-way, except the license, which is already ruled for every crate. Effort: S.

2. The files a stranger needs do not exist.
   Finding: the crate has no `README.md`, no `LICENSE-MIT`, no `LICENSE-APACHE`, no
   `CHANGELOG.md`, no `_typos.toml`, no `docs/stranger-test.md`.
   Why it matters: `check / standalone` requires the README with `EXAMPLES` and `BUGS` headings,
   the two license files and the typos configuration; `release / gate` refuses while
   `docs/stranger-test.md` reads `Last run: none`.
   Change: the scaffold's files, filled in. The README's one screen says what the crate is for in the words of item 3. It shows the verdict line and how a matcher keys on `SELF_TEST` and `SELF_TEST_FAILED`. It carries `EXAMPLES` (a reader that finds the verdict in a transcript) and `BUGS` (the crate's current `BUGS` list, reworded for a stranger).
   Door: two-way. Effort: S.

3. Shipped text points into this tree.
   Finding: `script/crate-standalone` counts 10 milestone citation lines (first `Cargo.toml` line 11). It counts 3 lines naming a path in nife (first `src/lib.rs` line 8, "See notes/boot-ladder.md"; also lines 79 and 141). It counts one line naming the architect outside a `Name:` block (`src/lib.rs` line 62 to 63, a ruling cited by a pull request number). Beyond what
   the script counts, the doc comments name `kernel_main`, `kernel/src/machine_description.rs`, `kernel/src/self_test.rs`, `components/src/swish.rs`, `kernel::console::yield_screen`,
   `machine_discovery::framebuffer::SCREEN_HOLD`, `cargo xtask swish-check` and `cargo xtask
   uefi-boot`, and `AGENTS.md` rule 7 (lines 5, 31, 42, 50, 109 to 111, 119, 127, 139 to 140).
   Lines 82 to 83 and 108 to 111 carry a dated history of the check list, which the documentation
   standard sends to git.
   Why it matters: the move cannot strip these as it strips `Name:` blocks; each is a link a
   stranger cannot follow, and the ruling citation names a person in text that leaves the tree.
   Change: rewrite each doc comment to say who prints the line and who reads it in role words ("the
   kernel", "the shell", "a console reader"), with no path. State the ruling behind `absent`
   without the name or the pull request number. Keep the table of earlier list lengths, keyed by
   which names each list held rather than by date. Move the design narrative (why the banner is
   generic, why the machine line comes last) to the README.
   Door: two-way. Effort: S.
