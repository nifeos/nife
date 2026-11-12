# P1: before the tag

An appendix to [the milestone](../1091-filesystem-fixture-outside-review.md).

## P1: before the move, or before the first tag

6. The crate still describes this tree.
   Finding: the externalization lint counts 18 hits, which must reach zero before the crate leaves.
   Seven are the word `nife` in prose (lines 13, 616, 638, 639, 654, 1157 and `README.md` line 7).
   Three are data literals (`walk::SMALL_BODY` is `"nife walk entry\n"` at 571; `blank::NAMES` are
   `"nife boot"`, `"nife root"`, `"nife data"` at 1167; `blank::MADE_BODY` at 1174). Eight are
   program names in doc comments (lines 49, 209, 370, 764, 776, 781, 817, 976, 1012). Beyond what
   the lint counts, the prose explains internals a stranger cannot open. They are `verb::TABLE`'s
   row for `SETMTIME` and `Server::set_mtime_now` (976 to 981), `read_record` and RedoxFS record
   levels (179 to 191), `grant_plan::nav::TwoRoots::resolve` (805), `fs_service::start` (209),
   `bench/transfer-size-sweep.sh` (156), `crates/calendar`'s `DateTime::to_unix` (365), and the
   `ignore` crate's parallel walker (660).
   Evidence: `helpers/lint_externalized_crates.py --count`; the lines above.
   Change: write the prose in the vocabulary of roles (the client, the server, the caretaker, the
   host tool) and keep the reasons without the names. The data literals change too. Keep `walk
   entry` inside `SMALL_BODY` because `RG_SEARCH` matches on it, and keep the body at 16 bytes. The
   host check reads back `blank::NAMES` from the GPT entries, and the host tool reads back
   `MADE_BODY`, so the image builder and `xtask/src/disk_check.rs` change in the same lane. Both peers of each literal are in this tree, so the interface version
   stays at 1; the lane records that judgment.
   Door: two-way. Effort: M.

7. History and process residue that survives the `Name:` strip.
   Finding: after `crate_release.strip_name_blocks`, the doc comments still say "this lane's
   coinage" (64), "the globbing lane's set grant" (438), "the globbing lane's tests" (481), "this
   milestone's gate" (517), "`walk_pricing` (provisional)" (537), a dated "Names: provisional"
   line (540, a plural the strip does not recognize), "at the time of writing" (566), "the milestone
   taught" (776), "the roadmap block's own example" (806), a dated "ruled on" (821) and a dated
   "It replaced an `ABSOLUTE_REFUSED` bit" (893). A document states the present rule, and git is
   the history. The template's history ratchet covers only the README and `docs/`, so no
   gate catches these, but a stranger reads rustdoc.
   Evidence: the lines above, checked against the stripped text.
   Change: delete or restate each in the present tense without the date or the process word.
   Door: two-way. Effort: S.

8. `durability` is a public module nothing reads.
   Finding: no file outside the crate names `filesystem_fixture::durability` or any of its five
   constants; the module's own `BUGS` says the role that ran the witness went with SMB, and
   `notes/filesystem-fixture.md` says re-homing it is small work with no design question. An
   outside maintainer meets five verdict words whose documentation says nothing produces them.
   Evidence: lines 90 to 119; the grep of every `pub` item.
   Change: either rebuild the witness on a role that needs no network before the move, or delete
   the module and keep the two-check classification in the repository's `docs/`. Deleting is the
   recommendation: a public item that nothing produces is a promise the crate cannot keep, and the
   classification is a paragraph, not an API.
   Door: two-way before the move (and the crate never publishes). Effort: XS to delete, M to
   re-home.

