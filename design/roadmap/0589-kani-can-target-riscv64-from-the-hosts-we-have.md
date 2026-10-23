---
status: PARTIAL
raised: 2026-09-24
promoted_from: kani-can-target-riscv64-from-the-hosts-we-have
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 589. Kani can prove riscv64 from the hosts we already have, with a 46-line patch and no riscv64 machine

*(Number minted at promotion, provisional until the merge queue lands it.)*
Promoted from the proposal `kani-can-target-riscv64-from-the-hosts-we-have`, filed 2026-09-24, after
calef ruled on it 2026-09-25 (UTC): option 1 adopted, recorded as
[§218 (carry a Kani patch so riscv64 is proved)](../decisions/0218-carry-a-kani-patch-so-riscv64-is-proved.md).
The carried half is built: [`patches/kani-0.67.0-riscv64-target.patch`](../../patches/kani-0.67.0-riscv64-target.patch)
and the `prove the kernel on riscv64` job (name provisional) in `.github/workflows/verify.yml`, which
builds the patched Kani on the arm64 runner and runs `script/verify --only kernel` for
`riscv64gc-unknown-linux-gnu`. The upstream half is model-checking/kani#4913; see `## Follow-on`. The text
below is the proposal's own, except for this paragraph, the gate, the `## Built` and `## Follow-on`
sections, and cuts at promotion to meet §212 (a prose budget) and §213 (writing standards). As filed: raised by the research lane `lane/kani-riscv64-host` (pull request
#1280). calef asked whether Kani could be fixed for riscv64, perhaps with `radon` as a native host
once bench runs are done. The maintainer added a second question mid-lane: what would containing
every `asm!` in thin wrapper functions buy? This file is a sibling of pull request #1276's proposal,
which found the `Unsupported architecture` panic.

§218 ruled the fork this proposal was gated on. As filed: every option that reaches
riscv64 changes the prover, either as a patch this tree carries or as a contribution upstream.
DECISIONS §46 (thin primitives or whole subsystems) makes that a dependency decision.

## The answer

A riscv64 host is not needed, and would not help. CBMC does not care which machine it runs on.
It checks a goto program, and the machine model is data written into that program. So what Kani
lacks is a riscv64 *target*, not a riscv64 *host*. A patched Kani 0.67.0 on patagonia (aarch64
macOS, stock CBMC 6.8.0) compiled the whole nife kernel for `riscv64gc-unknown-linux-gnu` and ran
its harnesses: three of four verified, one failed, 7.27 s real
([transcript](0589-kani-can-target-riscv64-from-the-hosts-we-have/transcripts.md)).

The failure is the point of the second probe. It calls `flush_asid`, which runs `sfence.vma`, and
Kani reports the reachable `asm!` as unsupported rather than refusing to compile. That is the same
behavior a native aarch64 `asm!` gets today. The first probe proves that `ttbr0_value` and `asid_of`
round-trip the ASID and the root for every page-aligned root below 2^56. It sits inside
`arch/riscv64/mmu.rs`, the largest riscv64 file, with its real `cfg` dispatch and its real
`crate::arch`. Nothing here is a fiction about which architecture is underneath.

A third probe stubbed `read_satp` (a one-instruction `csrr` wrapper) with a model and proved
`current_root_pa` extracts the root it was given.

## Part 1: What Kani needs per architecture, read in the source

Read at the `kani-0.67.0` tag (the version this tree runs locally) and at `cbmc-6.8.0`, the CBMC that
tag pins in `kani-dependencies`.

Kani, four places, all keyed to the host:

| where | what it does | host-bound how |
|---|---|---|
| `compiler_interface.rs` `new_machine_model` | builds CBMC's machine model | matches `Arch::X86_64` and `Arch::AArch64`, otherwise panics |
| `compiler_interface.rs` `check_target` | allowlist | `llvm_target` must be x86_64 or aarch64, Linux or Apple |
| `kani-driver/src/call_cargo.rs` | the `--target` Kani passes to cargo | `env!("TARGET")`, the triple Kani itself was built for |
| `tools/build-kani/src/sysroot.rs` | builds Kani's `std` and `kani` rlibs | `-Z build-std` for `env!("TARGET")` only |

The machine model is 20 fields: pointer width, endianness and alignment come from the rustc session,
and the C widths (`char` signedness, `long double`, `wchar_t`) are hard-coded per arch.
`cprover_bindings/src/env.rs` writes them into the goto program as `__CPROVER_architecture_*`
symbols.

CBMC already supports riscv64. `src/util/config.cpp` has `set_arch_spec_riscv64()`: LP64,
little-endian, 128-bit `long double`, unsigned `char`. `set_arch("riscv64")` dispatches to it, and
`set_from_symbol_table` reads the architecture string from Kani's symbols before overriding each width
from them. So CBMC needs no change and no rebuild. The stock macOS binary checked the riscv64 program
above.

The patch, measured: 46 lines added and 5 removed across the four files. The model arm is the
aarch64 arm with the RISC-V psABI's `wchar_t` (signed) and `long double` (binary128). The prototype
reads the target from a `KANI_TARGET` environment variable, which is a lab convenience, not a design.
It built in 244 seconds wall at four jobs on patagonia, 1.09 GB peak resident, 1.5 GB of `target/`.
It applies to Kani 0.68.0 (released 2026-09-16) with 9 of 10 hunks at an offset. The one rejected
hunk appends a function at end of file, a one-line fix.

One loose end: every riscv64 compile warns `target feature 'd' must be enabled`. Adding riscv64
features in Kani's `target_config` did not silence it. It is a warning today, and rustc says it will
become an error, so an upstream version has to find the right place to set it.

## Part 2: Two shapes: a target flag, or a riscv64 host

| | (a) target flag on today's hosts | (b) native Kani on a riscv64 host |
|---|---|---|
| Kani change | the four places above | three of them: the model, `check_target` and a bundle; only the driver's `--target` goes away |
| CBMC | stock, any host | built from source: 6.8.0 ships Windows, x86_64 and arm64 Ubuntu packages only |
| Kani install | build from source | build from source: 0.67.0's four release bundles are x86_64 and aarch64, and `cargo kani setup` looks up a bundle named for the host triple |
| rustc | the nightly Kani pins, on the host we have | `nightly-2025-11-21` does ship `rustc`, `rustc-dev` and `llvm-tools` for `riscv64gc-unknown-linux-gnu`, checked in its channel manifest |
| removes the architectural gap | yes: real `cfg`s, real `crate::arch`, the 95 riscv64 `cfg` lines outside `arch/` | yes, identically, since the goto program is the same |
| runs on every PR | yes, on the existing `ubuntu-24.04-arm` runner | only with a self-hosted runner; GitHub's hosted runners are x86_64 and arm64 (recalled, not checked) |
| machine | none | `radon` or the Scaleway RV1, as a Linux box |

(b) is (a) plus a machine. `new_machine_model` panics on a riscv64 host exactly as on any other,
so it still needs the patch, and saves only the driver's four lines.

Why `radon` is the wrong host. `radon` runs nife by netboot on the bench: UART into patagonia,
smart plug 2. Milestone 225 (run the soak on radon, argon and xenon) and pull request #1275's bench
rehearsal both need it as a nife board. As a Linux runner it would boot Linux from local storage, so every bench run first has
to take it off CI duty and back again. A soak cannot share it at all. A self-hosted runner on a
public repository also executes pull request code on a machine on calef's LAN, next to patagonia.
The Scaleway RV1 avoids the LAN problem but pull request #1278 is preparing it as a nife port
target, so it has the same conflict.

## Part 3: Would upstream take (a)?

Read, not recalled:

- model-checking/kani#2402, "Command-line flag to change model target or environment", is open since
  2023-04-23. A zerocopy maintainer added two use cases. Nobody has objected or sent a pull request.
- #2886 (a custom build target) was closed as its duplicate.
- #2086 (32-bit armv7) hit a CBMC crash. A Kani contributor traced it to `goto-cc` needing `-m32`,
  and the fix worked.
- #2197 asks Kani's docs to warn that other hosts lack machine models.
- A flag behind `-Z unstable-options` is not the "one way door" Kani's RFC process covers.

So the demand is on record and nothing says no; review latency is unknown. An upstream-quality
version adds a real flag, a sysroot with more than one target's `std`, bundle and `setup` support,
and a regression test. Estimated, not measured: two to four lane-days, plus review.

## Part 4: What stays unreachable even with riscv64 support

Measured on base `334804c8e`, counting `asm!` without `global_asm!` or comment lines:

```console
$ grep -rnE --include='*.rs' 'asm!' kernel/src/arch/$ARCH | grep -v global_asm \
    | grep -vE '^[^:]+:[0-9]+:\s*(//|\*)'
```

| arch | inline `asm!` | `global_asm!` | Rust lines in `arch/` |
|---|---:|---:|---:|
| aarch64 | 45 | 6 | 6,214 |
| riscv64 | 56 | 5 | 5,873 (6,663 with the four `.s` files) |
| x86_64 | 38 | 4 | 10,043 |

(#1276's "61 sites" is these 56 plus the 5 `global_asm!`. Its `arch/riscv64/` total was counted
twice, so its "6%" for option 1's 736 lines is really 12.5%.) What a riscv64 harness can prove is
bounded by `asm!`, fixed-address MMIO and the four `.s` files; `notes/kernel-proofs/riscv64-with-a-patched-kani.md`
has the per-file counts and the reach that is left.

## Containment, priced (the maintainer's addition)

The idea: each arch keeps all `asm!` in one-instruction wrapper functions, logic calls only those,
and a proof stubs them.

The sites classify cleanly. Of 139 inline sites, 127 are single instructions or fixed fences
(a CSR or system-register access, a barrier, a TLB or cache op, `wfi`, `ecall`, port I/O). The
per-site table is in pull request #1280's description, and the command above reproduces the counts.

| arch | single-instruction | must stay asm | logic inside asm |
|---|---:|---:|---:|
| aarch64 | 37 | 3 | 5 |
| riscv64 | 53 | 2 | 1 |
| x86_64 | 37 | 1 | 0 |

The six "must stay" are two register-preservation tests, the `at`+`par_el1` translation probes, the
`satp` ASID probe and the x86_64 GDT reload. Context switch and trap entry are already in `.s`. The
"logic" are four aarch64 read-modify-writes of CPACR and ICC_SRE (movable to Rust around two
wrappers) and two calibration spin loops that are asm on purpose, to fix the instruction count.

riscv64 is nearly contained already. By a crude count (asm inside a function of 8 or fewer code
lines), 34 of its 56 sites already sit in small functions. Most of the rest are SBI `ecall` helpers
padded with comments, copied per call site with no shared helper. About six functions carry asm in
real logic: `mmu::install`, `probe_asid_bits`, `permit_kernel_access_to_user_pages`,
`timer::set_cycle_counter_grant`, and two in `exceptions.rs` init and tests. aarch64 has 30 sites
inline in larger functions and x86_64 12, by the same count.

Does it answer #1276's refusal of its option 2? Only partly. #1276 refused compiling riscv64
files on the aarch64 host because `crate::arch` resolves to aarch64. Containment plus a model module
swapped in by `cfg(kani)` would let riscv64 logic compile without riscv64 register names, and
`super::` stays inside the riscv64 tree. But the riscv64 files hold 40 `crate::arch` references and
at least 58 into portable modules (`crate::memory`, `crate::smp`, `crate::cpu` and others). On the aarch64
host those run aarch64's architecture underneath. Rewriting the 40 to `super::` narrows the fiction;
the 58 remain, and the 95 riscv64 `cfg` sites in portable code still take their aarch64 branch. The
target flag removes all of it. So the host-swap half of containment is dominated by the flag, and
is only worth doing if calef refuses the flag.

The stub half stands on its own, on every architecture and host. The tree has zero `kani::stub`
today. It is also what turns option (a)'s compile reach into proof reach.

## The options

1. Carry the patch, and upstream it at the same time. The patch goes in `patches/` in `git
   format-patch` form, which is the tree's precedent for fixes it carries until upstream releases
   them (the two redoxfs patches). A new job, `prove-kernel-riscv64`, builds the patched Kani from
   source on the `ubuntu-24.04-arm` runner, cached on the patch's hash, and runs `script/verify
   --only kernel`. The same diff, reworked into a `-Z` flag, goes upstream against #2402. The carried
   patch leaves when a Kani release contains it. Cost: the patch as measured, and a CI job whose cold
   time is unmeasured (patagonia built Kani in about 4 minutes, before toolchain download). Then a
   rebase at each Kani release (one hunk at 0.68.0), and 2 to 4 lane-days for the upstream version.
2. Upstream only, and wait. No carried patch. Cost is the upstream work alone. Risk: #2402 has sat
   for three years, so riscv64 stays unproved for an unknown time.
3. A native riscv64 host. Refused: it needs option 1's compiler patch anyway, plus a
   from-source CBMC, Linux on a board whose job is running nife, and a self-hosted runner for
   pull request code.
4. #1276's option 1 alone (compile asm-free riscv64 files as a proof-only module on the aarch64
   host). Options 1 and 2 reach the same files with the real `crate::arch`, so this is the
   fallback if calef refuses to change the prover.
5. Containment, as priced above. Not an alternative to 1 or 2 for riscv64; it is the next step
   after either, and useful on aarch64 and x86_64 now.
6. Nothing.

## Recommendation

Option 1, then containment's stub half, riscv64 first. Carry the patch so riscv64 is proved on
every pull request now. Send the flag upstream so the patch has an exit. Then give riscv64's SBI calls
one shared helper and move the six logic-bearing sites behind wrappers. That wrapper set is what the
first riscv64 `kani::stub` harnesses need. aarch64 and x86_64 follow the same pattern afterwards.

## The seven questions

1. Considered and lost. Native host: option 1 plus a machine, and it takes `radon` from the
   bench. Upstream-only: unknown latency, not merit. #1276's option 1 and containment's host swap:
   the `crate::arch` fiction the flag removes. Containment's stub half is kept.
2. What the tree does in the analogous case. `patches/README.md`: carry a patch in format-patch
   form, submit it, drop it when the pin passes a release with the fix. And milestone 304 (`cargo kani
   -p kernel` only ever compiled one architecture) answered the x86_64 gap with a second CI row, which
   is the shape of `prove-kernel-riscv64`.
3. Prior art outside the tree. Kani and CBMC source at their pinned tags, and #2402, #2886, #2086,
   #2197 and #1304, all read through `gh` for this proposal. The GitHub hosted-runner architectures
   are recalled.
4. Is the premise true? The question's premise, that riscv64 support needs a riscv64 host, is not.
   CBMC is host-independent, and a patched Kani on an aarch64 Mac proved riscv64 code. #1276's
   reading of #2086 as ending in a crash is also incomplete: the crash had a one-flag fix.
5. Cost. Patch size, build time, memory, rebase fit and the kernel run are measured above. Kani's
   upstream-quality rework and CI cold time are estimates, marked.
6. Reversibility. A carried patch and one CI job come out cleanly, and nobody downstream acts on
   them. An upstream contribution is a public commitment to maintain, in a small way, a flag other
   people will use; that is the irreversible part, and it is an architect's.
7. Same cost, same choice? Yes. If the native host cost what the flag does, the flag still wins:
   it runs on hosted runners, on every pull request, and leaves `radon` on the bench. This
   recommendation is not about effort. Carrying versus waiting is about time, and says so.

## Built

Built 2026-09-25. `notes/kernel-proofs/riscv64-with-a-patched-kani.md` is the reader's
document: how to run it, what it cannot reach, and its `BUGS`.

- The patch, `patches/kani-0.67.0-riscv64-target.patch`: the tested shape above, 48 lines added and 5
  removed over four files. Its `git format-patch` header says what it does, where it goes upstream,
  when to delete it and when a fork would be worth it. Its file name is the tree's only Kani pin.
- The `d` warning is fixed, not recorded. rustc's `check_abi_required_features`
  (`rustc_interface/src/util.rs`, at Kani's `nightly-2025-11-21`) requires every feature the ABI
  needs in `sess.unstable_target_features`. rustc fills that from the backend's `target_config`,
  and Kani's returned nothing for riscv64. The patch's riscv64 arm there is the fix. Rebuilt
  without it, the kernel's crates gave 54 warnings; with it, 0. 
- script/verify-riscv64 (name provisional) builds that Kani once into
  `~/.cache/nife-kani-riscv64/` and runs `script/verify --only kernel` for
  `riscv64gc-unknown-linux-gnu`. It passes the launcher through `VERIFY_CARGO_KANI`, also
  provisional. It refuses green unless this run's goto output is under the riscv64 triple; a
  stand-in running stock Kani proved four aarch64 harnesses and was refused.
- The CI job `prove the kernel on riscv64` runs on `ubuntu-24.04-arm` with its aarch64 sibling's
  draft, A′ and scope conditions. It is folded into `verify (Kani proofs)` the same way, so the
  ruleset needs no change.

Timings. In CI, cold: the build step took 145 s and the whole job 2 min 57 s.
Warm: 45 s, 3 s of it proving. On patagonia a cold build took 244 to 621 s by load, under 1 GB resident, leaving
375 MB. A warm local run took 16 s.

## Follow-on

- **Outstanding.** The upstream half is [kani#4913](https://github.com/model-checking/kani/pull/4913),
  open and out of draft. feliperodri requested changes on 2026-10-05 (UTC); calef answered every
  thread and asked for re-review. It waits on a maintainer approving the fork's CI run, then on
  re-review. Upstream `main` is merged in, and the `kani_lib.c` object and review fixes are in it.
  Then dropping our patch.
- **Done.** The stub half of containment landed as #1302. Its three riscv64 `satp` proofs run
  only in this job.
- **Milestone 536.** `design/fatal-risks/README.md` risk 2 ("Only riscv64 is unreachable, and nobody here
  can change that") and `design/fatal-risks/proofs-and-their-reach.md` say the same false sentence.
  Both are an architect's files; milestone 536 (two records still say the prover cannot see `kernel/src`) is the correction already in flight for that risk's text.
- **Recorded.** `notes/kernel-proofs/riscv64-with-a-patched-kani.md`'s `BUGS`: the rebase per Kani release, the
  unpinned Kani of the sibling jobs, the one-slot sysroot, and riscv64-only harnesses the
  falsification sweep cannot replay.

## Index row

Kani compiled only for its own host, so no job anywhere compiled a line of `kernel/src/arch/riscv64/`. A carried fifty-line patch to Kani 0.67.0 gives it a riscv64 target instead of needing a riscv64 machine, and a new CI job proves the kernel row for riscv64 on the arm64 runner. What the prover can reach there is still bounded by `asm!`, MMIO and the `.s` files; the upstream flag is model-checking/kani#4913, in review.
