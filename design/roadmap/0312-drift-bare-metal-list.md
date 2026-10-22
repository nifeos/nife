---
status: BUILT
raised: 2026-09-17
built: 2026-09-17
---
# 312. `script/drift`'s lists are derived, because the second copy drifted exactly like the first

Built by a lane on `milestone/312-drift-bare-metal-list`.
*(Number provisional until the merge queue lands it.)*

**This is the second occurrence of a defect `script/drift`'s own header describes**, which is the
reason the block is worth writing rather than the fix. The header, written when the script was
created on 2026-07-31, says:

> `cargo build --workspace` was missing `--exclude xtask`, and xtask is a host tool that uses `std`,
> so building the workspace for a bare-metal target could never succeed. The workflow was red on
> every run it ever had, which meant it could not report anything about upstream; and because that
> job is *designed* to tolerate red, the permanent failure looked like the mechanism working.

Every clause of that is true again, of the file that says it. `.github/workflows/toolchain-drift.yml`
had failed **17 consecutive daily runs since 2026-09-01**, `script/cadence-check` reported it DEAD,
and the reason was:

```
error[E0463]: can't find crate for `std`
  --> crates/serial_console/src/port.rs:54:5
```

`crates/serial_console` arrived with milestone 216, opens a real serial port on the dev machine and
spawns `stty`, so it is `std` by nature. Nothing added `--exclude board_console`, because nothing
was watching a list.

## Why it recurred, which is the only interesting part

The first fix moved the list **out of the workflow and into the script**, and the header argues for
that move in the right terms: *"which crates are host-only and which are bare-metal is a fact about
the workspace, and a copy of that fact in a workflow file drifts silently as the workspace changes.
Keep it here, where it is one fact in one place."*

One copy is better than two. **It is still a copy**, and AGENTS.md's ladder prices it: a hand-kept
list of `--exclude` flags is rung four, a written record that somebody has to read, and rung four
drifts. The fix was one rung up from where it started and three below where it could have been.

Two things made this instance quiet rather than loud. The job tolerates red by design, so there was
no signal distinguishable from upstream breaking us. And nobody runs `script/drift` by hand, which
is the header's own point about a command living only in CI, applied to a command that now lives in
`script/` but that a developer still has no daily reason to invoke.

## What was built

Both lists are gone. Not corrected: **gone**, replaced by the fact they were copies of.

The host-only crates come from the crates. A crate root without a `no_std` attribute gets
`extern crate std` injected by the prelude, and a target with no operating system under it has no
`std`. So such a crate cannot be in a bare-metal build no matter what its code does. A crate root
with one compiles for these targets by construction. `script/drift` reads `cargo metadata` for the
workspace members' `lib`/`bin`/`proc-macro` roots and greps each for the attribute:

```
==> Deriving the host-only crates
    host-only: board_console xtask
```

Exactly the two that are host tooling and not part of what ships, named by the tree rather than by
the file. A crate added tomorrow answers the question on the day it is written.

`test` and `bench` targets are deliberately not consulted: they are compiled by `cargo test` on the
host and never enter a `cargo build --target <bare metal>` graph, so their use of `std` decides
nothing. The derivation refuses to return an empty set, because `xtask` is host-only by
construction and an empty answer means the detection stopped working rather than that the workspace
changed.

The targets come from `rust-toolchain.toml`. Its `targets` array already names all four, and
rustup already honors it for the pinned toolchain. That is why the missing
`x86_64-unknown-none` went unnoticed. The pin covered for it everywhere except the one path this
script exists for, a `--toolchain` install that rustup will not apply the array to. `script/drift`
now installs every target the array declares, and builds the workspace for every one whose OS
field is `none`. That is the physical property rather than a second list:
`x86_64-unknown-uefi` is in the array, has firmware underneath it, and exactly one crate
(`uefi_loader`) targets it, so building the workspace for it would assert something untrue about
every other crate.

That closes the admitted `BUGS` note that sat two lines above the defect being fixed:

> BUGS: ONE architecture, where three are supported and two are installed four lines above. A
> nightly that breaks only the riscv64 or x86_64 build is invisible to the toolchain-drift workflow
> this script exists to make honest.

And the workflow's own copy of the target list goes too. It named two of three; it now installs
the toolchain and lets `script/drift` add targets to whatever toolchain is in effect, which works
for both the argument form and the `RUSTUP_TOOLCHAIN` form CI uses.

## Which rung this sits on

Rung one, in the ladder's own terms, for the part that can reach it: there is no list to forget to
update. Not "make the wrong state unrepresentable" in the compiler sense, which no shell script
can be, but the next closest thing available here, which is that the wrong state cannot be *written*
because nobody writes the state at all.

`script/lint`'s host pass derives the mirror-image fact the same way, after milestone 244 found
`system_initializer` missing from a hand-kept copy of it. Same tree, same class of bug, same answer,
arrived at twice; that this file did not already have it is the finding.

The honest limit: derivation removes the copy, not the incompleteness. Nothing stops a future
gate writing `for arch in aarch64 riscv64` by hand, which is exactly what
`notes/architecture-list-sweep.md`'s option A says about itself. One script is not that sweep.

## What `script/drift nightly` actually reported, which nobody has known for 17 days

Green, on all three architectures, against the newest published nightly. 94 seconds wall clock.

```
==> Checking nightly (not the pin)
==> Deriving the host-only crates
    host-only: board_console xtask
==> Bare-metal build (aarch64-unknown-none-softfloat)
==> Bare-metal build (riscv64imac-unknown-none-elf)
==> Bare-metal build (x86_64-unknown-none)
==> Host tests
==> drift: clean
```

