---
status: BUILT
raised: 2026-10-03
built: 2026-10-03
promoted_from: riscv64-proofs-check-against-the-riscv64-model
milestone_dependencies: 589
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 635. riscv64 proofs check against the riscv64 model

Built 2026-10-03 (UTC). The gate was red in CI on the old link: `verify` run 37108539047, all seven
harnesses `arch "arm64"`. It was green after the patch fix, in `verify` run 37109308103 at
`41601b1e2`. There, 7 of 7 verified and every linked binary carried the model Kani wrote, with
`os "linux"` added by the link and not judged. On patagonia, CBMC run directly on both sets of linked binaries gave the same
property statuses and SAT variable and clause counts. The changed patch cost one cold Kani build in
CI, 140 to 145 s, about 3 minutes for the job. *(Title and slug are drafts.)*

This follows up the architecture overwrite that tautschnig traced in review of
[model-checking/kani#4913](https://github.com/model-checking/kani/pull/4913). The parent record is
[`notes/kani-upstream.md`](../../notes/kani-upstream.md).

## Part 1: What our riscv64 proofs are checked against today

**The riscv64 kernel row is checked against the host's machine model, not riscv64's.** Measured on
patagonia (Apple Silicon, macOS) on 2026-10-03 (UTC), with the cached build of
`patches/kani-0.67.0-riscv64-target.patch` and its CBMC 6.8.0. All seven harnesses of the
`kernel` row for `riscv64gc-unknown-linux-gnu` were run with `--keep-temps`, and the
`__CPROVER_architecture_*` symbols of each linked `.out` were read with `goto-instrument
--show-symbol-table`:

| symbol | Kani writes (`.symtab.out`) | CBMC checks (linked `.out`) |
|---|---|---|
| `arch` | `"riscv64"` | `"arm64"` |
| `os` | absent | `"macos"` |
| `char_is_unsigned` | 1 | 0 |
| `long_double_width` | 128 | 64 |
| pointer, `int`, `long`, `wchar_t` widths, endianness, alignment, `NULL_is_zero` | riscv64 LP64D | equal values |

The two differing C fields are the platform's own ABI, not a goto-cc quirk. Measured on patagonia
on 2026-10-03 with Apple clang 21.0.0: for `-arch arm64`, `sizeof(long double)` is 8,
`LDBL_MANT_DIG` is 53 and `char` is signed; for `--target=aarch64-linux-gnu`, they are 16, 113 and
unsigned.

CBMC agrees: every harness logs `Adding CPROVER library (arm64)`. The cause is the link order in
the patched Kani, the same as upstream's. `link_goto_binary` passes `kani_lib.c` to `goto-cc` as C
source after the Rust symbol table. `goto-cc` compiles it with the host's configuration, and that
configuration overwrites the one Kani wrote.

**Does it change a verdict? Not for any harness we have.** The same seven harnesses were run again
through a measurement shim that hands `goto-cc` a precompiled `kani_lib.c` object after the Rust
inputs. Every linked `.out` then reads `"riscv64"`, and CBMC logs `Adding CPROVER library
(riscv64)`. For each harness the set of checks, each check's status, and the SAT variable and
clause counts were identical under both models. The only difference was one program step per
harness, not traced; the extra `os` symbol's initialiser is the likely one. So for today's harnesses the overwrite
is cosmetic, measured rather than argued.

That result does not generalize on its own. The fields that differ only matter where CBMC
interprets C. That means the CPROVER library that `--add-library` compiles with the active
configuration, `kani_lib.c`, and any `extern "C"` model a harness reaches. Kani's Rust codegen
writes explicit bit widths and signedness, which the equal rows above confirm. The list of CBMC
consumers of these fields is from memory of `config.cpp` and `ansi-c`, not re-read for this note.
A future harness that reaches a C library model taking `char` or `long double` could verify
differently, and nothing would say so.

**CI, unverified.** The `prove the kernel on riscv64` job runs on `ubuntu-24.04-arm`, an aarch64
Linux host. By the same link order its linked model should read `arch` `"arm64"` and `os`
`"linux"`. aarch64 Linux has unsigned `char` and a 128-bit `long double`, so it should differ
from riscv64 only in those two names. This is reasoned from the macOS measurement and has not been
observed on the runner. The gate in section 2 confirms or refutes it on its first CI run.

**CI, observed 2026-10-03.** It confirmed it. The gate's first CI run, `verify` run 37108539047
(job `prove the kernel on riscv64`), was red on all seven harnesses with `arch` `"arm64"` and `os`
`"linux"`; every other field Kani wrote, `char_is_unsigned` and `long_double_width` included,
survived. So in CI the overwrite changed the architecture name and CBMC's library selection, and
nothing C code would see.

## Part 2: The gate

`script/verify-riscv64` already refuses a run whose goto output did not land under the riscv64
triple. Extend that check: after the run, read `__CPROVER_architecture_arch` from every linked
`.out` newer than the run's marker, with the `goto-instrument` it already installs, and fail
unless every one reads `"riscv64"`. Today that gate fails on every harness. That is the point:
it can come back red, and it has to go green only because of the fix below.

## Part 3: The fix

Precompile `kani_lib.c` to a goto object once, and pass it to `goto-cc` after the Rust inputs in
`link_goto_binary`. When `goto-cc` links goto binaries, the first input's architecture symbols win,
so the model Kani wrote survives. The shim in section 1 is this fix, applied from outside.

- In tree: change `patches/kani-0.67.0-riscv64-target.patch`. Editing the patch rebuilds the
  cached Kani in CI once, which is the cost.
- Upstream: the follow-up pull request promised on #4913, in
  [the reply to tautschnig](https://github.com/model-checking/kani/pull/4913#discussion_r4172069063).
  It changes the linked model for host runs too, which is why it is separate from #4913. When it
  ships in a Kani release, the in-tree patch loses this hunk with the rest of the patch.

## Part 4: The fatal risks it bears on

[Fatal risk 2](../fatal-risks/README.md) ("The proofs prove trivia, and the real bugs live where
Kani cannot reach") is the direct one. Its reach argument counts the `arch/` subtrees that
milestone 589 (Kani can prove riscv64 from the hosts we already have) made provable. Section 1 is
a fact for that record: the riscv64 harnesses run against the host's C machine model. For the
seven harnesses that exist, no verdict depends on it, so it narrows no claim the entry makes
today. Without the gate it is a soundness gap in waiting rather than a hole.

Fatal risk 3 ("The tests do not test anything, and the quality is illusory") is measured by
mutation testing over the host crates, which this does not touch. It bears on 3 only through
principle 2's sentence that the proofs are among the gates that make the method work.

## Follow-on

- **Done.** The gate green in CI after the patch fix, `verify` run 37109308103.
- **Recorded.** A `--c-lib` given as C source still overwrites the model, and `os` is not in Kani's
  model, so the linked binary names the host's OS. Both are written at the hunk in
  `patches/kani-0.67.0-riscv64-target.patch` and in the risk 2 appendix; nife passes no `--c-lib`.
- **Done.** The upstream pull request is drafted, not posted, in
  `notes/kani-upstream/kani-lib-link-order.md`; posting it waits on calef reading every word.

## Index row

The riscv64 kernel proofs were linked against the host's C machine model, because Kani's
`kani_lib.c` link overwrote the riscv64 configuration it wrote. A gate in `script/verify-riscv64`
now fails unless every linked goto binary carries the riscv64 model, and the carried Kani patch
links `kani_lib.c` precompiled and last so it does; red in CI before the fix, green after. Risk 2's reach argument rests on these proofs.
