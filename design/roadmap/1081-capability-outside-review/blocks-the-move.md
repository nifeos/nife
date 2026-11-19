# P0: blocks the move

An appendix to [the milestone](../1081-capability-outside-review.md).

## P0: blocks the move

1. The manifest does not stand alone, and the files the template wants are absent.
   Finding: `Cargo.toml` inherits `license.workspace = true` and `[lints] workspace = true`, which
   name a workspace the new repository does not have; it carries no `rust-version`, `description`
   or `repository`; its `publish = false` comment points at `crates/publish-allowlist.txt`, a file
   that will not be there. There is no README, no `LICENSE-MIT`, no `LICENSE-APACHE`. The template's
   `check / standalone` reports all of this and fails. Outside the workspace the `#[cfg(kani)]`
   module warns under `unexpected_cfgs`, which `-D warnings` turns into an error, because the
   `check-cfg = ['cfg(kani)']` line lives in the workspace's lints. The move tool's `add_repository`
   inserts `repository` after a `license =` line and finds none in this manifest, so the staged
   manifest has no `repository` while the tool reports the step as done (see item 6).
   Evidence: `crates/capability_table/Cargo.toml` lines 5 to 9; `helpers/standalone.py` on the staged
   tree: 10 lines; `crate-workflows/sample/Cargo.toml` for the shape.
   Change: `license = "MIT OR Apache-2.0"`, `rust-version = "1.85"` (edition 2024 needs it; the
   tests pass there), `description`, `repository`, an own `[lints.rust]` block with `missing_docs`,
   `unsafe_op_in_unsafe_fn` and the `unexpected_cfgs` line, and a comment on `publish = false` a
   stranger can act on. Add both license files. Write the README in the template's section order
   (`readme_sections.py`): summary, EXAMPLES, "What is proven", Building and testing with the Kani
   install line on one line, one real falsification example, BUGS, Minimum Rust version, License.
   The two crate-level doctests are the EXAMPLES.
   Door: two-way. Effort: S.

2. The crate's text refers back into nife.
   Finding: `src/lib.rs` and the fourteen patch headers cite section numbers (15 lines) and milestones (29 lines). They also cite paths into the tree (20 lines: `notes/capabilities/table.md`, `kernel/src/...`,
   `design/roadmap/0778-...`, `script/verify`) and name the architect with dates (5 lines after
   the `Name:` blocks go; 7 before). The rule for a crate that leaves is that it never mentions
   nife, and a document states the present rule without its history. Two of the citations are
   already wrong inside the tree. `kernel/src/capability.rs` (line 213) does not exist; the file is `kernel/src/capability_object.rs`. ``split_never_widens_rights`` (line 138) renders as a link to a file that rustdoc does not serve. `cargo doc -D warnings` does not catch it because it is a URL and not an intra-doc path. The crate documentation also carries kernel
   facts a generic reader cannot use ("Only `Rendezvous` consults it", "Only `Process` consults
   it", lines 70 and 79) and the phrase "A thread's capability table," above (line 43) points at
   text that now sits below it.
   Evidence: `helpers/standalone.py` citation counts on the staged tree. `src/lib.rs` lines 38 to 43, 66 to 79, 123 to 124, 136 to 141, 150 to 155, 173, 186 to 192, 209 to 217, 237 to 241, 496 to 506, 529 to 534 and every harness comment. Also the patch headers.
   Change: rewrite each doc comment to state what the item does and why, in the crate's own words. Move the reasons that need the kernel to `notes/capabilities/table.md`, which stays in nife. Turn the harness links into intra-doc links (`[`verification::split_never_widens_rights`]`
   under `#[cfg(kani)]` needs a plain code span instead). Rewrite the patch preambles to name the
   defect and the assertion only. Keep the dates out: git is the history. Do this in nife before
   the move, so the dry run reads zero citation lines.
   Door: two-way. Effort: M.
