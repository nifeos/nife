# P1: before the tag

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md).

## P1: before the first tag

6. `reply_errno` is not total, and the error range is unstated.
   Finding: `reply_errno` (`src/lib.rs` line 62) computes `(-r0) as i32`. At `r0 == i64::MIN` the
   negation overflows: a panic in a debug build, a wrap in release, by the language's rule (not
   executed). For any negative word below `-2^31` the cast truncates to an unrelated errno. The
   contract says "a reply's first word is an `i64`: a non-negative result, or the negated errno"
   and bounds the errno nowhere; `reply_err` (`src/lib.rs` line 46) accepts any `i32`, 0 and negatives
   included, and `reply_err(0)` is 0, a success. Callers: the std client's `errors.rs`, both
   caretakers.
   Why it matters: a decoder on a wire is the one function that must be total. Linux bounds its
   convention (`MAX_ERRNO` 4095) for this reason.
   Change: Decision C states the range. Then `reply_err` asserts `1..=MAXIMUM_ERRNO` as `blk::req`
   asserts its count. `reply_errno` reads `-MAXIMUM_ERRNO..=-1` as that errno and any other negative
   word as the protocol-violation errno the decision picks. The test pins `i64::MIN`,
   `-(MAXIMUM_ERRNO + 1)` and `-1`.
   Door: the range is a contract statement, Decision C; the code is two-way. Effort: XS.

7. The packers mask where the contract names truncation as the worst failure.
   Finding: `fs::req` (`src/fs.rs` line 351) masks `handle` to 16 bits and `len` to 40.
   `fs::rename_dst` (`:368`) does the same. `grant::spec` (`src/grant.rs` line 116) masks `len` to 8
   bits, so `spec(256, rights)` carries length 0, which is `WHOLE_NAMESPACE` (`:143`).
   `xattr::spec` (`src/xattr.rs` line 82) masks `value_len` to 32 bits. The `const` block at `src/fs.rs` line 318 says a
   truncated length "would arrive at the server as a smaller successful read rather than as an
   error", and `blk::req` (`src/blk.rs` line 73) is the model: it asserts its count. The callers of
   `grant::spec` pass `name.len()` after their own checks (`components/src/swish/running.rs` line 871
   and others; whether every caller checks first is unverified).
   Change: `assert!` in each `const fn` on the field's range, as `blk::req` does, with a
   `#[should_panic]` test each; or return `Option` where a caller can act on it.
   Door: two-way. Effort: S.

8. The proofs cover one `&`, and nothing else.
   Finding: three harnesses in `src/dir.rs` line 221, all over `Rights`; the grandchild patch's own
   header says it adds no falsification the first does not catch. Nothing proves seven properties.
   First, `fs::req` and `rename_dst` round-trip for every in-range opcode, handle and length, and no
   field bleeds. Second, `xattr::reply` is non-negative for every kind and length and round-trips
   with `reply_kind` and `reply_value_len`. Third, `blk::req`/`req_blocks` round-trip in `1..=16`.
   Fourth, `grant::pack_name`/`unpack_name` round-trip at every length to 16. Fifth, the four
   walkers (`dirent::iter`, `nameset::iter`, `xattr::list::iter`, `xattr::store::iter`) consume
   their input and terminate on every buffer up to a page. The recorded mutation runs show this is
   the live failure class (ten timeouts, all iterator cursors). Sixth, `verb::of` never indexes out
   of bounds. Seventh, `parent.attenuate(parent.resolve(x))` never widens, which is the server's
   actual path. Each is a bounded bit-vector or small-buffer harness Kani finishes in seconds.
   Change: one harness per property with a falsification patch each, in `src/<module>/`
   `verification.rs` or beside the code as `dir` does; the README's "What is proven" grows a line
   per property.
   Door: two-way. Effort: M.

