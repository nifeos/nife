# Documentation audit, 2026-11-17: the newcomer path

**Kind:** documentation. **Lens:** the newcomer path, run rather than read: every command the
README's Try it block, `CONTRIBUTING.md` and `AGENTS.md` tell a reader to run, followed to the
scripts and notes they send that reader to next, with each claim along the way checked against the
tree. **Findings:** fixed 0, minted 9, accepted 1.

The finding to carry off is that `script/catch-up`, which the README offers as the way to learn what
changed, has reported no milestone changes for any window since 2026-09-21 (UTC). It still reads
the roadmap index the tree retired that day, finds no rows, and prints empty sections with exit 0.
Minted as [`script/catch-up` reads the milestone
blocks](../roadmap/proposals/catch-up-reads-the-milestone-blocks.md).

## Why this lens

`script/audits` on `main` at `a4faa4614` reported documentation DUE on milestones built 489 to 510
(+21, fires at 15) and external packages 199 to 209 (+10, fires at 10). The security audits run the
same day take the verifier and radon's device authority; this sweep reads the documents.

Principle 3 says a newcomer must succeed without asking anyone, and milestone 117 (the stranger
test) asks whether a stranger can succeed at all. This sweep's reader already knows the tree and
asks a narrower question: does each command the front pages name do what the page says it does?
None of the four earlier sweeps ran the front pages' commands. The staleness worklist, decisions
read forward, one rewritten crate and the window's renames were all read, not run.

### The two judgment questions, answered

Has a subsystem been rewritten inside its existing crate since the last sweep? **No crate was
rewritten in place.** The window's large move was milestone 1094's split of `grant_plan` into the
planner, `grant_contract` and `program_table`, which the component count sees. It left one note
half moved: `notes/adding-a-program.md` names `grant_contract` correctly and still sends the
reader to `program_table`'s parent file for the rows (finding 3).

Has a decision superseded a plan that a note still prescribes? **One.** §322 (the QEMU bootstrap is
the one QEMU check) made the pinned build from `script/ci-qemu` the only QEMU, and `AGENTS.md`'s
Environment section still says QEMU comes from Homebrew (finding 4).

## How it was read

The worktree was a fresh lane checkout on a Linux host. Each command below was run, and its output
compared with what the page that names it says.

- The Try it block. `script/ci-qemu` found the pinned QEMU 11.1.2 with every device the suite needs.
  `cargo xtask build` exited 0. `script/console`, bounded with `helpers/qemu-bounded.sh`, reached
  the prompt in about 60 seconds, and the three commands the README suggests (`help`,
  `echo hello`, `least_authority_demo 7`) each answered as the README says. `script/test` ran to the
  end (see below). `script/catch-up` ran with its default window and with `--since 2026-09-01`.
- Start here. `script/roadmap --ready` lists ready blocks; `script/apropos syscall` was run as
  `CONTRIBUTING.md` and the script's header suggest; `helpers/agents-index-check.py` passed, and
  both skill link directories hold all seven links.
- `notes/adding-a-program.md`, every path and every test it names, each found by `git grep`.
- `notes/scripts.md`, every `script/` name it gives checked against the directory (84 scripts, none
  missing), and `cargo xtask`'s usage text compared with its dispatch table.
- `script/ci-build --list`, and the figures `CONTRIBUTING.md` and `AGENTS.md` give for the tree.

`script/setup` and `script/bootstrap` were not run. Bootstrap adds `clippy` and `miri` through
rustup, and the nightly `clippy` install on this host is under repair by another lane, so a run
would have disturbed that repair. What bootstrap would have checked, the pinned QEMU and the build,
was run directly instead.

## Findings

### 1. MINTED: `script/catch-up` reports no milestone changes for any window

`parse_roadmap` in `script/catch-up` tries `design/roadmap/README.md` before the block files. That
file still exists as prose with no `| N |` rows, so the function returns an empty table and never
falls back to `parse_blocks`. With the default window (since 2026-11-10) the script prints
`MILESTONES THAT CHANGED STATUS (0)` and `MILESTONES NEWLY MINTED (0)`; a count over the blocks
with `helpers/roadmap_block.status_and_built` for the same two commits gives 26 and 60. A window that
starts before 2026-09-21 reports 208 `MILESTONE ROWS THAT DISAPPEARED`. Commit `ada05a225` retired
the generated index on 2026-09-21. Severity high for the newcomer path: the tool answers wrongly
and says nothing about it. Minted as [`script/catch-up` reads the milestone
blocks](../roadmap/proposals/catch-up-reads-the-milestone-blocks.md).

### 2. MINTED: `script/apropos syscall` no longer finds `crates/abi`

