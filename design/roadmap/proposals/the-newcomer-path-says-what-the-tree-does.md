---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The newcomer path says what the tree does

Raised by the documentation audit of 2026-11-17 (UTC),
[the newcomer path](../../audit-reports/2026-11-17-the-newcomer-path.md), findings 2 to 9. Each item
below is a claim that a command on `main` at `9e8c189c0` showed false, and the report gives the
command. Severity is medium for items 1 to 3 and low for the rest.

Reuse: every item edits a file that exists, so there is nothing to search for. No new dependency.

Two items touch `AGENTS.md` and one touches `design/decisions/`, which a lane does not edit on its
own. The `AGENTS.md` items change words, so they go to an architect with the pull request
(§88 (`needs-architect` as a required check)). The `design/decisions/` item removes a sentence that
stopped being true at merge; whether that counts as bookkeeping is the integrator's call.

## The work

1. `script/apropos` keeps the promise it opens with. Its header and `CONTRIBUTING.md` give
   `script/apropos system call` as the way to find `crates/moho_abi/src/lib.rs`. The search now returns 16
   pages of 520 matches and `crates/moho_abi` is not among them. Either the ranking puts a crate's own
   `lib.rs` above prose that mentions the word, or both documents name an example that works. The
   header's figures (512 pages, five megabytes, four system call numbers) become a sentence with no
   count, or a count a test checks: the corpus is 2040 pages and 16.6 MB, and `crates/moho_abi` declares
   five calls.
2. `notes/adding-a-program.md` points at the file a newcomer edits. Step 1 says the
   `programs!` row goes in `crates/program_table/src/program.rs`. The macro and its 28 rows are in
   `crates/program_table/src/program/catalog.rs`, and the `manifest()` arms are in
   `crates/program_table/src/program/manifests.rs`.
3. `AGENTS.md`'s Environment section states the tools the tree uses. It says QEMU comes from
   Homebrew, which §322 (the QEMU bootstrap is the one QEMU check) replaced with the pinned build
   that `script/ci-qemu` makes. It lists three targets where `rust-toolchain.toml` pins five (the
   two UEFI targets are missing), and it names macOS as the only host while Linux runs the whole
   suite. Its opening line calls the project "a capability microkernel for aarch64"; the README and
   rule 5 say three architectures.
4. `script/ci-build --list` prints only checks. Four comment lines inside the `checks()` table
   heredoc print as rows between `fmt` and `vt-replay`. Move the comment above the heredoc, or have
   the reader skip lines that open with `#`.
5. `cargo xtask`'s usage names every command it dispatches. `xtask/src/usage.rs` omits
   `disk-throughput`, `job-mix`, `network-bench`, `region-check` and `vt-replay`, which
   `xtask/src/command_line.rs` dispatches. A host test that compares the two lists keeps them
   together.
6. `CONTRIBUTING.md` gives `AGENTS.md`'s real size. It says "about 40 KB" in two places; the
   file is 19 KB.
7. Two decided sections drop their merge caveat. §321 (reclaiming a whole region subtree) and §322
   are `status: DECIDED` and still say their number and title are provisional until the merge
   lands them.
8. The newcomer path states the rule in force, with no narration (§298 (a document states what
   is in force now), §267 (a comment states the constraint as it is now)). The places found:
   - `script/setup`'s comments on the dated toolchain and on a date that "used to" repeat.
   - `script/apropos`'s header dates.
   - `CONTRIBUTING.md`'s `BUGS` opening, which dates the page by a run of milestone 117 (the
     stranger test).
   - `AGENTS.md` principle 1 ("As of 2026-08-30") and rule 5 ("corrected 2026-10-06, #1739").
   - `notes/adding-a-program.md`'s lines on when `user/` split, and on what the page told
     newcomers before §89 (`provisional` becomes the fourth provenance state).
