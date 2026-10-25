---
status: PROPOSED
raised: 2026-10-25
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# Every pin has a watcher, and a dependency bump carries evidence for where it sits

*(Title and slug provisional.)*

Raised 2026-10-25 (UTC) by lane `dependency-updates-measured`, at calef's request for a mechanism
that detects dependency updates and takes them in a measured way. The appendix
[inventory.md](every-pin-has-a-watcher-and-a-bump-carries-evidence-for-where-it-sits/inventory.md)
lists every pin in the tree, what watches it, and the measurements this file cites.

Reuse: Dependabot's `directories`, `groups`, `group-by` and `cooldown`; `script/kani-bump` and the
`propose-kani` job in `toolchain-bump.yml`, whose shape the tool-pin watcher copies;
`script/vendor-watch` for vendored upstreams; `cargo tree` for classification; cargo-vet's relative
audit as the model for reading a dependency's own diff. This proposal does not take cargo-vet or
cargo-crev themselves; "The alternatives" says why.

## The claim

Detection has gaps, and incorporation has one rule for everything. A bump to `spin`, which every
kernel links, and a bump to a fuzz harness's `libfuzzer-sys` meet the same gate today. The gate is
already strong: every pull request runs Kani, the icount tripwire, the fuzz targets, the boot file
budget and cargo-deny. What a bump lacks is anyone reading the dependency itself, a merge rule that
knows where the crate sits, and a record when a bump fails.

## What exists, verified on 2026-10-25

Detection, by pin:

- The root `Cargo.lock` and the GitHub Actions: Dependabot, weekly.
- The other 13 committed lockfiles: Dependabot since #2714, with a `script/lint` check that fails
  when a lockfile `script/supply-chain` checks has no entry. The two `vendor/` lockfiles are
  upstream's byte for byte and stay out; the measurement harness under `notes/` has no `Cargo.toml`.
- `vendor/redoxfs` and `vendor/talc`: `script/vendor-watch`, weekly, one pull request on a fixed
  branch.
- `rust-toolchain.toml` and `.kani-version`: `toolchain-bump.yml`, daily, default to merge.
- `.qemu-version`: milestone 943 (a new QEMU release is proposed as a pin bump), not started.
- Nothing watches `.cargo-deny-version`, `.cargo-fuzz-version`, `.cargo-mutants-version`, the
  riscv64 row's patched Kani 0.67.0, the cbindgen 0.29.0 that generates relibc's headers, the
  `ubuntu:24.04` base of the runner container, the relibc seed or DejaVu 2.37.

Four findings changed the shape of this proposal:

1. Closing a Dependabot pull request ignores that version, silently. calef closed #2 and #4
   (`tock-registers` and `aarch64-cpu` majors) on 2026-07-23 because a hand commit had already taken
   both together, and Dependabot answered each with "OK, I won't notify you again about this
   release". GitHub's documentation says Dependabot "stores the preferences for the repository
   centrally". That closing was harmless, but the mechanism is general: a foreclosure under §81 (a
   dependency stays upgradable) can happen with no edit to `.github/dependabot.yml`, where the §81
   lint cannot see it.
