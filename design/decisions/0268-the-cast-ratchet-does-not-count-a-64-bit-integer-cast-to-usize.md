---
status: DECIDED
raised: 2026-10-09
decided: 2026-10-09
ratified_by: calef
---

# 268. The cast ratchet does not count a 64-bit integer cast to `usize`

It answers
[the cast proposal](../roadmap/0895-narrowing-casts-and-unchecked-arithmetic-on-addresses-are-counted-and-fall.md)'s
first open question.

## The ruling

calef ruled on 2026-10-09 (UTC) that the cast count filters out `u64` and `i64` to `usize`, and
that the filter gets a short decision of its own rather than stretching §61 (a lint is adopted on
evidence from this tree).

## What this decides

The lint ratchet counts `cast_possible_truncation` hits, and before it counts it drops every hit
whose message names a cast from `u64` or `i64` to `usize`. No other class is filtered. A selftest
fixture proves the filter drops exactly that class and nothing else.

## Why

§61 dropped `cast_possible_truncation` on 2026-08-03 because 199 of its 497 hits were this class.
Those warn about 32-bit pointers, and §19 (architectural parity is a tenet) names three 64-bit
targets only. The ratio held on 2026-10-09: 340 of 844 hits, 143 of the kernel's 271. Clippy still
has no setting for pointer width.

§61's objection was to a `-D warnings` gate over every hit, which a reader learns to skim. The
ratchet reads clippy's JSON, where each message names both types, so it can drop the inapplicable
class before a reader sees it. What remains is 504 genuine narrowings in scope, 128 in the kernel.

## What it does not change

§61 holds as written. `cast_possible_truncation` stays out of `[workspace.lints]` and out of
`-D warnings`. This section adds a filtered count beside that ruling, in one pass, over the 16
files the cast proposal scopes.

## Revisit when

A supported target has a pointer narrower than 64 bits. The filter is then wrong on that target,
and this section is superseded.
