# P1: before the tag

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

## P1: before the first tag

4. The writer and three readers each spell out the verdict grammar, and they share no code.
   Finding: the kernel builds `nife self-test: X of Y passed[, N absent: a b][, M FAILED: c d]`
   by printing pieces (`kernel/src/self_test.rs` line 160 to 178). `serial_console` parses it with
   `verdict_total`, `verdict_absences` and a `find(SELF_TEST_FAILED)` (`crates/serial_console/
   `src/progress.rs` line 586` to 618 and 627 to 646). `xtask` parses it again
   (`xtask/src/stick.rs` line 414 to 421). Each reader encodes the grammar's separators (`of`, `,`,
   whitespace) itself. The crate exports the heads and nothing that formats or parses.
   Why it matters: the crate's reason to exist is that two binaries agree by construction (its own
   crate doc, lines 3 to 8). The heads agree by construction; the tail's grammar agrees by
   inspection. A kernel change to the separator between the absent list and the failed count would
   pass the kernel's tests and the readers' tests separately.
   Change: a `Verdict<'a>` type in the crate with `passed`, `total`, `absent: &'a str`, `failed:
   &'a str` (borrowed, `no_std`, no allocation), a `core::fmt::Display` the kernel prints through
   `print!`, and a `Verdict::parse(&str) -> Option<Verdict>` the readers call. The kernel's
   `print_names` keeps its no-buffer property by printing the names after the `Display` of the
   counts, or the type takes an iterator. `serial_console`'s three branches and `xtask`'s one
   become calls to `parse`.
   Door: two-way (additive API; the printed bytes do not change). Effort: M.

5. Nothing checks the check list's invariants.
   Finding: `SELF_TEST_CHECKS` (`src/lib.rs` line 84 to 100) is a plain slice. The kernel finds a
   check's index with `position` (`kernel/src/self_test.rs` line 98 to 104), so a duplicated name
   would leave the second entry never `seen` and the verdict would name it as `did not run` on
   every boot. A name with a space or a comma would split the readers' `split_whitespace` and
   `split(',')` (`progress.rs` line 630 to 633). `SELF_TEST_CHECKS_EARLIER` (line 115) has no check
   that each entry is below the current length.
   Why it matters: the list is the contract's only variable part, every lane that adds a check
   edits it, and the failure modes above are silent on the kernel side.
   Change: a `const _: () = assert!(...)` block in the crate that walks the list: non-empty,
   unique, bytes in `[a-z0-9_]`, and every entry of the earlier-lengths list below the current
   length and ascending. This is rung one of the mechanism ladder and costs nothing at run time.
   Door: two-way. Effort: S.

6. The crate has no tests and no proofs, and the template passes it vacuously.
   Finding: `cargo test` runs 0 tests. The template's `coverage` job skips a file with zero
   executable lines (`helpers/coverage-floor.sh`, header, lines 9 to 11), and `proofs / falsify`
   prints "no `#[kani::proof]` harness under `src/`; nothing to falsify" (`helpers/falsify.py` line 307)
   and passes. So a repository of this crate shows green `tests`, `coverage`, `kani` and
   `falsify` checks that proved nothing.
   Why it matters: a reader takes the green as a claim. A stranger expects the checks to mean something.
   Change: tests that pin the contract's shape: every head except the shell banner ends in a space
   or punctuation (the boundary claim of the crate doc), no head is a substring of another, the
   invariants of item 5. When item 4 lands, a round-trip test (format then parse) and one Kani
   harness that `parse` never panics on any input up to a bounded length, with its falsification
   patch, so the proof jobs have something to run.
   Door: two-way. Effort: S (M with item 4).

7. Eight sites in `xtask` spell `PROMPT`'s text as a literal.
   Finding: `"nife capability shell"` appears as a literal at `xtask/src/install.rs` line 130 and 185,
   `xtask/src/confirm.rs` line 54, `xtask/src/rollback.rs` line 145, `xtask/src/swish_check/launch.rs` line 206,
   `xtask/src/swish_check/transcript.rs` line 131, `xtask/src/swish_check/boot.rs` line 480 and
   `xtask/src/uefi_screen_modes.rs` line 52. Yet `xtask` already depends on the crate and uses
   `boot_ladder::PROMPT` at `xtask/src/install.rs` line 82. The crate's own `BUGS` (`src/lib.rs` line 12)
   says nothing gates this.
   Why it matters: a wording change to `PROMPT` passes every unit test and fails those gates as a
   timeout, which milestone 972 (the boot stick's shared names are code) names as the failure
   that reads like a hang.
   Change: in nife, replace the literals with the constant (the trailing period is the shell's
   tail, not the head), and add a `script/lint` grep that the head strings appear outside
   `crates/boot_ladder` only in `tests.rs` files and fixtures. That gate is nife's and moves rung
   two under the crate's `BUGS` entry, which then goes.
   Door: two-way. Effort: S. This item lands in nife, not in the crate.

8. Ten public names await ratification, and three of them say the wrong thing.
   Finding: `PROMPT` (`src/lib.rs` line 132) names the shell's banner line, and the crate's `BUGS` (lines 14 to 15) admits it. `SELF_TEST_CHECKS_EARLIER` (line 115) holds counts, not checks. `TOUR` (line 123) names the line `nife: the capability core runs on <arch>.` by an internal
   word a stranger has no way to read. The other seven say what they hold.
   Why it matters: a published name is one-way, and a name is the claim a reader meets first.
   Change: the decisions section below carries the recommendation; one rename lane applies the
   ruling, and `serial_console`'s `Stage::Prompt` and `Stage::Tour` follow the renamed constants.
   Door: one-way at release. Effort: S.

9. The crate doc's boundary claim is false for one head, and the crate never states the matching rule.
   Finding: lines 7 to 8 say every constant carries its own trailing space or punctuation so a match is on a boundary. `PROMPT` is `"nife capability shell"` with neither; the shell prints
   the period as part of its tail (`components/src/swish/entry.rs` line 97 to 98). Readers also disagree on how to find a head. `exposure.rs` line 139 uses `starts_with(BANNER)`. `progress.rs` line 485 uses `find(BANNER)` so that serial noise before the head still matches. The crate says nothing about which a reader may rely on.
   Why it matters: a stranger writing a reader needs the matching rule, and the one sentence that
   states it is wrong for one of eight heads.
   Change: correct the sentence (the shell banner matches whole, the rest are heads), and add one paragraph stating the rule a reader may rely on. A head begins a line, possibly after
   bytes a serial link corrupted, so a reader finds it anywhere in a line and takes the tail from
   there. Moving the period into `PROMPT` would make the claim true instead, at the cost of a wire
   change; the decisions section lists it.
   Door: two-way for the text; see decision 1 for the constant. Effort: S.