9. The shared-frame witness's documentation states the outcome the system test refutes.
   Finding: lines 79 to 88 describe `SHARED_SUBSTITUTED` as the current state and
   `SHARED_ISOLATED` as what a fix would make true. `system_tests/src/user/fs_shared_page_tests.rs`
   asserts `SHARED_ISOLATED` on separate windows and uses `SHARED_SUBSTITUTED` as the negative
   control on one shared window, and the milestone the witness exists for is `BUILT`. The text
   is the pre-fix wording left standing.
   Evidence: lines 79 to 88; `fs_shared_page_tests.rs` lines 58 to 83.
   Change: restate in the present: `SHARED_ISOLATED` is the expected verdict on separate windows,
   `SHARED_SUBSTITUTED` the control's verdict on one window, `SHARED_UNEXPECTED` a wiring fault.
   `notes/filesystem-fixture.md`, "The shared-frame witness", wants the same sentence.
   Door: two-way. Effort: XS.

10. The 14 surviving mutants need exemptions the new repository does not have.
    Finding: the census records 14 missed mutants in every weekly run. Seven are `1 << 0` written as
    `1 >> 0` (lines 236, 715, 794, 828, 883, 1017, 1050, each a witness's first bit, and both
    expressions are 1). Seven are `|` written as `^` in `attrs::EXPECTED` (1081 to 1087, disjoint
    bits, so both operators agree). Nife's triage records them as equivalent. The template fails a
    survivor without an entry in `.github/mutants-exempts`, in both `check / mutation-diff` and the
    weekly `mutation` verdict, and the release gate reads that verdict.
    Evidence: `notes/project-metrics/mutation-triage.csv` and `mutation-survivor-classes.csv`.
    Change: either carry the 14 entries with their reasons into the scaffold's `mutants-exempts`, or
    remove the mutation sites. To remove them, write each first bit as `1` and `EXPECTED` as the
    literal `0xFF`, with the test (already at `src/tests.rs` line 300) proving it equals the OR of
    the eight. The
    second is smaller and leaves no exemption to maintain.
    Door: two-way. Effort: S.

11. The caller workflows, the manifest and the records the template expects.
    Finding: the scaffold's `ci.yml` sets `targets: ''`; this crate builds for three bare-metal
    targets and its README says so. The template's `allow-unpublished` input exists for a fixture
    crate that carries `publish = false` on purpose. The manifest's comment on that line points at
    `crates/publish-allowlist.txt`, a file the new repository will not have. It is the same finding
    as milestone 1048 (what an outside maintainer would fix before 0.1.0) item 1. The manifest has no `repository` field, which the externalization lint
    treats as the one place `nifeos` may appear. `.github/accepted-words` needs the Unix spellings
    item 4 keeps. `release.yml`'s gate refuses a tag while `docs/stranger-test.md` reads none, and
    the pre-release tag `v0.1.0-pre.1` the move should carry goes through that gate.
    Evidence: `crate-workflows/scaffold/caller-workflows/ci.yml`; `Cargo.toml` line 11;
    `crate-workflows/README.md`, "The release path".
    Change: `targets` set to the three; `allow-unpublished: true`; the publish comment reads "this
    crate is test data and never publishes"; a `repository` field; the accepted-words file; a
    stranger run recorded before the tag.
    Door: two-way. Effort: S.

12. The README's one-screen claims mislead a stranger in two places.
    Finding: "nothing that boots depends on it" is true of the kernel and init, which 886's
    done-when pinned. But `disk_partitioner` (the `blank` layout and names), `mkfs` (`blank::LBA`,
    `MADE_NAME`, `MADE_BODY`), the `redoxfs_server` binary (`crash::CUT`) and library (the `RECORD`
    assertion) and `swish` (the witness module) link it. The first three ship in the `disk-tools`
    and `redoxfs` packages. A stranger reads the sentence as "test-only". The heading
    "## The contract" names the one thing the crate says it is not.
    Evidence: `README.md` lines 10 to 11 and 13; `redoxfs_server/src/bin/redoxfs_server.rs` line
    198; `redoxfs_server/src/block.rs` lines 28 to 33; `components/src/disk_partitioner.rs` lines
    59 and 180 to 184; `redoxfs_server/src/bin/mkfs.rs` lines 76 and 183.
    Change: add one sentence naming the roles that compile it in and why (a test hook, a test layout
    the partitioner writes). The heading becomes "What it holds".
    Door: two-way. Effort: XS.
