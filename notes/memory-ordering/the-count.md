# The count, and how it was taken

*Name: provisional. After the parent's own heading.*

The population [`notes/memory-ordering.md`](../memory-ordering.md) adjudicates, and the method
behind the numbers. The counts are as taken for milestone 116 (the fences with no
partner). They have since drifted; the parent says by how much and which milestone re-derives them.

From the merged tree, outside test code:

| | |
|---|---|
| `Ordering::*` occurrences, total | 504 |
| ... in test code | 198 |
| ... outside test code | 306 |
| ... of those, `Relaxed` | 243 |
| ... of those, carrying acquire or release semantics | 63 (27 `Acquire`, 22 `Release`, 14 `SeqCst`) |
| `fence` call sites outside test code | 13 (11 at the 2026-09-0x count, plus milestone 243's two in `drivers/ramfb.rs`) |
| `compiler_fence` call sites outside test code | 1 |
| Files holding at least one non-test site | 45 |

Two things about that count are worth more than the number.

## A line-oriented grep gets it wrong

CLAUDE.md already warns about this from the `#[path]` module count. `rustfmt` splits a long
`compare_exchange` across five lines, so the ordering argument and the atomic it belongs to are
rarely on the same line. The scan behind this table blanks out comments and string literals first.
Then it walks backwards from each `Ordering::` to the open paren of the call that contains it. So a
multi-line call is attributed to the right atomic.

Checking the pattern against the real shapes first is the part that matters. Prose in this tree
says "Release" and "Acquire" constantly: `kernel/src/sync.rs`'s header, `redoxfs_server`'s
"Release a handle". A naive grep counts every one of them.

## The test-code exclusion hid a real fence

This is the place a scan goes wrong in the direction that hides things. The kernel's tests are
separate files pulled in with `#[cfg(test)] mod disk_tests;`, which has no brace. A scan that looks
for the next `{` after the attribute swallows the following 400 lines of live kernel, and quietly
classifies a real fence in `kernel/src/user.rs` as test code.

An attribute followed by a semicolon before any brace marks a whole file, not a region. Both forms
exist in this tree, and a scan has to know the difference.