The script's header, and `CONTRIBUTING.md`, give `script/apropos syscall` as the way to find
`crates/abi/src/lib.rs`, and the header says that finding it is why the tool exists. The search
returns `16 of 520 pages`, a fixed cap (`RESULTS_MAXIMUM`), and `crates/abi` is not among them.
The header's figures are stale too: it says 512 pages and five megabytes where the script reports
`2040 pages, 16602078 bytes`, and "four syscall numbers" where `crates/abi` declares five calls.
Severity medium. Minted as item 1 of [the newcomer path says what the tree
does](../roadmap/proposals/the-newcomer-path-says-what-the-tree-does.md).

### 3. MINTED: `notes/adding-a-program.md` sends the reader to the wrong file

Step 1 says the `programs!` row goes in `crates/program_table/src/program.rs`. That file holds no
row. The macro is at line 16 of `crates/program_table/src/program/catalog.rs` and its 28 rows
follow it, and the `manifest()` arms the compiler asks for next are in
`crates/program_table/src/program/manifests.rs`. Every other path and test the page names exists.
Severity medium: it is the first edit the page asks for. Minted as item 2.

### 4. MINTED: `AGENTS.md`'s Environment section prescribes a QEMU the tree no longer uses

It says "QEMU via Homebrew"; §322 made the pinned build from `script/ci-qemu` the one QEMU, and
`script/bootstrap` no longer installs a packaged one. It lists three targets where
`rust-toolchain.toml` pins five, missing `aarch64-unknown-uefi` and `x86_64-unknown-uefi`. It
names macOS on Apple Silicon as the host, and this sweep ran the build, the console and the suite
on Linux. Its opening line says "a capability microkernel for aarch64", where the README and rule 5
name three architectures. Severity medium. Minted as item 3; a lane does not edit `AGENTS.md`, so
the change goes to an architect.

### 5. MINTED: `script/ci-build --list` prints comment lines as checks

Four comment lines inside the `checks()` table heredoc (lines 139 to 142, added by `2f5b0c854`)
print as rows between `fmt` and `vt-replay`. Tier membership is unaffected, because
`names_in_tier` reads only rows with a tier cell. Severity low. Minted as item 4.

### 6. MINTED: `cargo xtask`'s usage omits five commands it dispatches

`xtask/src/command_line.rs` dispatches 49 commands and `xtask/src/usage.rs` lists 44. The missing
five are `disk-throughput`, `job-mix`, `network-bench`, `region-check` and `vt-replay`. Each is
documented in a note or a script, so a reader can find them, but not from the usage text. Severity
low. Minted as item 5.

### 7. MINTED: `CONTRIBUTING.md` gives `AGENTS.md` twice its size

It says "about 40 KB" at lines 26 and 223. `wc -c AGENTS.md` gives 19170. Severity low. Minted as
item 6.

### 8. MINTED: two decided sections still carry their merge caveat

§321 and §322 are `status: DECIDED` and still say their number and title are provisional until
the merge lands them. Severity low. Minted as item 7.

### 9. MINTED: narration on the newcomer path

§298 and §267 hold documents and comments to the rule in force. The front pages and the scripts
they name carry history in six places: `script/setup`'s comments on the toolchain date,
`script/apropos`'s dated header, `CONTRIBUTING.md`'s `BUGS` opening, `AGENTS.md`'s "As of
2026-08-30" and "(corrected 2026-10-06, #1739)", and `notes/adding-a-program.md`'s lines on when
`user/` split and what the page said before §89. Severity low. Minted as item 8.

### 10. ACCEPTED: the walk table in `notes/adding-a-program.md`

The page's `BUGS` section keeps a dated table of every walk of the page and what each found wrong.
That is history in a document. It stays: it is a measurement, it sits in `BUGS` where a reader
meets the risk it prices, and the page states its reason for keeping the rows, that deleting them
would hide how often the page has been wrong. Finding 3 is a seventh row's worth of evidence for
it.

## What was deliberately not examined

- `script/setup` and `script/bootstrap` as run on a cold machine, for the reason above. Milestone
  117's stranger runs own that path.
- The macOS path: `--hvf`, Homebrew's role in bootstrap, and the timings `CONTRIBUTING.md` gives
  for it. This sweep ran on Linux.
- ABI constants against their crates. The count did not move (99 to 99), and the 2026-11-03 sweep
  read them.
- The notes behind the external packages the window added. A security audit reads dependencies;
  this sweep read only the front pages' claims about them.
- Fuzz findings and advisories, which no lane reads.

## What wants a lane of its own

- [`script/catch-up` reads the milestone
  blocks](../roadmap/proposals/catch-up-reads-the-milestone-blocks.md), finding 1, with a host test
  over two fixed windows of this repository's history.
- [The newcomer path says what the tree
  does](../roadmap/proposals/the-newcomer-path-says-what-the-tree-does.md), findings 2 to 9. Its
  `AGENTS.md` items need an architect.
