# Function length

*(Appendix of [the register](../register-of-measures.md). Stem provisional, minted by milestone 888
(a ratchet on function length) on 2026-10-11 (UTC).)*

No function in scope is over 100 code lines, and `script/lint` keeps it that way. The gate is the
lint ratchet, `helpers/lint_ratchet.py`, for §283 (a function is held to 100 code lines by a lint
ratchet). Its baseline, `design/lint-baseline.tsv`, holds no function-length rows, so a function
over 100 code lines fails outright. The measure has no chart: at zero, the gate is the whole record
(calef, 2026-10-14 UTC).

## The measure is clippy's

A function's length is the count clippy's `too_many_lines` reports: the lines of its body, braces
excluded, that carry code once comments and blank lines are skipped. `helpers/function_length.py`
reproduces it from source text, so the gate can judge code no clippy pass compiles. Measured at a
threshold of 0 on 2026-10-13 (UTC) over the lint ratchet's eight clippy passes, the two agreed on
11,209 of the 11,217 functions clippy reported; the 8 others are under BUGS. The lint ratchet
compares them on every function clippy reports over 100, and fails on any difference. A closure is
not a function here: its lines belong to the function around it.

## The scope

Every tracked `.rs` file outside `vendor/`, test code and `#[cfg(test)]` items included (§283,
calef, 2026-10-13 UTC). `helpers/function_length.py`'s `in_scope` is the one scope function, and
the gate reads it. The text measure also reaches code no clippy pass compiles, such as the kernel's
build-mode features, the UEFI loader and the exercisers.

## BUGS

- The function finder is a lexer, not a parser. It skips a `fn` written in a `macro_rules!`
  body, where clippy reports it: 8 functions on 2026-10-13 (UTC), all under 10 code lines. The lint
  ratchet's comparison fails if one ever grows past 100.
- None of the eight clippy passes compiles about 1,160 functions in 127 files: `patches/`,
  `c_library/`, `uefi_loader/`, the std exercisers, `jig/`, `cryptography_provider/`, kernel build
  modes, `notes/` and `bench/`. They build only for nife's std targets or in a separate step, so the
  text measure alone holds them, with no clippy cross-check.
- A function clippy compiles and the text measure counts over 100 while clippy counts it under would
  not be caught. At threshold 0 no such function existed.
