# Taking a dependency update

*Name: provisional, minted 2026-10-25 (UTC) by lane `dependency-updates-measured`. Named for the
job a reader arrives with.*

This page says what to do with a pull request that moves a dependency: a Dependabot bump, a
toolchain or Kani bump, or a vendored pin raised by `script/vendor-watch`. The rules here took
effect on 2026-10-25 (UTC).
[A proposal](../design/roadmap/proposals/every-pin-has-a-watcher-and-a-bump-carries-evidence-for-where-it-sits.md)
holds the design behind them and the parts still to build. §81 (a dependency stays upgradable) is the rule they serve, and §46 (thin primitives or whole
subsystems) decides what we take in the first place.

## What notices a new release

Dependabot reads every committed `Cargo.lock` except `vendor/`'s and the measuring harness under
`notes/packages/`, and `script/lint` fails when a new lockfile has no entry in
`.github/dependabot.yml`. `script/vendor-watch` watches the vendored pins, and `toolchain-bump.yml`
proposes the nightly and Kani. The proposal's appendix lists every pin and its watcher, including
the tool pins nothing watches yet.

## When to take a release

Dependabot waits 3 days after a release before it proposes a version update, its default, and
`dependabot.yml` sets no `cooldown`. A security update skips that wait, so take an advisory fix as
soon as its pull request goes green.

## Who merges a bump

Most bumps merge on green, armed by whoever readies the pull request. A crate the kernel links is
the exception. List those crates per target with

    cargo tree -p kernel --target x86_64-unknown-none -e normal,build --prefix none

and the same for `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf`, keeping the
lines without a path. On 2026-10-25 (UTC) they were `aarch64-cpu`, `tock-registers` and `uuid`.

- A minor or patch bump of such a crate merges once the pull request carries the kernel-class
  evidence. That means a delta review of the crate's own diff between the two versions, and the
  icount table and boot file size against `main`. It also means §46's kernel bar, rechecked for the
  new version.
- A major bump of such a crate carries `needs-architect`.
- A package new to the kernel's graph is a take, not a bump, and carries `needs-architect` under
  §46.

## When a bump cannot land

Never close a Dependabot pull request to block a version. Dependabot answers a closed pull request
by never proposing that version again, and it stores that choice on GitHub, outside
`dependabot.yml`, where the §81 lint cannot see it. Instead:

1. Write a `BUGS` entry beside the code that holds the dependency back: the version, what broke,
   what would unblock it, and a date to re-argue by. `deny.toml`'s RUSTSEC-2023-0071 entry has the
   shape to copy for the claim and what ends it; it carries no date, so add one.
2. Leave the pull request open, add the `bump-blocked` label, and link the entry from it.
   Dependabot updates the same pull request when the next version arrives.
3. When a later version lands, remove the entry in the same change.

Closing a pull request that a hand commit has made moot is fine. Say so in the closing comment.

## BUGS

- Nothing yet checks that a `bump-blocked` pull request, or a Dependabot pull request closed
  without merge, has a `BUGS` entry citing it. The proposal names that scheduled check; until it
  exists, the rule above rests on whoever readies the pull request.
- Nothing yet labels a kernel-crate major for an architect. A person or agent reading the bump
  applies `needs-architect` by hand, from the `cargo tree` list above.
- The evidence for the other classes (the dependency's own diff summary, a full-corpus fuzz replay,
  mutation testing over the affected crates) waits on the proposal's scripts.
