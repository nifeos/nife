# Draft: the `kani_lib.c` link-order follow-up to model-checking/kani#4913

*Name: provisional, for milestone 635 (riscv64 proofs check against the riscv64 model). The parent
note is [kani-upstream.md](../kani-upstream.md). **In force:** the change is in
model-checking/kani#4913 (`1eaa2d9`, with the Apple `char` fix in `9b72fa016`), per the ruling
below; no separate pull request is opened, and the branch plan below is not used. calef reads
every upstream word before anything goes to model-checking/kani. Measured 2026-10-03 (UTC).*

This is the pull request promised in calef's reply to tautschnig on #4913
([discussion_r4172069063](https://github.com/model-checking/kani/pull/4913#discussion_r4172069063)):
"I'd rather do the precompiled-object change as a follow-up, because it changes the linked model
for every run, host runs included (on macOS `char_is_unsigned` would go from 0 back to the aarch64
model's value)." tautschnig's comment it answers
([discussion_r4143913613](https://github.com/model-checking/kani/pull/4913#discussion_r4143913613))
traced the cause and proposed the fix.

**calef's ruling, 2026-10-03 (UTC): one pull request, not two.** The Apple `char` fix to Kani's
aarch64 model goes into this pull request. His reason: on its own the model fix changes nothing
observable today, because `goto-cc`'s overwrite hides Kani's model, so a reviewer would see a
one-line fix to a value that is never used, with its reason in another pull request. Together they
read as one change: keep Kani's model, and make that model right for Apple. It also keeps a
first-time contributor at two open pull requests. Effort was not the reason. The options weighed
are in this file's history (`git log -p` on it).

## What the measurements show

1. An Apple Silicon host run does not move `long_double_width`: Kani's aarch64 model uses 64 off
   Linux (#2757), and 128 is nife's riscv64 model. Only `char_is_unsigned` changes on this Mac, as
   calef's #4913 comment said.
2. macOS is not the only host that differs. x86_64 macOS matches Kani's model (by proxy, below);
   aarch64 Linux does not (`wchar_t_is_unsigned`: `goto-cc` 0, Kani 1).
3. Linux sees no change on x86_64, and a change on aarch64, where Kani's 1 is right.

## Branch plan

- Base: upstream `main` at `1640445da3fa0d2cb19739652057dc992fef3f18` (2026-10-02, "Stop a SIMD
  comparison with float mask lanes from aborting the crate (#4951)").
- Fork `calef/kani`, branch `kani-lib-link-order` (provisional). Cut from `main`, not from #4913,
  because it changes host runs and should not wait for or be reverted with #4913.
- One commit: [kani-lib-link-order.patch](kani-lib-link-order.patch), `git format-patch` form. To
  use it: `git checkout -b kani-lib-link-order 1640445da && git am
  notes/kani-upstream/kani-lib-link-order.patch`. 6 files, 99 lines added, 5 removed. Checked
  with `git am` in a scratch checkout of `1640445da`: it applies and gives the tree that was tested.

## The change

Two parts, one commit.

`kani-compiler/src/codegen_cprover_gotoc/compiler_interface.rs`, `new_machine_model`: for aarch64,
`char_is_unsigned = matches!(os, Os::Linux)` instead of `true`, with a comment citing Apple's arm64
ABI page. The model already splits `long_double_width` and `wchar_t_is_unsigned` the same way.

`kani-driver/src/call_goto_cc.rs`, `link_goto_binary`: the hunk nife carries as the second commit
of `patches/kani-0.67.0-riscv64-target.patch`, applied unchanged. It compiles `kani_lib.c` with
`goto-cc -c` to `<output>.kani_lib.o`, records that as a temporary, and passes it after the Rust
inputs and any `--c-lib`. The stale `TODO` ("kani_lib_c is just an empty c file") goes.

The mechanism was re-measured here with CBMC 6.11.0, using a C file compiled with `-funsigned-char`
as the stand-in for Kani's symbol table. Object then C source gives the source's model. Object then
object keeps the first; reversed, the second object's model wins.

## The test

`tests/script-based-pre/linked_machine_model/`: a one-harness file, run with `--keep-temps` in a
`mktemp -d` directory. For every `*.symtab.out` it reads each `__CPROVER_architecture_*` symbol
from that file and from the linked `.out` with `goto-instrument --show-symbol-table`, strips the
`(__CPROVER_integer)` cast, and requires every line of the first to appear unchanged in the second
(`comm -23`). Fields the link adds and Kani does not write (`os`, `argument_evaluation_order`) are
allowed. It fails loudly if no `.symtab.out` exists or one has no such symbols, so it cannot pass
vacuously. No host values are written into it.

It proves it can fail, on two hosts:

| host | build | result |
|---|---|---|
| patagonia (macOS arm64) | upstream `main`, no hunk | **red**, compiletest exit 1: `__CPROVER_architecture_char_is_unsigned 1` written, `0` linked |
| patagonia | link hunk only | **green**, exit 0 (`char_is_unsigned` 1 in both files) |
| patagonia | link hunk and model line, the patch as saved | **green**, exit 0 (`char_is_unsigned` 0 in both files) |
| arm64 Ubuntu 24.04 container | (a) the patch as saved | **green**, exit 0 (`char_is_unsigned` 1, `wchar_t_is_unsigned` 1 in both files) |
| arm64 Ubuntu 24.04 container | (b) the patch with only the link hunk reverted | **red**, exit 1: `__CPROVER_architecture_wchar_t_is_unsigned 1` written, `0` linked |
| arm64 Ubuntu 24.04 container | (c) upstream `main` plus only the test | **red**, exit 1: the same `wchar_t_is_unsigned` 1 written, 0 linked |
| GitHub `ubuntu-24.04` (x86_64) | (a), (b) and (c) | **green** each, exit 0; written and linked both `char_is_unsigned=0 long_double_width=128 wchar_t_is_unsigned=0` |

The container ran natively on patagonia (podman, `--platform linux/arm64`, `ubuntu:24.04`, the
image Kani's `ubuntu-24.04-arm` runner matches). Dependencies were installed the way Kani's
`scripts/setup/ubuntu/install_deps.sh` does, with two differences. CBMC 6.11.0 came from its
`ubuntu-24.04-arm64` release `.deb` (Kani's `install_cbmc.sh` builds it from source on arm64). And
`time` and `jq`, which GitHub's runner images carry, were added by hand. Toolchain,
kissat 4.0.1 and cvc5 1.3.0 came from Kani's own scripts. `cargo build-dev -- -j 3` exited 0 in 150 s with the patch.

With the model line, the red row on this Mac would have nothing to show, since Kani and `goto-cc`
would then agree here. The test still guards aarch64 Linux (`wchar_t_is_unsigned`) and any model
field that later diverges from a host's.

Kani's CI runs its regression on `macos-15-intel`, `ubuntu-22.04`, `ubuntu-24.04`, `macos-14` and
`ubuntu-24.04-arm` (`.github/workflows/kani.yml`). Without the link hunk the test was observed red
on macOS arm64 and arm64 Linux, and green on x86_64 Linux, where host and model agree. The x86_64
rows are nife's scratch workflow run [37156714124](https://github.com/nifeos/nife/actions/runs/37156714124) on a GitHub-hosted `ubuntu-24.04` runner
(2026-10-03, UTC): Kani's own setup scripts, CBMC 6.11.0's x86_64 `.deb`, the saved patch applied
with `git am`, then the three variants. x86_64 macOS was not run.

## Runs on patagonia, 2026-10-03 (UTC)

Apple Silicon, macOS 26 (Darwin 25.6.0), toolchain `nightly-2026-09-25` (Kani's pin). CBMC 6.11.0
and kissat 4.0.4 unpacked from the cached Homebrew bottles into the session scratchpad, cvc5 1.3.0
(Kani's CI pin) from its GitHub release zip, nothing installed. Every job niced, one at a time;
load average was 7 to 36 from other lanes throughout.

The saved patch is the second build below. The model line in it is byte-for-byte the one tested
(applied by the same script), so its runs stand for the patch and were not repeated.

| run | exit | result |
|---|---|---|
| `cargo build-dev -- -j 4`, upstream `main` | 0 | 186 s wall, 1.73 GB peak resident |
| `./scripts/kani-fmt.sh --check`, link hunk | 0 | clean |
| `rustfmt --check` on `compiler_interface.rs` with the model line | 0 | clean |
| `cargo clippy -p kani-driver -- -D warnings` | 0 | clean |
| new test, no hunk | 1 | red, as above |
| **Build 1: link hunk only** (`cargo build-dev` exit 0, incremental) | | |
| `kani` suite | 1 | 618 passed, 2 failed, 23 ignored. Both failures are `#[kani::solver(cvc5)]` harnesses (`Quantifiers/arbitrary_range`, `Quantifiers/typed_variables`); cvc5 was absent |
| those two, rerun with cvc5 1.3.0 on `PATH` | 0 | 2 passed |
| `expected`, `ui`, `script-based-pre`, `cargo-kani` | 0 each | 504, 153, 91 (plus the new test, skipped as up to date after its green run, which compiletest reports as ignored), 71 passed |
| **Build 2: link hunk and model line, the saved patch** (`cargo build-dev` exit 0, incremental) | | |
| `kani` suite, cvc5 present | 0 | 620 passed, 23 ignored |
| `expected` | 0 | 504 passed, 15 ignored |
| `ui` | 0 | 153 passed |
| `script-based-pre` | 0 | 92 passed; the compiler change invalidated every stamp, so all ran |
| `cargo-kani` | 0 | 71 passed |
| `git status` after the suites | | only the commit's own files; no `.kani_lib.o` left behind |

In the arm64 Ubuntu container, with the patch as saved, `script-based-pre` gave 87 passed, 4
failed, 1 ignored (exit 1, 838 s). Each failure was the environment, and each passed rerun alone
(exit 0). `cbmc_truncated_results` and `compiler_defaults` needed `jq`. `playback_expected` (CBMC
out of memory) and `std_codegen` (SIGKILL compiling std) ran short of the 6 GiB VM under parallel
tests; `std_codegen` passed with `CARGO_BUILD_JOBS=1`.
`target_riscv64` is #4913's test and does not exist on upstream `main`, so it was not run here.

On the x86_64 runner, with the patch, `script-based-pre` gave 91 passed, 1 ignored (the test just
run in (a)), exit 0. Not run: the other suites on either Linux, x86_64 macOS, `coverage`,
`cargo-coverage`, `std-checks`, firecracker.

## What changes for each host

Fields Kani writes that differ between Kani's model and `goto-cc`'s host configuration, which is
what the linked `.out` carried before this change. Every other field Kani writes was equal.

| host | before (goto-cc) | link hunk only | the patch | right answer | how measured |
|---|---|---|---|---|---|
| aarch64 macOS | `char_is_unsigned = 0` | 1 | 0 | 0: Apple clang leaves `__CHAR_UNSIGNED__` undefined, Rust's `c_char` is `i8`, CBMC's own `config.cpp` says signed for `macos`/`arm64` | Kani run, `.symtab.out` and `.out` |
| x86_64 macOS | none | none | none | | proxy: `goto-cc -arch x86_64` on this Mac against Kani's x86_64 model in source |
| aarch64 Linux | `wchar_t_is_unsigned = 0` | 1 | 1 | 1: gcc 13.3 on Ubuntu 24.04 arm64 gives `__WCHAR_TYPE__ unsigned int` | Kani run in an arm64 Ubuntu 24.04 container, `.symtab.out` and `.out` |
| x86_64 Linux | none | none | none | | Kani run on a GitHub `ubuntu-24.04` runner, run 37156714124; agrees with tautschnig's #4913 measurement |

The link hunk alone corrects aarch64 Linux and, on Apple Silicon, swaps a right value for a wrong
one because Kani's model was wrong there. With the model line, every host ends on the right value.

## Why the Apple `char` correction is in this pull request

### What Kani's model did per target

`new_machine_model` in `kani-compiler/src/codegen_cprover_gotoc/compiler_interface.rs` matches on
architecture, then on OS for two fields of aarch64 only (before this patch): `long_double_width` (128 on Linux, else
64) and `wchar_t_is_unsigned` (Linux only). `char_is_unsigned` was `true` for every aarch64 target.
x86_64 has no OS split and needs none. `check_target` accepts exactly four hosts:
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-*`, `arm64-apple-*`.

Prior discussion, read: in #1167 (Apple M1 support,
https://github.com/model-checking/kani/issues/1167) diagprov measured aarch64 Linux on
2022-07-20, AGSaidi cited AAPCS64 for unsigned `char`, and diagprov found `wchar_t` signed on an M1.
Nobody measured `char` on the M1, and the `wchar_t` split is what landed. #2757
(https://github.com/model-checking/kani/pull/2757, aarch64 Linux support) added the `long double`
split. Searches of issues and pull requests for `char_is_unsigned`, `long_double_width`,
`kani_lib.c`, "machine model macos" and "signed char apple" found nothing else on this.

### Is there a harness whose verdict changes?

None found. Kani's own `char` type (`CInteger(Char)`) is lowered with Kani's model at codegen, so
the link never touches Rust code. What the linked symbols decide is how CBMC parses its C library
(`goto-instrument --add-library`, "Adding CPROVER library (arm64)") and nothing in `kani_lib.c`
uses `char`. So the question is which CBMC 6.11.0 library functions depend on the signedness of
plain `char`, and whether a harness reaches them.

- Without `-Z c-ffi`, a harness reaches only the builtins Kani declares in
  `cprover_bindings/src/goto_program/builtin.rs` (`memcmp`, `memcpy`, `memset`, the math
  functions, and so on). `memcmp` and `memset` cast to `unsigned char` explicitly.
- With `-Z c-ffi`, more of the library is reachable. I grepped all 34 files of the CBMC 6.11.0 C
  library. The plain-`char` variables are in `strcpy`-style copies and `getline` (compared only with `'\0'`), `strtol` (compared with
  ASCII ranges, where a high-bit byte fails both ways), and `intrin.c` (MSVC interlocked
  intrinsics, Windows only). `strcmp`, `strcasecmp` and `strncmp` copy into `unsigned char` first.
- `--c-lib` given as C source still overwrites the model, so it is unaffected. Given as a `goto-cc`
  object, its code was typed when compiled, so only the library is reinterpreted.

I ran five harnesses: `[u8]` ordering over `memcmp`; `strcmp`, `strchr` and `strtol` on `0xff` under
`-Z c-ffi`; and Rust's `c_char` sign. Each gave the same verdict and the same SAT variable and clause
counts on upstream `main`, with the link hunk alone, and with the whole patch. So the Apple `char`
bug is wrong in the model and harmless in every case found. That is a property of CBMC's library
being written carefully, not a guarantee.

### What the model line costs

One line and a comment. No test in Kani's tree hard-codes `char` signedness on macOS, and every
suite in Build 2 passed. x86_64-apple-darwin needs nothing. A test for the model itself would have
to name host values, which is what the link test avoids; a unit test of `new_machine_model` would
need a `Session` and is more than the line it guards.

## Commit message

In the patch: "Keep Kani's machine model through the link, and make it right for Apple arm64". Its
claims match the measurements above.

## Pull request body, ready to paste

Title: Keep Kani's machine model through the link, and make it right for Apple arm64

> `link_goto_binary` passes `kani_lib.c` to `goto-cc` as C source. `goto-cc` compiles it with its
> own host configuration, and the `__CPROVER_architecture_*` symbols that produces replace the
> machine model Kani wrote into the symbol table. CBMC reads those symbols back when it adds its C
> library. @tautschnig traced this in review of #4913
> ([comment](https://github.com/model-checking/kani/pull/4913#discussion_r4143913613)), and this is
> the follow-up I offered there.
>
> **Keeping the model.** This PR compiles `kani_lib.c` on its own and passes the object after the
> Rust inputs. When `goto-cc` links goto binaries the first input's symbols win, so the model Kani
> wrote survives. Comparing each field Kani writes with what the link produced before, with CBMC
> 6.11.0, only two fields differed on any host:
>
> - aarch64 Linux: `wchar_t_is_unsigned` was 0 (`goto-cc`'s), and Kani writes 1. gcc 13.3 on
>   Ubuntu 24.04 arm64 has `wchar_t` as `unsigned int`, so keeping Kani's value is a correction.
> - Apple Silicon: `char_is_unsigned` was 0 (`goto-cc`'s), and Kani's aarch64 model wrote 1.
> - x86_64 Linux and x86_64 macOS: no field Kani writes differed.
>
> **Making it right for Apple.** Kani's aarch64 model set `char_is_unsigned` for every OS. Apple's
> arm64 ABI makes `char` signed: Apple clang 21 leaves `__CHAR_UNSIGNED__` undefined for `-arch
> arm64`, Rust's `c_char` is `i8` on `aarch64-apple-darwin`, and CBMC's own configuration for
> `macos`/`arm64` says signed. The model already splits `long_double_width` and
> `wchar_t_is_unsigned` on the OS for this reason (#2757, #1167), so this splits `char` the same
> way. It belongs here because the overwrite above hid the model's value: without the link change
> the line changes nothing, and without the line the link change would move Apple Silicon runs onto
> the wrong `char`. With both, every host checks against the right value.
>
> I could not find a harness whose verdict changes. The only C these symbols affect is CBMC's
> library, and the functions I checked (`memcmp`, and `strcmp`, `strchr` and `strtol` under
> `-Z c-ffi`) gave the same results and formula sizes before this PR, with the link change alone,
> and with both.
>
> **What it does not change.** `os` is not part of Kani's model, so the linked binary still names
> the host's OS. A `--c-lib` given as C source still overwrites the model; I left that alone and
> added a comment. Compiling `kani_lib.c` once in build-kani would save a process per harness but
> changes the bundle layout; happy to do that instead if you prefer.
>
> **Testing.** A new script-based test, `linked_machine_model`, compares every
> `__CPROVER_architecture_*` field in each harness's `.symtab.out` with the linked `.out`. It hard
> codes no values. It fails on `main` and passes with this PR on an Apple Silicon Mac (on
> `char_is_unsigned`) and in an arm64 Ubuntu 24.04 container (on `wchar_t_is_unsigned`). On
> arm64 Linux it also fails with the model change but without the link change. On the Mac, with this
> PR, `kani`, `expected`, `ui`, `script-based-pre` and `cargo-kani` pass, and so do
> `kani-fmt.sh --check` and clippy with `-D warnings` on `kani-driver`. In the arm64 Linux
> container, `script-based-pre` passes (four tests needed `jq` or less parallelism there and passed
> on rerun). On a GitHub-hosted x86_64 `ubuntu-24.04` runner the test passes on `main`, with the
> link change alone and with this PR, as expected where host and model agree, and
> `script-based-pre` passes with this PR.
>
> By submitting this pull request, I confirm that my contribution is made under the terms of the
> Apache 2.0 and MIT licenses.

Nothing in the body is from memory. Its two indirect claims are marked in the checklist: Apple's
ABI page (cited, not read here) and x86_64 macOS (measured by proxy).

## calef's checklist before posting

- [ ] When to post is calef's call. The coordinator recommends waiting until a maintainer has
      engaged on #4913, so a first-time contributor's second pull request does not land on an
      unanswered first one. That is a recommendation, not a ruling.
- [ ] `git am` the patch on `1640445da` (or rebase if `main` moved) and rerun the new test once.
- [ ] The code comment links Apple's "Writing arm64 code for Apple platforms" page; it is taken from
      CBMC's `config.cpp` comment and Kani's own source, not read here. Open it once.
- [ ] "x86_64 macOS: no field differed" rests on `goto-cc -arch x86_64` on an arm64 Mac, not on an
      Intel Mac. Kani's `macos-15-intel` CI job is the real check.
- [ ] The body says "I". Every run in it was made by an agent on patagonia, as recorded here.