**And the caveat that makes that a weaker result than it looks.** The pin was raised to
`nightly-2026-09-17` earlier the same day (#910), and `rustup run nightly rustc --version` is
`1.100.0-nightly (923c95cdf 2026-09-16)`, so "the latest nightly" and "the pin" are the same
toolchain today. This run proves the script works end to end and that the tree is clean on the
newest nightly; it does not prove the script would *catch* drift, because there is no drift to
catch this afternoon. The first run that answers that question is tomorrow's scheduled one, which is
the first in eighteen days able to say anything at all.

## BUGS

- The `std` overlay rebuild is still only in YAML. The workflow's own comment calls
  `rm -rf target/nife-farm && cargo xtask std-exerciser` "the most fragile thing first, because it
  is the thing that actually breaks", and it is the step most likely to find real drift, since
  `patches/std-nife` tracks upstream *internals*. It is not in `script/drift` and was not run by
  this lane. It is two lines with no list in them, so it is not the defect this milestone is about,
  but it is the same argument the script's header makes, unfinished. Moving it would make
  `script/drift` blow away a developer's farm and take the account-wide `nife-dev` link, which is
  why this lane did not do it on its own initiative.
- The host-test exclusions are still written out (`--exclude kernel --exclude components
  --exclude fixtures`). Left alone deliberately. A new crate that belongs on that list breaks the
  host test with a compile error the moment it lands. That is rung two and loud, where the
  bare-metal list failed silently inside a job designed to tolerate red. Different rung, different
  risk.
- `script/lint`'s rustdoc pass names `--exclude board_console` by hand, a third consumer of the
  fact this milestone derived. Same reasoning as above: rustdoc fails loudly and immediately in
  every CI run, so that copy announces its own staleness. It is a candidate for the same treatment
  and not an outage waiting to happen.
- The OS-field test is a string match on the triple (`*-none`, `*-none-*`). It is correct for
  all four targets in the array and for any `-unknown-none` triple a fourth architecture would use.
  A bare-metal triple that spelled its OS field some other way would be silently dropped from the
  build rather than loudly rejected.
- A crate could declare `#![no_std]` and still fail to build for one architecture, on an
  `asm!` block or a target-specific intrinsic. The derivation answers "does this need `std`", which
  is the question that produced both occurrences of this defect; it does not answer "does this
  build", which is what the three builds themselves are for.

## Names

No new names. No crate, program, module or public function was added. The two shell locals
(`all_targets`, `kernel_targets`) and the loop variable are internal to one script.

## Gates

| gate | result | wall clock |
|---|---|---|
| `script/drift` (the pin, three architectures) | clean, exit 0 | 82s |
| `script/drift nightly` | clean, exit 0 | 94s |
| `shellcheck --severity=warning script/drift` | clean | |
| `script/lint` | 0 | 71s |
| `script/test` (aarch64) | green, exit 0 | 340s |
| `script/test --arch riscv64` | green, exit 0 | 274s |
| `script/test --arch x86_64` | green, exit 0 | 270s |

## Follow-on

- **Done.** The `toolchain drift` workflow can report something again, for the first time since
  2026-09-01, and now reports it for three architectures rather than one. `script/cadence-check`
  should stop calling it DEAD after tomorrow's 07:00 UTC run.
- **Done.** The `BUGS` note about one architecture of three is resolved and removed rather than
  left as a standing confession.
- **Recorded.** That the `std` overlay rebuild lives only in the workflow is stated in this block's
  `BUGS` and beside the step in `.github/workflows/toolchain-drift.yml`. It is the next thing worth
  a lane if anyone wants the header's argument finished, and it needs a decision first about
  whether a developer-facing `script/drift` may destroy the farm and take the `nife-dev` link.
- **Recorded.** The two remaining hand-written consumers of the host/bare-metal split
  (`script/drift`'s host-test line, `script/lint`'s rustdoc line) are in this block's `BUGS`, with
  the reason each is a lower risk than the one that failed. Both break loudly, in a gate nobody
  tolerates red from.
- **Recorded.** `notes/architecture-list-sweep.md`'s option A, a single derived architecture list
  every script reads, remains unbuilt and is now one consumer smaller. It is already written up
  there with its cost measured, which is why this is a pointer rather than a new proposal. This
  milestone applied its technique (parse `rust-toolchain.toml`) in one file. That note counts
  seven more places the same list is spelled out by hand.

## Index row

`.github/workflows/toolchain-drift.yml` had failed every daily run since 2026-09-01 and could report
nothing about upstream. That is the second occurrence of the defect `script/drift`'s own header
describes: a hand-kept `--exclude` list, `crates/serial_console` (milestone 216) arriving as a host
crate that needs `std`. And a job designed to tolerate red made the permanent failure look like the
mechanism working. The first fix moved the list out of the YAML and into the script; one copy is
better than two and is still a copy, which is rung four of AGENTS.md's ladder and drifted the way
rung four drifts. So both lists are now **derived**. Host-only crates come from the crates
themselves, since a crate root without a `no_std` attribute gets `extern crate std` injected and a
target with no OS has no `std`. The answer is `board_console` and `xtask`, named by the tree rather
than by the file. The target list comes from `rust-toolchain.toml`'s `targets` array. The
workspace is built for every one whose OS field is `none`, which closes the admitted `BUGS` note two
lines above the defect. The check built one architecture where three are supported. Measured:
`script/drift nightly` clean on all three in 94s, with the honest caveat that the pin was raised
the same day. So the latest nightly and the pin are one toolchain today, and the first run that can
really catch drift is tomorrow's.
