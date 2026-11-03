# Documentation audit, 2026-11-03: names the window moved

**Kind:** documentation. **Lens:** names the window moved, read from the prose that still uses
them: the four capability names `crates/abi` spelled out, the four crates that left the tree or
took a new name (`jh7110_entropy`, `supervision_protocol`, `intrusive_fifo`,
`usb_keyboard_report`) and the removed `rmle`, each followed to every current document, script and
comment that names it. **Findings:** fixed 4, minted 0, accepted 1.

The finding to carry off is a tool, not a sentence. The kernel mutation census selects its corpus
by path, and it still named `crates/supervision_protocol`. The code that was there moved to
`process_builder`, and the hot-swap runtime moved to `hot_swap_server`; the host census excludes
both because they reach the user-mode runtime. So neither census mutated either crate, and nothing
said so. Fixed here: the census now names both.

## Why this lens

`script/audits` on `main` at `88c730fa9` reported documentation DUE on components 235 to 252 (+17,
fires at 10) and ABI constants 97 to 99 (+2, any change fires). Issue #3098 is the scheduled
workflow's repeat row. The security audit run in the same lane,
[2026-11-03-from-the-package-to-the-grant.md](2026-11-03-from-the-package-to-the-grant.md), reads
what the window added; this sweep reads what it renamed and removed.

The window is 328 merges, and a run of them were naming rulings: the capability spelling, the
register-name rulings for the mobile storage and clock-and-reset crates, crate renames and crate
moves to repositories of their own. A name a reader types and the tree no longer has is the
documentation failure that costs the most per sentence, because the reader acts on it. The last
three sweeps took the staleness worklist, decisions read forward and one rewritten crate; none
took the window's renames, so this one does.

### The two judgment questions, answered

Has a subsystem been rewritten inside its existing crate since the last sweep? **No crate was
rewritten in place.** The window's large moves were splits into new crates (the runtime halves of
the supervision and hot-swap contracts), which the component count sees.

Has a decision superseded a plan that a note still prescribes? **One.** The removal of `rmle` on
2026-10-31 (UTC) left `notes/the-first-split.md` measuring a split candidate that includes it
(finding 5).

## How it was read

Every public item a renaming merge removed and did not re-add (54 names, from the merges' diffs)
was searched for in prose, scripts and data outside `design/decisions/`, the audit reports and the
bench logs. Only a datasheet register name in a milestone appendix came back, so the register
renames are clean. Then each crate and ABI name above was searched for the same way, and every hit
was read in place.

## Findings

### 1. FIXED: two current documents used ABI names the ABI no longer has

`SECURITY.md`'s summary of the 2026-10-03 audit named `RECEIVE_CAP` and `NO_CAP`, and the
`ipc_stack_depth` feature's comment in `kernel/Cargo.toml` named `RECEIVE_CAP`. The constants are
`RECEIVE_CAPABILITY` and `NO_CAPABILITY`; a reader searching for either old name finds only the
doc aliases. Both now use the current names. Milestone titles that quote an old name as a gloss
stay as they are, because a title is the milestone's name.

### 2. FIXED: three documents pointed into a directory that left the tree

`components/src/jh7110_entropy.rs`, `notes/entropy.md` and `notes/entropy/jh7110-trng.md` each
said the TRNG's register layout and device-tree query are in `crates/jh7110_entropy`. The crate is
its own repository now, and `components/Cargo.toml` takes it by git revision. All three name the
crate and its repository (`nifeos/jh7110_entropy`).

### 3. FIXED: the kernel mutation census mutated neither runtime half

`helpers/mutation_kernel_census.py` lists the crates the host census cannot reach and the kernel
census therefore must, in its `kernel-needing-crates` slice and in `ROOTS`. Both lists named
`crates/supervision_protocol`, which no longer exists. The runtime code that was there is
`process_builder`, and the hot-swap server's is `hot_swap_server`. `helpers/host-excluded-crates.sh`
excludes both from the host census, so before this fix neither census mutated them, and the
kernel census dropped every path under them without a word. Both lists now name both crates, and
`--selftest` passes. The pure halves, `supervision_contract` and `hot_swap_contract`, stay where
they were.

### 4. FIXED: a proofs note gave commands for a package that no longer exists

`notes/user-proofs.md` told a reader to run `cargo kani -p user --bin printenv`, three times, for a
package that is now `components`; the command fails. It also called the program packages `user/`
with 68 programs, described its found defect in an editor the tree no longer carries in the
present tense, carried a `BUGS` line about that editor's editing operations, and carried a `BUGS`
line asking for a timing that `script/verify`'s table has since taken from CI. The commands name
`components`, the counts are gone, the defect is told as one in a removed program, and the two
moot `BUGS` lines are gone. `script/verify`'s comment on the same selection said `components` has
49 binaries; it has more, and the comment no longer counts them.

### 5. ACCEPTED: the first split's measurement table includes a removed program

`notes/the-first-split.md` measured each split candidate's commits while `rmle` was in the tree,
and its `rmle` + `mdr` + `demos` row counts commits to a program that is gone. The table is a
measurement at one commit, and changing a measured number by hand would make it neither. Recorded
in the note's `BUGS` (the note sits at its 3,000-word cap, so the line is short): the row predates
the removal, and that candidate is `mdr` + `demos` until someone measures the table again.

## What was deliberately not examined

- Records that keep the names they had on purpose: `design/decisions/`, built milestone blocks and
  their appendices, dated notes and censuses, bench logs, falsification patches,
  `design/removed.tsv` and the naming worklists. `design/init-and-granular-spawn.md` is milestone
  19's dated decision record and keeps `Tcb::CAP_INSERT` for the same reason.
- `design/fatal-risks/`, which names `SEND_CAP` in its account of past findings; a lane does not
  edit that directory, and the names there describe what was found when it was found.
- `design/roadmap/0955-the-kernel-and-its-programs-are-mutated-under-qemu.md`'s cost table, which
  priced the census with `supervision_protocol` in it. A cost measured at one commit stays as
  measured; finding 3 moves the corpus slightly and the next rotation's record carries the new size.
- `.cargo/mutants-baseline.txt` still lists `intrusive_fifo`. `script/mutation-census
  --save-baseline` writes that file, and its next run replaces the row.
- The staleness worklist's top entries, which the next sweep can take.

## What wants a lane of its own

Nothing new. The worklist's top entries remain the next sweep's starting answer.
