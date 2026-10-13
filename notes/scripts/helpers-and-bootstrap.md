# Two things that are deliberately the way they are

*An appendix to [`notes/scripts.md`](../scripts.md), the front door to `script/`. It holds the
reasons `script/` and `helpers/` are split the way they are, and why `bootstrap` installs and builds
what it does. A reader who only needs to run a command should not have to open it. The directory
`notes/scripts/` and this file's stem are provisional names; naming is calef's.*

`script/` is the front door and `helpers/` is the drawer behind it. The normalized entry points are
in `script/`, GitHub's convention. `helpers/` holds `qemu-runner-aarch64.sh`, `qemu-bounded.sh` and
`memory-bounded-runner.sh`. These are internal plumbing that cargo and the scripts call, not things
you run by hand. `memory-bounded-runner.sh` (milestone 277, name provisional) is a cargo runner of
the same kind: `script/mutation` points `CARGO_TARGET_<HOST>_RUNNER` at it so each mutant's test
binary runs under a memory ceiling. `qemu-bounded-selftest.sh` (milestone 226) is the exception
beside them: it is in no gate and exists to be run by hand when `qemu-bounded.sh` changes. Two
directories an `s` apart is a little awkward, but each follows its own convention, and keeping the
runner where cargo already expects it (`.cargo/config.toml` points at
`helpers/qemu-runner-aarch64.sh`) was cheaper than moving it.

The drawer is named `helpers/`, not `scripts/`, because `script/` and `scripts/` differ by one
character, sort next to each other, and neither says which one a person types. calef ratified it:
*"I'm totally disoriented in the script directory. Also, why do we have script and scripts?"*
Captured transcripts and dated accounts keep `scripts/` where they describe the past, the same way
this tree keeps `cricker-os`.

One thing in `helpers/` is not internal plumbing, and it is worth naming so the rule above is not
misread. `helpers/qemu-uefi-x86_64.sh` (milestone 87) boots the x86_64 kernel under OVMF, the real
UEFI firmware, from a staged EFI system partition. It is run by hand as well as by `cargo xtask
uefi-boot`. It lives beside the runners rather than in `script/` because it is a QEMU invocation of
exactly their kind. It is not a cargo `runner` only because this boot path has no `-kernel` argument
for cargo to pass it. See notes/x86-uefi-boot.md.

And one thing in `helpers/` is sourced rather than run. `helpers/qemu-path.sh` (milestone 287,
name provisional) puts this project's own QEMU on PATH when `script/ci-qemu` has built one. It
has no shebang on purpose: it exists to edit the caller's PATH, which an executed script cannot do,
so every `script/` entry point that can reach an emulator reads it with `. helpers/qemu-path.sh`
immediately after the `cd` to the root. `script/lint`'s *the project's QEMU is on PATH* check is
keyed on exactly that, so a new entry point cannot quietly skip it.

Why PATH and not the 38 call sites. The emulator is named bare from 38 places across 16 files in
three languages, including `exec qemu-system-aarch64` at the bottom of each
`helpers/qemu-runner-*.sh` and a `subprocess` list in `script/netboot-rehearsal`. Exporting PATH
once at the top of the process tree reaches every one of them, including the ones nobody has written
yet. That is why the helpers under `helpers/` do not source it themselves: they are spawned by an
entry point, or by the `cargo xtask` that entry point spawned, and have already inherited it.

And one thing in `helpers/` is not a script at all. `helpers/rust_source.py` is a Python module
nothing executes, and both `script/lint` and `script/metrics` import it. It holds the derivations
both need: the comment-and-literal strip that makes a code-line count a code-line count, the
`unsafe` census, and the harness count taken from source text alone. Milestone 236 put it there
because those three derivations existed twice, once in a gate and once in the dashboard, and a gate
that changed its definition would have left `notes/project-metrics.md` quietly asserting the old
one. It sits in `helpers/` rather than `script/` for the reason the paragraph above gives: `script/`
is the front door, and this is not a door.

The premise that had kept the derivations copied turned out to be false. It is worth writing down
because the same shape will come up again. An inline `python3` heredoc looks like a place nothing
can be imported into, but every one of these scripts `cd`s to the repository root before it runs
python, so `sys.path.insert(0, 'helpers')` is all it takes. The alternative on the table was a host
crate, which would have made three `script/` commands depend on a `cargo build`.

`bootstrap` installs system packages, and on Linux it also builds one. Running
`script/bootstrap` will `brew install qemu` on macOS or `apt-get install` on Linux if QEMU is
missing. That is the pattern's intent: a fresh clone should be one command from working, but it is
also why `script/test` does *not* call `bootstrap` every time: re-checking a package manager on
every inner-loop test run is a poor trade. `setup`/`update` do the heavy dependency work; `test`
stays fast; `ci-build` provisions on its no-argument path, because that is the command a person runs on a
cold checkout before pushing. A named check (`ci-build fmt`) does not, because the caller naming
one check is a CI job that has already installed exactly what it needs, and a rustfmt runner has
no business apt-installing QEMU.

`NIFE_SKIP_LINT_TOOLS=1 script/bootstrap` skips `cargo-machete` and `typos`, the two tools only
`script/lint` uses. On Linux both compile from source, about 85 s per job on the arm64 runners
(measured on PR #1778, 2026-10-06 UTC). The CI jobs that never lint set it; the `lint` job installs
them itself and a developer's bare `script/bootstrap` still installs everything.

On Linux the package manager cannot finish the job, so bootstrap runs `script/ci-qemu` itself
(milestone 287). No Ubuntu release ships a QEMU with `-device riscv-iommu-pci`, and `apt-get`
already fetches the newest package for the release, so there is nothing better for apt to get.
Bootstrap therefore says what it is about to cost and builds the pinned version, which takes about
twelve minutes the first time and under a second on every run after it. Before that milestone it
printed the two commands for a human to type, which is rung four of AGENTS.md's ladder; worse, the
printed sequence looped, because nothing outside `.github/workflows/ci.yml` put the build's install
prefix on PATH. See `helpers/qemu-path.sh` and
design/roadmap/0287-bootstrap-installs-a-working-qemu.md.

What this appendix's numbers cite, gathered in `notes/scripts.md` until the split: milestone 87 (the
x86_64 bare-metal machine) and
milestone 287 (`script/bootstrap` installs a working QEMU on Linux).

## What this file's numbers cite

Glossed here because the split made this a file of its own, and `script/citations` asks each
file to say once what a number cites. Each gloss is the record's own title.

- milestone 226 (`qemu-bounded.sh` leaves an emulator behind, and the next run blames the wrong thing)
- milestone 236 (Three derivations are copied between scripts, and nothing notices when they drift)
- milestone 277 (Bound what one mutant may allocate, so a runaway kills the mutant and not the machine)
