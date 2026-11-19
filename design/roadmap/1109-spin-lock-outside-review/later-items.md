# P2: later items

An appendix to [the milestone](../1109-spin-lock-outside-review.md).

11. Say what the models do not check. `read` and `write` (`src/lib.rs` line 77, `src/lib.rs` line 87) DONE (crate documentation).
    return loom's pointer out of `with` and `with_mut`, which loom's documentation names as the
    pattern it does not track for duration. The causality check still fires on every access, which
    is the property the crate claims, and the review confirmed it. Fix: say so in the comment at
    `src/lib.rs` line 73 and in the README's "What is proven"; or hold loom's `ConstPtr` and `MutPtr`
    in the guard under `cfg(loom)` so the guard tracks the duration too. Effort S for the words, M for
    the pointers. Two-way.

12. `T: ?Sized`. std and `spin` both allow it; nothing here needs `Sized` except `new`. DONE (loom's cell is `?Sized` too).
    Relaxing the bound on the type and the guard is compatible. Effort S. Two-way.

13. `repr(C)` reads as a promise. `src/lib.rs` lines 46 to 50 says the flag sits at offset 0 so an DONE (the comment).
    inlined `lock` needs no offset arithmetic. The fields are private, so no caller can rely on the
    layout soundly, and the codegen claim is on nobody's record this review found (unverified).
    Fix: either cite a measurement in the comment or say the layout is an implementation choice
    and not part of the API. Effort S. Two-way.

14. Pick the `rust-version`. The crate builds and tests on 1.85, the edition's floor DONE (1.88, tested on it).
    (verified); every review package so far writes 1.88. Either is defensible; the number is the
    maintainer's. Effort S. Two-way.

15. Loom's lockfile footprint. In a repository of its own the crate's `Cargo.lock` holds 31 DONE (the README says why).
    packages, 30 of them loom's tree (`tracing-subscriber`, `regex-automata`, `generator`,
    `windows-sys` among them). `cargo deny` passes them (verified), and Dependabot will open bumps
    for a tree only the model run links. No action beyond awareness; the README says why the
    lockfile is large. Effort S. Two-way.

16. Considered and not recommended. `is_locked` (not std; `try_lock` answers it) and `try_lock_weak` DECLINED.
  (no caller). Also not recommended: a `lock_api::RawMutex` impl (a third-party dependency, which
  needs its own ruling and would give loom nothing), and a fair or ticket variant (the kernel's
  sections are short and the layer above this crate masks interrupts).
