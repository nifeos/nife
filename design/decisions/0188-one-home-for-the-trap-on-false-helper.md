---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 188. What the lifted `fn check(ok: bool)` is called, now that nine programs write it out by hand

Ruled 2026-10-28 (UTC): `require`: `user_mode_runtime::require` is the public name for the lifted
`fn check(ok: bool)`, matching the kernel's `trust::require` semantics ("not recoverable and must
not be recovered from") at one privilege level up. Milestone 408's provisional `Name:` line is
ratified.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 408 (one home for `fn check(ok: bool)`)'s
`DECISION` gate naming no section. Filed 2026-09-14 by milestone 291's lane, which added seven of
the nine copies and said so rather than leaving the count to be re-derived. *(Section number
provisional until the merge queue lands it.)*

## What is being decided

The name, and only the name. Whether the function is lifted is already decided by §94 (what may live
in a userspace library). What is open is what a public item on `crates/user_mode_runtime` is called.
That is the most-read function name this tree could add, since every program in the tree links that
crate.

## Is the premise true

Checked 2026-09-19 in this worktree. `grep -rn 'fn check(ok: bool)'` finds nine files:
`components/src/block_driver.rs`, `fixtures/src/call_server.rs`, `console_test_client.rs`,
`frame_revoker.rs`, `fs_test_client.rs`, `hello.rs`, `page_frame_producer.rs`,
`rendezvous_minter.rs` and `rendezvous_peer.rs`.

Every one is the same three lines, and they all mean *this program's only way to say no is to die
where the mistake was, because a failed check must be indistinguishable from a broken program*.

Two of the nine already disagree about what saying no means, which is the drift rather than a
prediction: `components/src/block_driver.rs:51` reaches the trap through `panic!()` where the rest
call `user_mode_runtime::trap()`. Same instruction, different path.

**And `script/lint` check 5 will never see this**, because nobody shared a file: each copy was typed
again, so there is no `#[path]` module to count consumers of. That is the cheaper failure of the two
and it is still a failure.

## What this tree already does in the analogous case, which decides everything but the name

§94 is the ruling and it is exact. It asks what the language forces to be per-binary and lifts
everything else. A property that attaches to the final link (`#[panic_handler]`,
`#[global_allocator]`, `_start`) genuinely cannot be an item in a shared crate, and the mechanism it
is built out of usually can. `user_rt::trap()` holds the instruction; `user_rt::panic_handler!()`
expands to the handler where the language requires it.

§94 also states the tell, and this is that tell exactly: **"a per-binary item whose body is copied
verbatim into every binary. If the body is identical everywhere, it is not per-binary; only its
declaration is."** It was written about 58 copied panic handlers. Nine copied `check`s is the same
shape at a seventh the scale.

So this section does not re-decide the lift. It asks the one question §94 left open for each
instance, which is what the lifted item is called.

## Why 291 chose the duplicate, recorded rather than criticized

The alternative was adding a public function to `user_rt` in a lane already touching two archive
tables and the filesystem's directory geometry, and `user_rt` is the crate every program links. The
existing two copies were the tree's established pattern, and following it kept the change
reviewable. That is an argument from effort and it was the right call for that lane; nine is past
the point where a pattern is a convention.

## The candidates, with the refusals

| | argument |
|---|---|
| `check` | what the nine copies are called, so zero call sites change. **It is one of the generic words `design/naming.md` names as a failure mode**: half the tree checks something, and a public item on the crate every program links is the worst place to spend a vague word. |
| `require` | the kernel's own word for the same shape. `kernel/src/trust.rs:62`'s `require(name, bytes)` prints what it expected, what it measured, and halts, and its doc says in as many words that a mismatch "is not recoverable and must not be recovered from". That is the same semantics one privilege level up, which makes the word a recognition rather than a coinage. |
| `insist` | unusual enough to need explaining, which is the jargon failure §39 refused `linedisc` on. |
| `must` | reads as a modal verb at the call site (`must(x == y)`), so the line parses as a fragment rather than a call. |

## Recommendation

`require`, on the recognition argument: the kernel already spends the word on "a mismatch that must
not be recovered from", and a reader who has met `trust::require` meets the same idea at EL0 under
the same name. The risk is the mirror of the reason, and it should be weighed. One word meaning two
related things at two privilege levels is either a recognition or a collision. §31 (the
foreign-language seam)'s refusal of `witness` and milestone 440 (what `script/ci-build` with no
arguments should mean)'s refusal of `runner` are both cases where this tree decided a second sense
cost more than it bought. The difference here is that the two senses are the *same* idea rather than
two unrelated ones.

## Would we still choose this if both options cost the same

Yes. `check` is the cheaper option (nine call sites already say it, so lifting it renames nothing)
and it is still the one this section refuses, on the generic-word rule.

## How reversible, and who has acted on it

Mechanically trivial and expensively public. Nine call sites today, and every future program
that links `user_mode_runtime`, which is all of them. Nobody outside this tree has acted on it, so
the cost is entirely what a reader learns, which is the category AGENTS.md says to spend deliberation
on rather than speed.

## State

Milestone 408 (one home for `fn check(ok: bool)`) is BUILT: the lifted function is
`user_mode_runtime::require` (`crates/user_mode_runtime/src/process.rs`), the recommended name, and
it carries a provisional `Name:` line. The section stays PROPOSED until an architect ratifies that
name through `script/names --unratified`.
