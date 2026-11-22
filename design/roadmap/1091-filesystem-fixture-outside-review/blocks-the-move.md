# P0: blocks the move

An appendix to [the milestone](../1091-filesystem-fixture-outside-review.md).

## P0: blocks the move

1. The template's cargo-deny policy refuses the one dependency the moved crate will have.
   Finding: the moved manifest pins `file_system_contract` by `git` and `rev` on `nifeos/<crate>`,
   the form §305 allows and 1017 prescribes. The template's `deny.toml` and its scaffold's copy both
   say `unknown-git = "deny"` under `[sources]` and allow no git source, so `check / supply-chain`
   is red on the first pull request of the new repository. No crate repository in the organization
   depends on another yet (unverified beyond the repository list), so this is the first time the
   policy meets the case.
   Evidence: `crate-workflows/deny.toml` and `crate-workflows/scaffold/deny.toml`, section
   `[sources]`; 1017, "The version policy".
   Change: one line in the template's two `deny.toml` files, `allow-org = { github = ["nifeos"] }`
   under `[sources]`, released and pinned before anyone creates this crate's repository. Every later
   crate that links a contract (step 3's `user_mode_runtime` links six) hits the same line, so the
   template is the place, not a per-crate exception.
   Door: two-way. Effort: S (a template pull request and its release).

2. The order and pairing are stale in three places.
   Finding: `README.md` line 56 and the crate root `BUGS` (lines 11 to 12) say the fixture and the
   contract leave together; 1017's step 2 row says the same, citing a dev-dependency that commit
   `ce4ea2015` removed. The contract links nothing; the fixture links it, and Decision 1 puts both
   in one workspace repository.
   The reverse coupling is `byte_glob`, which dev-depends on the fixture and leaves in step 5, which the
   bottom-up rule allows. The sibling review of `byte_glob` proposes moving `crates/byte_glob/tests/
   fixture_agreement.rs` into `crates/file_system_fixture/tests/` with `byte_glob` as a dev-dependency.
   That would make this crate link a crate that leaves three steps later and block step 2 until
   `byte_glob` has left, which is the pairing problem reintroduced the other way round.
   Evidence: `README.md` lines 55 to 56; lines 11 to 12; 1017 step 2 row; `crates/byte_glob/Cargo.toml`
   dev-dependencies; `crates/byte_glob/tests/fixture_agreement.rs` header.
   Change: restate both `BUGS` entries as "depends on `file_system_contract` by path, inside the
   one workspace both crates share"; the owner of 1017 corrects its row's reason. For the glob test,
   `crates/grant_plan/tests/` (which already depends on `byte_glob` and leaves in step 6, after both) or
   leaving it in `byte_glob` keeps the order; this crate must not gain a dev-dependency on `byte_glob`.
   Door: two-way. Effort: XS.

3. Two template gates at ceiling 0 are red on the shipped text.
   Finding: `prose_acronyms` finds ten acronyms used before expansion (SMB line 98, EL0 158, PRNG
   175, RFC 360, FS 765, DOS 890, ISA 1079, BFS 1098, EFI 1151, GUID 1153). The gate judges Rust doc
   comments once per crate at ceiling 0, with no baseline. `typos` with the template's en-us locale refuses
   the British form of "defense" (`src/tests.rs` line 209) and the British form of "behavior" (line 258), which the house style refuses as
   well.
   Evidence: the helper outputs above; `crate-workflows/README.md`, rows "acronym gate" and "house
   style, typos". Change: expand each acronym at first use or drop it. `FS` and `DOS` can go, `EL0`
   becomes "user mode", and the text can write `RFC 3339` as the document name it is. Spell the two
   words the American way.
   Door: two-way. Effort: S.

4. Names the template's name gates refuse at ceiling 0, where nife's baseline does not travel.
   Finding: `spelled_out` refuses `SCRATCH_INIT` (line 49), `MADE_DIR` (316 and `navscape::MADE_DIR`
   918), `NAV_DIR` (339), `GLOB_DIR` (463), `dirscape::MADE_A_DIR` (733), the binding `dir` in
   `rg_match_path` (696) and `TOUCH_MTIME_ADVANCED` (967). `register_names` refuses `walk::SIZES`
   (564) and `walk::SIZE_FILES` (567) for carrying no unit. Seven of these sit in
   `design/naming-rule-baseline.txt`, which stays behind. Beyond the gates, the rules in
   `design/naming.md` catch more. The unit rule ("a length or a limit carries its unit") fails on
   `blank::LBA` (a byte count, 1149), `throughput::UNIT`, `TOTAL` and `RECORD` (byte counts, 140, 150, 192),
   `throughput::WARMUP` (a transfer count, 167), `tree::NAV_TOUCH_AT_UNIX` (seconds, 367) and
   `walk::WIDE_COUNT` (a count, which names its things, 560). Its shape rule ("name the property,
   not the count") fails on `twodir` (790) and `twotrees` (824). Its abbreviation rule fails on `attrs` and the four `_ATTR`/`_ATTRS` names,
   `ABS_INNER`/`ABS_SECRET`, `REDIR*`, `SEQ_*`/`RAND_*` and the `NAV_*` family. The `abbreviations`
   report lists 88 components; `rm`, `rmdir`, `unlink`, `pwd`, `glob`, `motd` and `rg` are Unix or
   ripgrep spellings a reader types. They keep their owner's form with a `#[doc(alias)]`.
   Evidence: the helper outputs; `design/naming.md`, sections "Units", "Shapes" and "Acronyms and
   abbreviations".
   Change: the names Decision 2 decides, provisionally. One rename lane applies them with the
   consumers, since every rename crosses nine manifests.
   Door: two-way before the first tag; every repository that pins them is a nifeos one. Effort: S after the ruling.

5. The crate's names are not ratified and no review package exists.
   Finding: `script/crate-standalone` step 0 fails on the crate name (provisional) and on 244
   unratified names. The ruling that every name a stranger meets has its ratification before a crate moves stands, and the move commit strips every `Name:` block, so the ratification has to happen here.
   There is no `notes/crate-reviews/filesystem-fixture.md` and no row in `notes/crate-reviews.md`,
   so the architect has no package to read.
   Evidence: `script/crate-standalone file_system_fixture`; `ls notes/crate-reviews/`.
   Change: a lane writes the package in the shape of the existing ones (identity, public API,
   proofs, names, license, what the move still needs, recommendation), adds the index row, and
   attaches `python3 helpers/name_release.py --draft file_system_fixture`. Item 4's renames go in
   the same ratification so the architect reads each name once.
   Door: two-way. Effort: M.