9. The equivalent mutants need a home, or the full run is red.
   Finding: the last recorded census for this crate is 548 caught of 588 viable (93.3 percent). It
   has 38 to 41 survivors classified as equivalent in four groups (`|` over bit-disjoint fields,
   unions of distinct single bits, `1 << 0` under `>>`, the `const _` loop bound). The
   classification sits in `notes/mutation-testing/regressions-...-filesystem-contract.md` and
   `census-...-triage-part-2.md`. The template's `.github/mutants-exempts` starts empty and the
   release gate reads the full run's verdict. The `const _` loop gap is since closed:
   `in_opcode_order` is a function with a test that feeds it a bad table (`src/verb/tests.rs` line 17).
   Change: carry the classification into `.github/mutants-exempts` with one reason per group, or
   kill the killable ones; dispatch `mutation.yml` once on `main` before the tag. The crate's
   tests already carry "pinned after a mutation run" sections in every module.
   Door: two-way. Effort: S.

10. The README is short of the scaffold, and three prose gates refuse it.
    Finding: no badge row under the title (`readme_badges.py` fails on its absence). There is no
    `rustup target add` before the bare-metal build (`README.md` line 76). The README gives no Kani run
    time and memory (6.6 seconds, 383 MB measured here), no single-harness command, and no word on
    what a passing run prints. The falsification command is wrong after the move (item 3). The
    minimum-Rust-version section gives no check command. `passive_voice`: 5 passives in 754 words (`README.md` line 3,
    `:10`, `:20`, `:36`, `:92`) against a cap of 1. `prose_caps`: a sentence of 82 words and 5.5
    bold spans per 1,000 words. The README's section order passes `readme_sections.py`.
    Change: use the scaffold's `README-crate.md` shape. Put four sentences into the active voice.
    Split the 82-word sentence (the verb list in "The contract"). Drop bold from two spans.
    Door: two-way. Effort: S.

11. Twelve acronyms are bare at first use.
    Finding: `prose_acronyms` (ceiling 0) flags POSIX, ONE and FS (`src/lib.rs` line 43, `:44`). It flags SMB
    (`:123`), DMA (`src/blk.rs` line 59) and EL0 (`:85`). It flags EOF (`src/fs.rs` line 34), NUL
    (`src/xattr.rs` line 51), FUSE (`:73`) and BFS (`src/xattr/tests.rs` line 44). It also flags
    READDIR and STATFS used as words in prose (`src/dirent.rs` line 28, `src/statfs.rs` line 25). ONE is emphasis in capitals, not an acronym.
    Change: expand each at first use or add the universally known ones (POSIX, EOF, NUL) to the
    repository's `prose_acronyms.toml`; write the verb names in code spans; lower-case ONE.
    Door: two-way. Effort: XS.

12. `rm` is not filesystem protocol.
    Finding: `src/rm.rs` defines the exit status and the report framing of one program, over a
    different contract (`byte_sink_contract`), and names both. Its only in-crate reader is the
    `dir::explain` cross-reference. Likewise `fs::CLIENT_WINDOWS` (`src/fs.rs` line 302) is a server
    capacity ("eight is provisional, a capacity decision not yet made") whose doc leans on a
    kernel badge mechanism; a stranger cannot tell whether a badge of 8 is a wire refusal or a
    deployment choice.
    Change: move `rm` to the `rm` program or to `byte_sink_contract` before the move (its two
    callers follow); state `CLIENT_WINDOWS` as a contract fact ("a server serves at least this
    many channels and refuses a badge at or above it") or move it to the server.
    Door: two-way before release. Effort: S.

13. Dates and British spellings in the shipped text.
    Finding: `src/verb.rs` line 369 and `:417` and `src/verb/tests.rs` line 245 narrate a change by date
    ("until", "since"). `src/verb.rs` line 119 and `src/verb/tests.rs` line 218 write the British form of "behavior"
    (`british_spelling` judges names only, so it passed; the rule covers words). `src/fs.rs` line 224
    carries a decision date outside a `Name:` block.
    Change: state the present rule (`EBADF` is about the handle, `ENOTDIR` about the request) and
    drop the dates; American spelling.
    Door: two-way. Effort: XS.
