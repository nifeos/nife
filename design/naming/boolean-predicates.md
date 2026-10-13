# Functions that answer yes or no

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the prior
art behind the rule for boolean predicates, measured rather than recalled, and the cases where the
rule needed a judgement. It exists to verify or challenge the main page. The file's stem is a
provisional name; naming is an architect's.*

## The ruling

calef, 2026-09-24, reviewing the provisional name `tick_pending`: *"I want consistency with rust,
which tells me we need to make a consistency pass."* The tree had no written rule and used both
shapes: `is_mapped(addr)` and `is_enabled()` beside `enabled()`, `crash_disk_present()`,
`local_apic_ready()` and `rx_waiting()`.

## What "Rust's convention" turned out to be

The premise needed checking first, because Rust has no written rule for this. The Rust API
Guidelines' naming chapter ([source][guidelines], read 2026-09-24) has seven rules: casing
(C-CASE), conversions (C-CONV), getters (C-GETTER), iterator methods and types (C-ITER,
C-ITER-TY), feature names (C-FEATURE) and word order (C-WORD-ORDER). None is about predicates. The
one `is_` in the chapter, `is_xid_start`, illustrates how an acronym is lower-cased in
`snake_case`. [RFC 430][rfc430], which finalised the casing conventions, carries the same example
for the same purpose, and [RFC 344][rfc344] is silent on booleans too.

So the convention is what `std` does, plus one lint that assumes it:

- Clippy's `wrong_self_convention` ([source][clippy], read 2026-09-24) holds `is_` to taking
  `&self`, `&mut self` or no `self`. It checks the receiver of an `is_` name. It does not require
  `is_` on a function returning `bool`, and no lint in clippy does.
- `std`'s own names, counted. Against the `rust-src` of `rustc 1.100.0-nightly (923c95cdf
  2026-09-16)`, every `pub fn ... -> bool` in `core`, `alloc` and `std`, outside tests, `sys`
  and the intrinsics: 342 functions, 236 of them (69%) `is_`. Next come 45 relations
  (`contains`, `contains_key`, `starts_with`, `ends_with`, `eq` and `ptr_eq` and
  `eq_ignore_ascii_case`, `exists`, `needs_drop`, `will_wake`), 3 `can_`, and 2 `has_`
  (`Path::has_root`). The count reads source with a regex, so it includes some unstable items.

**The remaining 56 are mostly not predicates at all.** They are actions that report what happened:
`HashSet::insert` and `remove`, `Vec::pop_if`, `AtomicBool::swap`, `fetch_and`, `load`. A `bool`
return does not make a function a question, and std does not prefix those. That is the exemption
the main page gives for `push`, `claim` and `take_`.

What is left is std's honest exceptions, all from before 1.0 or from `Formatter`:
`ExitStatus::success`, `WaitTimeoutResult::timed_out`, `Permissions::readonly`,
`thread::panicking`, and `Formatter::alternate`, `sign_plus` and `sign_minus`. Newer stable API
does not repeat them: `Option::is_some_and` (1.70), and `Option::is_none_or` and
`slice::is_sorted` (both 1.82). The one recent counter-example is honest to record: the unstable
`FormattingOptions` spells its flags `get_alternate` and `get_sign_aware_zero_pad`, because each is
half of a getter and setter pair (C-GETTER's `get`, beside `alternate(bool)` as the setter). The
rule follows what std converged on for a question, not what it kept for compatibility.

## Why a verb phrase is allowed

std's relations read as a sentence with the receiver as subject: `set.contains(x)`,
`path.starts_with(p)`, `waker.will_wake(other)`. A finite verb already says "this is a question",
which is the job `is_` does for an adjective. So `rights.allows(needed)`, `entry.overlaps(other)`
and `user_can_read(va)` fit as they stand, and forcing them to `is_allowing` would be less Rust,
not more. The test is grammatical: a finite verb (`allows`, `was_already_up`,
`machine_has_no_rtc`) passes, and an adjective, participle or noun standing alone (`enabled`,
`pending`, `truncated`, `owned`) takes `is_` or `has_`. A leading quantifier (`all_`, `any_`)
counts as std's `Iterator::all` and `any`.

## Judgements the rule needed

- Rust has no `are_`. A plural subject still takes `is_` or `has_`, the way `slice::is_sorted`
  does: `has_clocks_running`, not `are_clocks_running`.
- Adding `is_` sometimes leaves a name that does not parse, such as `is_shift` for "is a shift
  key held" or `is_aarch64` on a set of legs where `All` also answers yes. Those need a word
  chosen, which is a naming decision rather than the rule's application. The worklist lists each
  one with a recommendation and does not rename it.
- **A name that shadows std or another project stays**: our `Permissions::readonly` in the `std`
  overlay is std's own method, and `page_frames`'s bitmap `get(i)` follows C-GETTER's `get`.
- `is_` on a by-value receiver is fine for `Copy` types and warns under clippy otherwise, so a
  rename of a `self` method should expect the lint and take `&self` if the type is not `Copy`.

## BUGS

- **Nothing gates the rule.** A check that every `-> bool` starts with an allowed prefix would fire
  on every action that reports success, and telling a question from an action needs the doc
  comment read. It is rung three: written where a person naming a function looks.
- The std census is a regex over source, not a rustdoc query, so the percentages are close
  rather than exact.

[guidelines]: https://github.com/rust-lang/api-guidelines/blob/master/src/naming.md
[rfc430]: https://github.com/rust-lang/rfcs/blob/master/text/0430-finalizing-naming-conventions.md
[rfc344]: https://github.com/rust-lang/rfcs/blob/master/text/0344-conventions-galore.md
[clippy]: https://github.com/rust-lang/rust-clippy/blob/master/clippy_lints/src/methods/wrong_self_convention.rs