2. Dependabot already waits. With no `cooldown` key, it applies "a default cooldown of 3 days" to
   version updates, and never to security updates (GitHub's options reference, read 2026-10-25).
3. The kernel links four registry crates, not three: `spin` on all targets, `aarch64-cpu` and
   `tock-registers` on aarch64, and `uuid` on x86_64 through
   `globally_unique_identifier_partition_table` and `boot_slot` (`cargo tree -p kernel --target
   x86_64-unknown-none -e normal,build`). A hand list of kernel dependencies is already wrong, so
   the classes below must come from the graph.
4. The mutation gate runs `cargo mutants --in-diff`. A bump changes a lockfile and no first-party
   line, so it generates no mutants: a bump carries no mutation evidence at all.

## Part 1: detect

Every pin gets a watcher, and a lint keeps that true, the same pair #2714 made for lockfiles.

- A tool-pin watcher. A script (provisional name `script/pin-watch`) reads a table of pins and asks
  each upstream for its newest stable release: crates.io for cargo-deny, cargo-fuzz, cargo-mutants
  and cbindgen. It raises one pin per run on one fixed branch per pin, as `script/kani-bump` does,
  and the job opens one pull request that defaults to merge. Kani keeps its own job; QEMU joins this
  table when milestone 943 builds, rather than getting a third workflow.
- The watcher reports, and does not raise, a pin with no upstream release to compare against: the
  riscv64 Kani patch (a report that a newer stock Kani exists), the relibc seed, and DejaVu.
- `script/vendor-watch` gains relibc. The seed is "not a pin" (`vendor/README.md`), so the watch
  reports upstream commits that touch files we took since `893a3b9133ac`, and raises nothing.
- Dependabot's `docker` ecosystem watches `.github/runner-container/Containerfile`. The proposal
  `ci-runners-move-to-ubuntu-26-04` owns the base's next major, so this entry only reports.
- `script/lint` fails when a `.*-version` file, a `vendor/` directory, or a `version = ` line in the
  tool table has no row in the watcher's table. The row may say "reported only", but it must exist.

## Part 2: incorporate, by where the dependency sits

### Five classes, computed

A script (provisional `script/dependency-class`) places each registry package in the highest class
any edge puts it in, from `cargo tree` over each committed lockfile with target filters. A pull
request touching a lockfile gets a comment naming the class of each package it moves.

| Class | A package is in it when | Today (root workspace) |
|---|---|---|
| K, kernel | the kernel links it on any of its three targets | 4 |
| V, verification | a crate in `script/verify`'s table depends on it | `uuid`, plus K |
| N, untrusted input | a crate with a fuzz target, a TLS workspace, or `smoltcp` depends on it | measured by the script |
| S, shipped | a program in an image or a package depends on it | the rest of `components`' 27 |
| D, development | only dev, build, fuzz-harness or host-tool edges reach it | everything else |

The roots for N and S live in one file the lint reads, so a new fuzz target or TLS workspace joins N
by a reviewed diff rather than by memory.

### The evidence a bump carries

Every class keeps what CI already runs. Each class adds to the one below it.

| Class | Adds, before merge |
|---|---|
| D | cargo-deny green (advisories, yanks, licenses, sources); the PR body names the versions and any new package |
| S | the dependency's own diff summary: lines changed, `unsafe` tokens added and removed, new `build.rs` or proc-macro, from the two published `.crate` tarballs |
| N | a fuzz replay of the full corpus for every target whose graph holds the package, not the 60-second run |
| V | `script/verify` for every crate whose graph holds it (CI already runs it all), and `cargo mutants` over those crates' files, which the in-diff gate cannot do |
| K | an agent reads the whole diff and writes a delta review in the pull request; the icount table and the boot file size against `main`; §46 (thin primitives or whole subsystems)'s kernel bar rechecked for the new version, items 2 and 4 by name |

A new package in class K is a take, not a bump. §46 already asks an architect to rule on each one,
so the comment adds `needs-architect` whatever the merge rule below says.

The diff summary is a stdlib Python script over two tarballs from `static.crates.io`, which is the
sha-pinned source `vendor-verify` already trusts. Counting `unsafe` tokens is a tripwire, not an
audit. It says where a reviewer should look, as the icount table does for speed.

### Who merges

Decided 2026-10-25 (UTC). D and S merge on green, armed by whoever opened the pull request, as the
Actions group does today. N and V merge on green once the added evidence sits in the pull request,
by an agent. For K, an agent merges a minor or patch bump once the kernel-class evidence sits in the
pull request, and a major bump carries `needs-architect`.

Four registry crates run in the kernel with full privilege. Since 2026-07-22 they have moved four
times: `spin` twice through Dependabot (one of them a 0.x major), `aarch64-cpu` and `tock-registers`
once together by hand (coupled majors), and `uuid` arriving on 2026-10-22 under a proved crate. Two
of those moves were majors, so the rule asks for about one architect ruling a month at most, and it
puts that ruling where an API or a lock's semantics change.

### When to take a release

Decided 2026-10-25 (UTC). Dependabot's default cooldown of 3 days stands, and `dependabot.yml`
carries no `cooldown` key. A security update ignores the cooldown, so an advisory fix may land the
day Dependabot proposes it.

### When a bump fails

Decided 2026-10-25 (UTC). Nobody closes a Dependabot pull request to block a version. Finding 1 says
why: closing it is an `ignore` with no marker. A bump that cannot land leaves two records and keeps
its pull request.

- A `BUGS` entry beside the consumer: the version, what broke, what would unblock it, and a date to
  re-argue by. `deny.toml`'s RUSTSEC-2026-0387 entry is the shape: a bounded claim, what ends it,
  and a date.
- The pull request stays open with the `bump-blocked` label and cites the entry. Dependabot then
  updates it in place when the next version arrives, so the failure and the retry share one thread.

Still to build: a scheduled check that lists every Dependabot pull request closed without merge and
every `bump-blocked` one, and fails when one has no `BUGS` entry citing it. That is the rung-two
half §81's own `BUGS` section asks for. Open blocked pull requests count against
`open-pull-requests-limit`, 5 per entry; blocked bumps are rare (one group, #151, in three months),
and raising the limit is a change §81 leaves to us if it ever binds.

## The alternatives

- cargo-vet. Its audits record criteria (`safe-to-deploy`, `safe-to-run`) per version and accept
  relative audits of a diff, which is the K-class delta review in a file format. Not proposed now:
  it asks for an audit or an exemption for each of 86 root packages and 204 across all the lockfiles
  we resolve before the first bump, and the classes say only four need a human-grade read. We could
  write the K review as cargo-vet audits later, to import Mozilla's or Google's audit sets.
- cargo-crev. A web of trust over individual reviewers. Its value depends on reviewers this project
  does not have, and it gives no answer for crates nobody has reviewed.
- cargo-semver-checks. Its own documentation puts it in the publisher's hands, before release. It
  answers "did upstream break its API", which the build already answers for us.
- Renovate. Its `minimumReleaseAge` is the same idea as Dependabot's `cooldown`, read from the
  option's documented headings; moving tools would buy nothing the five keys above lack.
- One block per class in `dependabot.yml`. Dependabot groups by name pattern, not by graph position,
  so class blocks would need generated name lists in four places. One comment per pull request does
  the same with one script.

## The seven questions

1. The alternatives, with reasons: above.
2. What the tree does in the analogous case: `toolchain-bump.yml` proposes daily, defaults to merge,
   and lists five compelling reasons to hold; this copies that rule and adds the dependency's own
   diff. The RUSTSEC-2026-0387 entry is the tree's existing record of a blocked bump.
3. Prior art, read on 2026-10-25: GitHub's Dependabot options reference (cooldown, `directories`,
   `group-by`), cargo-vet's book and its built-in criteria, cargo-semver-checks' README. I read
   Renovate's option page only as far as its headings, and recall the malicious-release window.
4. Premise checks: findings 1 to 4 above; the tree commits 17 lockfiles, and 2 of them stay
   unwatched on purpose.
5. Cost: the classes and diff summary are two stdlib scripts and a workflow step; the tool-pin
   watcher copies `propose-kani` (about 80 lines of workflow). The architect's cost under the merge
   rule is about one ruling a month. Only the class counts are measured; the rest waits on a build.
6. Reversible throughout. Nothing outside this tree acts on a merge rule or a cooldown, and each is
   a one-line change.
7. The three decisions put architect attention where semantics move and keep one number implicit in
   Dependabot's default; none of them rests on effort.
