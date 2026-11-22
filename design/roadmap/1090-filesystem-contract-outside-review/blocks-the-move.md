# P0: blocks the move

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md).

## P0: red at the moving commit, or refused by the move rule

1. The crate does not stand alone.
   Finding: 21 externalization-lint hits, every one in prose. `nife` appears in `README.md` line 5 and `:84`,
   `src/lib.rs` line 29, `src/grant.rs` line 11 and `:13`, and `src/xattr.rs` line 76 and `:77`.
   Programs appear by name too. They are `disk_surveyor`, `disk_partitioner` and `mkfs`
   (`src/blk.rs` line 47, `:61`). They are `fs_nameset_caretaker`, `fs_subtree_caretaker`,
   `fs_file_caretaker` and `redoxfs_server` (`src/verb.rs` line 14, `:64`, `:119`, `:249`, `:333`;
   `src/xattr.rs` line 61, `:62`; `src/verb/tests.rs` line 133, `:174`). The last is
   `least_authority_demo` (`src/grant/tests.rs` line 40, `:44`, `:51`). Beyond the lint: `rm.rs`
   describes the `rm` program and cites `byte_sink_contract::eof()`; `fs.rs` line 298 cites
   `abi::rendezvous::BADGE`; `fs.rs` line 274 cites `bench/transfer-size-sweep.sh`; `fs.rs` line 232 cites
   `subtree_scope::Bindings::bind`; `nameset.rs` line 7 cites `grant_plan::Refusal::TooManyNames`;
   `xattr.rs` line 73 cites `redoxfs_host extract`. `standalone.py` adds: `publish = false`
   (`Cargo.toml` line 11, with a comment pointing at a nife file), 31 provenance blocks, 7 milestone
   citations, 4 lines naming the architect, 1 section citation. Of the provenance blocks,
   `src/fs.rs` line 255 quotes a ruling verbatim, against the standing rule that a record states the
   decision and never quotes the decider. The manifest has no `repository`, `documentation` or
   `homepage` field.
   Why it matters: the move rule is that nothing that leaves references back into nife; the
   template's `check / standalone` is a required check and reads every one of these.
   Change: reword each hit for any reader (a caretaker becomes "a proxy that narrows a directory
   capability", a program name becomes its role). The move commit strips the `Name:` blocks, as
   ruled, and the quotation goes with them. Replace the `publish` comment with one sentence a
   stranger can act on. Add the three URL fields. Delete the `rm` module (item 5) or reword it.
   Door: two-way, except `STORE_DIR` (Decision A). Effort: M.

2. `check / clippy` is red on the template's toolchain.
   Finding: stable clippy with `-D warnings` fails on `doc_markdown` for `RedoxFS` at
   `src/lib.rs` line 21, `:73`, `:108`, `src/blk.rs` line 41, `:57`, `src/grant.rs` line 11 and two more sites
   (8 in the library, 9 with the test build). nife's workspace `clippy.toml` carries
   `doc-valid-idents` with `RedoxFS`; the scaffold has no `clippy.toml`.
   Why it matters: the new repository's first pull request is red before anything else is judged.
   Change: a `clippy.toml` at the crate root with `doc-valid-idents = ["RedoxFS", ..]`, carrying
   nife's reason (backticks would render a proper noun as code). Backticking is the alternative
   and the worse one.
   Door: two-way. Effort: XS.

3. The falsification patches carry nife's path, and the README's command with them.
   Finding: all three patches diff `a/crates/file_system_contract/src/dir.rs`
   (`falsifications/*.patch`, the `diff --git` lines). The template's `proofs / falsify` applies
   each patch crate-relative with `git apply -p1`, so after the move none applies and the job is
   red. The doc comments at `src/dir.rs` line 227, `:248` and `:266` name the same path, and
   `README.md` line 74 says `patch -p3`.
   Change: rewrite the three headers to `a/src/dir.rs`, the three doc comments to
   `falsifications/<name>.patch`, and the README to the scaffold's `git apply -p1` form with one
   real example and what the failing run prints. The move commit is the right place, since the
   paths change with it.
   Door: two-way. Effort: XS.

4. `check / coverage` is red on `src/rm.rs`.
   Finding: `rm::status` is never called inside the crate; the standalone lcov reports 0 of 3
   lines, under the per-file floor of 85 that `coverage-floor.sh` applies to every file with an
   executable line. The function's callers are the `rm` program and `file_system_fixture`'s tests.
   Change: resolved by item 5 if the module leaves; otherwise one test that `status(errno)` is the
   errno and `OK` is 0.
   Door: two-way. Effort: XS.

5. The name gates refuse 22 names, and renames are the architect's.
   Finding: `spelled_out` (ceiling 0) refuses module `dir` (`src/dir.rs`). It refuses `IS_DIR` twice
   (`src/dirent.rs` line 4, `src/nameset.rs` line 33) and `STORE_DIR` (`src/xattr.rs` line 77). It
   refuses `grant::spec`, `spec_len` and `spec_rights` (`src/grant.rs` line 116, `:121`, `:126`). It
   refuses `xattr::spec`, `spec_kind` and `spec_value_len` (`src/xattr.rs` line 82, `:86`, `:90`).
   It also refuses seven `is_dir` and `spec` bindings in tests. `register_names` (ceiling 0): `blk::SIZE` (`src/blk.rs` line 27, an opcode, so a false
   positive), `BLOCK_SIZE` (`src/blk.rs` line 43), `MAXIMUM_LENGTH_OR_OFFSET` (`src/fs.rs` line 309,
   ratified), `file_system_statistics::LEN` (`src/statfs.rs` line 3). `question_shape`: `in_opcode_order`
   (`src/verb.rs` line 319). The naming worklist inside nife lists the same ten wire-or-format names.
   `GETMTIME`, `SETMTIME` and `SETMTIME_AT` (`src/fs.rs` line 193, `:204`, `:214`) contain `mtime`,
   which the spelled-out list expands, and the gate does not split `GETMTIME` into components, so
   it misses them; the rule still applies. No item in the crate carries a `#[doc(alias)]`, which
   the rule asks of every kept foreign spelling (the POSIX verb and errno names) and every
   spelled-out name a reader searches for by its abbreviation.
   Why it matters: `check / standalone` runs these at a ceiling of 0; a ratified name failing a
   check needs an exemption outside the `Name:` block, and an unratified one needs a ruling.
   Change: the names Decision B decides, provisionally; one rename lane applies them at the end,
   then adds the exemption for `blk::SIZE` with its reason and the aliases.
   Door: two-way before the first tag. Effort: S for the lane, after the ruling.
