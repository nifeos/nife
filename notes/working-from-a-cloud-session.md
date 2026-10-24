# Working from a cloud session

*Provisional name.* How to do this project's work from a Claude Code cloud session: a Linux
container on Anthropic's infrastructure, opened from a phone or a browser, with no local Mac behind
it. Written 2026-09-24, before calef's travel week.

The short version: claim, write, run the cheap gates, and land all work. The heavy gates run in CI.
Anything that needs a physical board, HVF or launchd does not work here.

## What past cloud sessions actually hit

These come from the pull requests those sessions opened, not from reading the tree. Each one says
whether it still applies.

| when | where | what happened | still needed? |
|---|---|---|---|
| 2026-08-15 | #179 | The workspace had not built on an x86_64 host since 2026-08-03. A session dismissed it as "a container artifact" for hours. | No, a `script/lint` check now derives the bare-metal crates. But CI builds on arm64 only, so treat an x86_64 host failure as real. |
| 2026-08-17 | #278 | `script/lint` refused the harness's `claude/` branch prefix. | No. `claude/*` is accepted now. |
| 2026-08-17 | #278 | QEMU was not installed; the session installed it by hand. | Yes. Run `script/bootstrap`. |
| 2026-08-17 | #278 | Four QEMU and host-network checks failed identically on `main` in the container: a virtio-keyboard test needing the QEMU monitor socket, and the inbound, multicast and SMB host checks. | Unverified since. The container ran Ubuntu's QEMU, not the pin. Gate in CI and compare against `main`. |
| 2026-09-18 | #937 | `script/lint` exited 1 at `cargo-machete not found`, and the lane reported exit 0 because it read `tail`'s status. | Yes. Bootstrap installs it; never pipe a gate. `briefs/gate-in-ci.md` says so now. |
| 2026-09-18 | #947, #948 | Ubuntu's QEMU 8.2.2 lacks `riscv-iommu-pci`, and `script/qemu-check` refuses it. A roadmap block was drafted against it anyway. | Yes. Bootstrap builds the pinned QEMU from source, about 12 minutes. |
| 2026-09-18 | #937 | `script/ci-build` was still in its QEMU tier after half an hour and was killed. | Yes. Gate in CI instead. #956 did finish it a day later, with TCG only. |
| 2026-09-18 | #937 | A draft pull request ran no CI at all, and the lane read seventeen skips as a pass. | Yes, by design. Dispatch the suite as `briefs/gate-in-ci.md` step 2 says. |

The harness also shapes the record. Those sessions pushed seven pull requests (#937 to #956) from one
`claude/...` branch. Their commits are authored `Claude <noreply@anthropic.com>`, while GitHub shows
calef as the pull request author, so the `**Lane:**` line stays true. A `claude/` branch carries no
milestone number, so `script/lint`'s roadmap-status check (4b) never fires on it. Flip the block's
status by hand.

## Set up once per container

    script/bootstrap
    git config core.hooksPath .githooks
    git fetch origin main

Bootstrap installs the pinned Rust nightly, `clang`, `shellcheck`, `ruby`, `cargo-machete`, `typos`
and the pinned QEMU. It uses `apt-get`, and works as root without `sudo` since this note was written. The
QEMU build lands in `~/.cache/nife-qemu` and is lost with the container. If the environment
has a setup script, put these three lines there.

The hooks path installs the pre-push `rustfmt` check, which a fresh clone lacks. The fetch matters
more than it looks. Without `origin/main`, the roadmap-flip check and `script/citations --ratchet`
skip quietly rather than fail.

`gh` must be signed in, usually through `GH_TOKEN`. Claiming, dispatching CI and landing all go
through it. `helpers/lane-claim-check.sh` now refuses to run without it, instead of calling every
branch unclaimed.

## Claim, gate and land

Claim as `briefs/` and `AGENTS.md` describe: branch, empty claim commit, push, draft pull request.
`script/claim` works, but defaults its worktree to `nife-worktrees` beside the main checkout, which in a cloud clone is not where you want it. Set `NIFE_WORKTREES`
beside the clone, or claim in the clone itself with `git switch -c`.

Gate with `briefs/gate-in-ci.md`, all of it. Run the four cheap gates locally, then dispatch
`ci.yml` and `verify.yml` and read the runs. On a cold container the first `script/lint` compiles
clippy for the whole workspace, which takes minutes rather than seconds.

Land through the merge queue as usual. The queue, the drain and trunk health are Actions workflows,
so none of them depend on a laptop being awake.

## What does not work here

- HVF. The default accelerator is TCG everywhere, and `--hvf` and `script/bench --real` are Mac-only.
- Boards. `script/board-console`, `script/board-netboot` and the UART rig need the bench desk.
- `helpers/at-risk-check.sh` under `launchd`. It watches its own Mac's lane worktrees and nothing
  else. A container has no launchd and nothing watches it, so commit and push before every pause.
- Toolchain links. They live in each user account's `~/.rustup`, so a cloud session has its own,
  and `briefs/merge-and-cleanup.md` step 4 is a no-op unless something gated locally.
- `script/effort` and the local half of `script/cadence-check`, which read `~/.claude/projects` from
  the Mac's own session records. A container has only its own.
- Memory files under `~/.claude` on the Mac. Anything a session must know is in `AGENTS.md`,
  `briefs/` or `notes/`, or it is not known.

## What to avoid

- `script/test`, `script/verify` and `script/ci-build` locally. They work, slowly, but a container's
  memory and disk are unknown and the session may end mid-run. CI proves the same thing.
- A pipe after a gate. Read the command's own exit status.
- Trusting Ubuntu's QEMU. `script/qemu-check` rejects it, and results from it describe an emulator
  this tree refuses.
- The `launchctl` drain commands from before 2026-09-24. The drain is a workflow now:
  `gh workflow disable "merge drain"` and `gh workflow enable "merge drain"`, and since 2026-10-03
  a red-trunk hold does not need it stopped.

## BUGS

- No cloud session has run `script/test` against the pinned QEMU and recorded it, so #278's four
  failures may or may not be container effects.
- Whether a cloud session can push a branch outside `claude/` is not recorded. The evidence shows
  only `claude/` branches.
- Nothing reports a container that died with unpushed work. The `launchd` watcher has no cloud
  equivalent.
