# Rust file size

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

*(Appendix of [the register](../register-of-measures.md). Stem minted by milestone 841
(a ratchet on Rust file length, and its dashboard row) on 2026-10-08.)*

No Rust file is over 2,000 lines, and `script/lint` keeps it that way. The gate is
`helpers/file_length_ratchet.py`, for §266 (a Rust source file stays under 2,000 lines). Its
baseline, `design/file-length-baseline.tsv`, holds no rows, so every tracked `.rs` file outside
`vendor/` meets the ceiling outright and a file at 2,001 lines fails. The measure has no chart: at
zero, the gate is the whole record (calef, 2026-10-14 UTC).

## The measure counts comments on purpose

A file's size is its physical line count, `wc -l`. Comments, blank lines and inline `mod tests` are
counted. That is §266 section 1's own ruling: a reader loads every line of a file, and comments are
41% of the non-blank Rust here. A measure that skipped them would hide most of what the reader pays.

That choice creates an incentive, and §266 names it where the rule is. A file at 2,050 lines can
pass by deleting 50 lines of comments, which would make the tree worse to buy a number. No gate can
tell a stale comment from a load-bearing one, so that is a review question. The ratchet's failure
message says the remedy: split the file along a seam.

## BUGS

- The gate cannot tell a split from a deleted comment. §266 makes that a review question.
