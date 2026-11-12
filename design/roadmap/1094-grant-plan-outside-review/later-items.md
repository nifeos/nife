# P2: later

An appendix to [the milestone](../1094-grant-plan-outside-review.md).

## P2: later

15. `NameSet`'s `Debug` prints byte lists.
    Finding: `Name` debugs as the name, `NameSet` as `[[97, 46, ...]]`, because `iter` yields
    `&[u8]`. The baseline triage recorded this as a deferred finding and pinned it as is.
    Evidence: `src/expand.rs` lines 103 to 108 and 749 to 752.
    Change: render each name through `Name`'s impl (or `from_utf8` with the byte fallback) and
    change the pinned test on purpose.
    Door: two-way. Effort: XS.

16. `job_operand` and `kill_operands` return `Result<_, ()>`.
    Finding: both carry `#[allow(clippy::result_unit_err)]` and answer `Err(())` for a tail that
    names no job. Every other failure in the crate is a typed `Refusal` or `nav::Refused` with a
    sentence; these two leave the shell to invent one.
    Evidence: `src/parsing.rs` lines 204 to 245.
    Change: a `Refusal` variant (or a small `JobOperand` error enum with a `message`), before the
    release while the signature is free to change.
    Door: two-way before the release. Effort: XS.

17. `Windows::set_bound` and `is_bound` shift by an unchecked window number.
    Finding: `1 << w` with `w` a `u64` the caller supplies; `new` bounds the pool but `set_bound`
    and `is_bound` take any `w`. In a release build a `w` of 64 or more wraps to a low bit, so
    a wiring mistake in the progenitor would set another window's bound bit; in a debug build it
    panics.
    Evidence: `src/job_windows.rs` lines 101 to 112.
    Change: `debug_assert!(w < MAXIMUM_WINDOWS as u64)` or a `bool` return on an out-of-range
    window, and a test. The module stays in nife under item 1, so this is the shell's and the
    progenitor's to take.
    Door: two-way. Effort: XS.

18. Tests pin only thirteen of twenty-eight wire ids.
    Finding: `the_wire_ids_already_shipped_never_move` lists ids 0 to 12; ids 13 to 27 have
    shipped since in images and nothing but the table itself pins them.
    Evidence: `src/program/tests.rs` lines 18 to 32; `src/program/catalog.rs` lines 281 to 435.
    Change: extend the table to every id the image has shipped. The catalog stays in nife under
    item 1, so this is nife's test.
    Door: two-way (the ids themselves are one-way and are not touched). Effort: XS.

19. The crate's one example of use is a test helper.
    Finding: `test_support::plan_line` is the only place that runs `line::split`, `parse`,
    `redirect_target` and `plan_stage` together, which is how a shell uses the crate. It is
    `pub(crate)` and `cfg(test)`. The crate root's doc comment names the three entry points and no
    sequence.
    Evidence: `src/test_support.rs` lines 97 to 142; `src/lib.rs` lines 1 to 15.
    Change: an `examples/plan_a_line.rs` built from `plan_line`, with a `Manifest` written out in
    full (which also shows a stranger what a manifest is), and the README's `## EXAMPLES` section
    pointing at it.
    Door: two-way. Effort: S.
