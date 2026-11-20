---
status: DECIDED
raised: 2026-08-03
decided: 2026-08-03
ratified_by: calef
---

# 64. A per-file coverage number counts where tests are written, not what they reach

`components/src/swish.rs` had **zero** `#[cfg(test)]` blocks, and that was first reported here as
"the shell is untested". It was not. The shell was covered by ~28 QEMU integration cases driving the
real binary, and by 93 host tests in `crates/grant_plan`. That crate already held its parsing and
navigation.

0% was a true fact about a **file** and a false claim about a **component**. The milestone that
number prompted came out smaller and differently shaped once that was checked.

This matters here more than in most codebases, because this project deliberately splits logic from
IO across crate boundaries (§63 (the line between a program and its crate) above). That split is
what makes per-file coverage misleading. The tests live in the crate, and the file that has none is
the one holding the system_calls. A metric that counts per file will always report the healthy
arrangement as the broken one.

Coverage is still worth measuring. What is not worth doing is reading a per-file number as a
statement about a subsystem without first asking which file the tests would be in if the design were
right.
