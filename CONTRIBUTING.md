# Contributing to nife

This file is for a **person** deciding whether to work on this project: what it wants, what it will
cost you, and what you can decide for yourself. GitHub links it from the pull request UI, which is
why it is here rather than folded into something longer.

It deliberately does not restate the rules. **The rules are in [`AGENTS.md`](AGENTS.md)**, which is
the project's constitution and is written for whoever is doing the work, human or agent. This file
tells you which parts of it you need before your first change, and in what order. Two copies of a
standard is how one of them goes stale.

## The shortest honest description

A capability microkernel for aarch64, riscv64 and x86_64, in Rust, from the first instruction, built
as a demonstrator rather than as a product, which is DECISIONS §14 (the project's direction). The
kernel allocates no memory of its own. Every driver and server is a userspace process. The
security-critical logic carries machine-checked proofs. Architectural parity across all three ISAs is
a gate rather than an aspiration, which is DECISIONS §19 (architectural parity is a tenet).

It is a research project with few architects ([ARCHITECTS.md](ARCHITECTS.md)), so the response you
get to a pull request is a real person reading it between other things. Both halves of that are
honest.

## Before you write anything

**Read [`AGENTS.md`](AGENTS.md).** It is about 40 KB and you do not need all of it to start, but you
do need these, and each one will otherwise cost you a rewrite:

- **Where architecture-specific code lives.** All of it is under `kernel/src/arch/`. `asm!` outside
  that directory is the bug, not the exception.
- **What goes in `crates/` versus `components/src/` and `fixtures/src/`.** Anything two binaries must
  agree on is a crate, never
  a `#[path]` module, because a shared module inside a `no_std` binary is unreachable by host tests
  and by Kani, and this project's whole method is pure logic in host-testable crates plus proofs.
- **Names are an architect's call.** Ship a provisional one and say so in your pull request; do not
  wait, and do not rename anything on your own initiative. [`design/naming.md`](design/naming.md)
  has the conventions, and `script/names --unratified` lists what is still waiting on a ruling.
- **`design/decisions/` section numbers are assigned at merge**, never claimed by a branch. Put your
  reasoning in `notes/` and in the pull request instead.
- **The ladder.** When something must not go wrong, prefer making the wrong state unrepresentable
  over a gate that fails loudly, over a written record at the thing itself, over a note. "Somebody
  will notice" is not a mechanism.

The reading order for everything else is at the top of [`README.md`](README.md#start-here).

**When you need a specific answer rather than an order, search for it.** `script/apropos <word>`
searches every note, decision and roadmap block, and every crate's and program's module header, and
prints the path of each page that says the word:

```sh
script/apropos socket      # notes/net.md, the socket decisions, and the program that serves them
script/apropos syscall     # the syscall decisions, and crates/abi/src/lib.rs, which holds the numbers
```

It is one word at a time and whole words only; `script/apropos`'s own header says what else it
cannot do.

## Getting it building

```sh
git clone https://github.com/nifeos/nife
cd nife
script/setup     # installs the pinned Rust toolchain, QEMU, python3, gh and ruby, then builds
script/test      # host crates, then the kernel under QEMU on all three ISAs
```

**On Linux, the first `script/setup` builds this project's own QEMU, and says so before it starts.**
No distribution ships a QEMU with `riscv-iommu-pci`, and the project refuses to drop the device
because a confinement test that quietly stops testing is worse than a red build. So when
`script/qemu-check` fails, `script/bootstrap` runs `script/ci-qemu` to build the pinned version.
That takes about twelve minutes the first time and is cached afterwards.

**Build through `script/*` or `cargo xtask`, never a bare `cargo build` at the root.** The workspace
holds `no_std` programs that only build for the bare-metal targets, so `cargo build --workspace`
fails on the host with "unwinding panics are not supported without std", which reads like a broken
tree and is only the wrong entry point.

The full command reference is [`notes/scripts.md`](notes/scripts.md). The few worth knowing on day
one are in the README's [Try it](README.md#try-it) block.

## What kind of change is wanted

**Whole pieces.** A complete, correct, tested change that lands with its documentation is worth more
here than a large one that lands without. That is not politeness; the documentation is part of the
deliverable, because a demonstrator that nobody can read demonstrates nothing.

Concretely, a change is finished when:

- It has a test that proves something **specific that nothing else would have caught**. `.bss` was
  zeroed, `sp` is aligned, the era pivot happens at the right second. Filler tests are worse than no
  test, because they cost a reader's attention and buy nothing.
- Pure logic lives in a crate that compiles for the **host**, so it runs in milliseconds without an
  emulator, and so Kani can reach it.
- It works on **every ISA** (aarch64, riscv64 and x86_64), or a scope note records the gap and the
  plan. A feature that works on aarch64 and silently not on the others is the bug.
- Any limitation it has is written in a **`BUGS` section next to the feature**, not in a tracker and
  not left for the reader to discover. This is the convention the project reaches for hardest, and it
  is not modesty: a newcomer who hits a limitation the docs named will trust the docs, and one who
  hits a limitation the docs hid will not trust anything again.
- Anything measurable is **measured**. `script/bench` runs icount microbenchmarks against a committed
  baseline. An honest tie recorded plainly is worth more than an overclaimed win.
- Its prose stays within budget. [§212 (a prose budget)](design/decisions/0212-a-prose-budget-for-every-document.md)
  caps a document's main body at 3,000 words, and
  [§213 (writing standards)](design/decisions/0213-writing-standards.md) limits sentence length and
  bold. A new document meets both outright; an edited one may not get worse.

## What is not yours to decide, and why that saves you time

Bring these up rather than building them, because each one is expensive to unship and cheap to ask
about:

| | |
|---|---|
| **The syscall surface** | A boundary, not a habit. A new method inside the existing capability model is fine and gets its semantics recorded in `design/decisions/`; a new syscall number is a design fork. |
| **A new dependency** | Taking one is a decision (DECISIONS §46 (thin primitives or whole subsystems)). The tree is thin architectural primitives or whole subsystems nobody would write, with nothing in between. |
| **Names** | Crates, programs, modules, public functions and directories are named by an architect. Ship provisional, say so. |
| **Anything two programs agree on** | A wire format, an opcode number, a packed word. The code is a morning's work; the un-shipping is not. |
| **`design/decisions/` section numbers** | Assigned at merge. |

Everything else you should simply decide and do. Most decisions here are reversible and deliberating
them costs more than getting them wrong.

## How to propose a change

```sh
script/claim fix/short-description --debt-paydown  # or --new-work; branch, worktree, empty commit, push, draft PR
# ...work in the worktree, committing and pushing as pieces prove out...
script/ci-build                              # every local check a PR must pass, cheapest first
gh pr ready && gh pr merge --auto --merge    # CI runs (a draft skips it); armed, it lands when green
```

Claim before you work. §90 (the claim is a draft pull request) makes a draft pull request the
claim, so two people cannot silently take the same milestone. `script/claim` makes it in one
command: it refuses a malformed branch name, cuts a worktree, and pushes an empty commit behind a
draft. It also refuses a lane that does not say whether it repays debt: pass exactly one of
`--debt-paydown` or `--new-work` ([notes/debt-paydown.md](notes/debt-paydown.md) has the line between
them). The empty commit matters, because without it GitHub closes the draft as merged when its base
lands. Any branch prefix is accepted except a near-miss of `milestone/<N>-<slug>`, the one prefix
with a meaning: `script/lint` reads it to require that milestone's roadmap block be updated.

`script/ci-build` is the one command to remember. With no arguments it runs every local check,
cheapest first, so a formatting slip costs twenty seconds rather than the whole run. It provisions
first with `script/bootstrap`, so it works on a cold checkout. `script/ci-build --list` prints the
checks with their tier. `local` is what the no-argument run does. `ci` is a check only a runner
waits for (Kani, the CPU matrix, coverage, fuzzing, the supply-chain audit), and you can still run
one by name: `script/ci-build coverage`. The list lives only in the table at the top of
`script/ci-build`, per milestone 286 (one enumeration of the checks that gate a pull request), and
CI reads the same table. `hvf` runs the aarch64 suite on the physical Apple Silicon core. It is the
slowest local check and the likeliest to flake on a busy host, and it skips loudly where
Hypervisor.framework is missing. See notes/load-sensitive-assertions.md if it flakes.

CI does not run on a draft. Both workflows open with a `draft gate` that skips the suite, and a
skipped check still satisfies a required one. Mark the pull request ready when the work is done, or
dispatch the workflows by hand to gate it earlier; [`briefs/gate-in-ci.md`](briefs/gate-in-ci.md)
has the commands.

Landing is automatic once you arm it, which is the second half of that last command: an armed
pull request lands through GitHub's merge queue when green, and the queue batches and rebases it, so
you do not need to keep the branch current. Nothing arms it for you. A ready pull request left
unarmed for 30 minutes, ejected from the queue, or conflicting with `main` is labelled
`needs-maintainer` by `merge-drain.yml`, for a maintainer to pick up. A pull request from a fork
waits for a maintainer: its workflows need an approving click. The `needs-architect` label holds a pull request for an architect's decision, and a
correction-of-error pull request gets that label automatically. If an agent writes a pull request,
its body opens with the `**Lane:**` line AGENTS.md describes, so a reader can tell who is speaking.

**One purpose per commit, and the message explains why rather than what** (the diff already shows
what). If a commit records a correction or a surprise, say so in the message: those are the most
useful commits in this history. `git blame` is the test. A reader tracing why a line looks the way it
does has to land on a commit that explains it.

**If your change is a design argument rather than code**, it goes in `design/decisions/` as a file
whose frontmatter says `status: PROPOSED` ([the schema](design/decisions/README.md)). It says what is
being decided, the options, the recommendation with its reason, and what is blocked until it is
answered. Work you found but are not doing goes in
[`design/roadmap/proposals/`](design/roadmap/proposals/README.md), unnumbered. A decision that lives only in a pull request thread
is in the medium this project exists to get things out of.

## Where the arguments are

- [`design/decisions/`](design/decisions/README.md) is what was chosen, what was rejected, and why,
  **including the decisions that were refused**. That is on purpose: you can disagree with an
  argument, but not with an authority.
- [`design/roadmap/`](design/roadmap/README.md) is the only status in the tree: one file per
  milestone, with `script/roadmap` as the index. It has a fixed status vocabulary
  ([notes/roadmap.md](notes/roadmap.md)) and a checker; anywhere else that claims status is stale by
  construction.
- [`notes/`](notes/README.md) is a glossary written while building, one file per question that
  turned out to be load-bearing.
- [`design/audit-reports/`](design/audit-reports/) is every audit, its lens, and when the next is due.

## Conduct

[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) is Contributor Covenant 2.1, with one honest change: its
enforcement section says plainly that reports go to one person, that there is no committee or rota,
and that no reply window is promised. The file says why, and what it costs.

## Reporting a bug, and reporting a security bug

Ordinary bugs: open a GitHub issue. The bug template asks for the **commit sha** you were on,
because `main` moves, and for which ISA and how you ran it, because parity is a gate here and one
architecture working while the other does not is itself a finding.

Anything you believe lets a confined process escape goes through [`SECURITY.md`](SECURITY.md)
instead, privately. Escaping is the exact thing this project claims cannot happen, so that class of
report is the most valuable one it can receive.

## Licensing

Dual MIT / Apache-2.0, at your option. Unless you state otherwise, anything you submit for inclusion
is dual licensed the same way, with no additional terms. There is no CLA.

## BUGS

- **This file is not a substitute for [`AGENTS.md`](AGENTS.md), and following only this file will get
  your first pull request sent back.** It names the five rules that most often cost a rewrite; there
  are more, and some of them (the ladder, the record-versus-code distinction, the worktree hazards)
  are judgement rather than checklist. The split is deliberate: this file is for deciding whether to
  contribute, that one is for doing the work.
- **`AGENTS.md` is about 40 KB and reads as a constitution rather than a manual**, which is a real cost to
  a first-time reader and is known. `CLAUDE.md` at the root is a symlink to it, kept so agent tooling
  keeps finding it; a human who opens `CLAUDE.md` and a human who opens `AGENTS.md` get the same
  file.
- **The issue templates route more than they invite.** A bug report is a real channel and prompts
  for the commit sha and the ISA. A feature request and a design argument both tell you the outcome
  is a file in `design/roadmap/` or `design/decisions/` rather than a thread, which is honest about
  where work lives here and is also more work than opening an issue elsewhere would be. Blank issues
  are off, so there is no way to file something that skips the prompts.
- **The contribution path assumes you can run QEMU on all three ISAs.** A change that only touches the
  host-testable crates does not, but nothing here tells you which crates those are without reading
  `script/test`.
- **Written 2026-08-18 by the third run of milestone 117 (the stranger test), which is an agent and not a person.**
  Every friction estimate in this file is therefore a lower bound: an agent does not get bored and
  reads further before asking than a human would.
