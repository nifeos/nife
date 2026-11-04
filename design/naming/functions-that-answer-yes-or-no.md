# Functions that answer yes or no

*Name: ratified 2026-10-18 (calef, "Yes"). Refused `boolean-predicates`.*

*An appendix to [`design/naming.md`](../naming.md). It holds the rule for a function that answers a
question, and the prior art it follows.*

## The rule

A function that answers a question and changes nothing takes one of these shapes, as Rust does:

- `is_` before an adjective, participle or noun phrase: `is_empty`, `is_tick_pending`.
- `has_` before a thing held (`has_unclosed_fence`), or `can_` before a verb (`can_read`).
- A third-person verb phrase, std's shape for a relation: `contains`, `starts_with`, `overlaps`,
  `allows`, `needs_drop`. A leading quantifier (`all_`, `any_`) reads as `Iterator::all` and `any`.

A bare adjective, participle or noun (`enabled`, `pending`, `present`, `truncated`, `read_only`,
`done`, `required`) reads as a getter, and std spells those `is_enabled`.

Exempt:

- A function that acts and reports whether it worked: `insert`, `push`, `claim`, a `take_` that
  clears a flag. A `bool` return does not make a function a question.
- Test functions, which read as sentences here, and fields (a field that is an array of booleans
  takes the predicate form: [shapes.md](shapes.md)).
- A trait method or a name another project owns: `eq`, std's `Permissions::readonly` in the `std`
  overlay, C-GETTER's `get`.
- An `extern` symbol that assembly calls by name.

## Why a verb phrase passes

std's relations read as a sentence with the receiver as subject: `set.contains(x)`,
`path.starts_with(p)`, `waker.will_wake(other)`. A finite verb already says "this is a question",
which is the job `is_` does for an adjective. So `rights.allows(needed)` and `user_can_read(va)` fit
as they stand, and `is_allowing` would be less Rust, not more. The test is grammatical: a finite
verb (`allows`, `was_already_up`, `machine_has_no_rtc`) passes, and an adjective, participle or noun
standing alone takes `is_` or `has_`.

## Cases that need a judgment

- Rust has no `are_`. A plural subject still takes `is_` or `has_`, as `slice::is_sorted` does:
  `has_clocks_running`, not `are_clocks_running`.
- Adding `is_` can leave a name that does not parse: `is_shift` for "is a shift key held", or
  `is_aarch64` on a set of legs where `All` also answers yes. Choosing the words is a naming
  decision, so the function carries a `/// Name: provisional` marker with a recommendation
  (`is_shift_held`, `includes_aarch64`) and joins `script/names --unratified`.
- A name that reads as a participle and acts (`removed` sends an unlink, `asked` prompts for a
  reply) passes as an action, but a verb (`remove`, `ask_to_confirm`) would say so more plainly.
- `is_` on a by-value receiver is fine for `Copy` types and warns under clippy otherwise, so a
  rename of a `self` method takes `&self` when the type is not `Copy`.

## What Rust's convention is

Rust has no written rule for predicates. The API Guidelines' naming chapter ([source][guidelines])
covers casing, conversions, getters, iterators, features and word order, and its one `is_` example
shows how to lowercase an acronym. [RFC 430][rfc430] and [RFC 344][rfc344] say nothing of booleans
either. Clippy's `wrong_self_convention` ([source][clippy]) checks the receiver of an `is_` name and
never requires `is_` on a function that returns `bool`.

So the convention is what std does. A regex census of every `pub fn ... -> bool` in `core`, `alloc`
and `std` (tests, `sys` and the intrinsics aside) found 342 functions: 236 `is_`, 45 relations
(`contains`, `starts_with`, `eq`, `exists`, `needs_drop`), 3 `can_` and 2 `has_`. Most of the rest
are actions that report what happened (`HashSet::insert`, `Vec::pop_if`, `AtomicBool::swap`). What
remains are std's older exceptions (`ExitStatus::success`, `thread::panicking`,
`Formatter::alternate`), and newer stable API does not repeat them: `Option::is_some_and`,
`slice::is_sorted`. The rule follows what std converged on for a question, not what it keeps for
compatibility.

## BUGS

- `helpers/name_checks.py` lists a `bool` function outside the shapes, but only judgment tells a
  question from an action, so the check reads a first word ending in `s` as a finite verb and a
  plural noun (`bits_set`) passes.
- The std census reads source with a regex, not rustdoc, so its counts are close rather than exact.

[guidelines]: https://github.com/rust-lang/api-guidelines/blob/master/src/naming.md
[rfc430]: https://github.com/rust-lang/rfcs/blob/master/text/0430-finalizing-naming-conventions.md
[rfc344]: https://github.com/rust-lang/rfcs/blob/master/text/0344-conventions-galore.md
[clippy]:
https://github.com/rust-lang/rust-clippy/blob/master/clippy_lints/src/methods/wrong_self_convention.rs
