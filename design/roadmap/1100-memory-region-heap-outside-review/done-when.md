# Done when

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md).

- `script/crate-standalone memory_region_heap` passes step 0: the crate name and every public item
  read `ratified`, under the names Decision A rules.
- The manifest names its `talc` the way Decision B rules, the three nife dependencies are `git` and
  `rev` pins with `version` beside them, and the README states what the unpatched `talc` lacks.
- The repository has a README a stranger can follow, both license files, an explicit license line,
  a `rust-version`, and no comment that points outside the repository.
- The template's `check` and `proofs` workflows are green on the first pull request: no binding
  named `va`, `commit_all` returns a `Result`, and `coverage` measures a test.
- `initialize` refuses a second call and a misaligned or sub-minimum cap, and says so in its return
  value. `grow` returns `false` for a request of zero. A non-`OutOfMemory` refusal stops growth for
  good and is readable through one accessor.
- A `PageSource` seam exists with the syscall as its default. Host tests cover doubling, the cap,
  exactness at a power-of-two cap, both `realloc` paths, zero-size layouts and the three refusals.
  At least three Kani harnesses with falsification patches prove the arithmetic. std's heap module
  and `second_mount` consume the crate's policy instead of a copy.
- The README names the minimum useful cap and a test pins the figure. It says the heap never
  shrinks. The type's doc names the spinlock and the two re-entrancy rules.
- `PAGE` is `PAGE_BYTES`.
