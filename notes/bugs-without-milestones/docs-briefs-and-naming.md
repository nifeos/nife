# Docs, briefs and naming

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  CONTRIBUTING.md | nothing tells a contributor which crates are host-testable without reading script/test | CONTRIBUTING names the host-testable crates (or derives the list) | S  |
|  design/decisions/0143-machine-names-in-source.md (index bullet) | `notes/target-hardware.md` index line does not say it defines the machine names. | Fix the target-hardware index line | S  |
|  design/decisions/0204-how-userspace-asks-where-a-thread-runs.md (first bullet) | The tree owes a one-sentence pointer where the two placement mechanisms meet. | Cross-sentence between SURVEY and the other placement mechanism | S  |
|  design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md (manifest bullet) | "manifest" means two things; the image's package list needs its own name before it is built. | Name the image's package list | S  |
|  design/fatal-risks/README.md (bold bullet) | Experiment lead-ins and verdict cells are parsed as bold, spending the entry's bold budget until they move to frontmatter. | Move fatal-risk lead-ins to frontmatter | S  |
|  design/naming.md (boot mode bullet) | Boot mode is still `shell` (`cargo xtask shell`) while the program is `swish`. | Rename the shell boot mode to swish | S  |
|  design/naming.md (capsicum bullet) | `design/capsicum-and-the-retrofit-question.md` still names `worker` in a present-tense claim. | Fix the capsicum doc's `worker` | S  |
|  design/roadmap/0106-deadline-wait.md | "There is no timed wait" still written in about a dozen places (confirmed by grep: 0051, 0103, 0126, 0129, 101, 43, `crates/vmstat`, `grant_plan` catalog) | Sweep the stale "no timed wait" claims | S  |
|  design/roadmap/0259-sweep-the-notes-for-stale-claims.md | notes/visionfive2.md, board-console.md, scripts.md never read; benchmarks.md, soak.md, load-sensitive-assertions.md, x86-port.md only skimmed | Finish the notes sweep on the unread and skimmed pages | M  |
|  design/roadmap/0259-sweep-the-notes-for-stale-claims.md | design/roadmap/ has the same stale-claim disease (66's gap table, 99's compressor claim); sweep scoped to notes/ only | Sweep roadmap blocks for stale claims | M  |
|  design/roadmap/0262-consolidate-the-naming-record.md | merge queue, lane roles and dependency rule never checked for being stated twice | Check other rules for duplicate statements | S  |
|  design/roadmap/0265-proto-is-a-truncation-not-an-abbreviation.md | nobody checked for other truncations of the same kind (`_cli` etc.); "different milestone" not found in the index | Sweep for other truncated name suffixes | S  |
|  design/roadmap/0276-naming-metrics.md | nothing counts public function and method names, types, helpers/ names or directories (also 0283:133, fifth surface with no gate) | A gate or count for the unguarded name surfaces | M  |
|  design/roadmap/0285-the-user-mode-prefix.md | four NOT-STARTED and eight PARTIAL blocks still spell the deleted crate `user_rt`; live intent, listed not fixed | Fix live roadmap blocks that name user_rt | S  |
|  design/roadmap/0297-soak-becomes-soak-test.md | bodies of 0373 and 0379 (and other ex-proposals) still show `soak`/`reboot_soak` as live cargo features and a build command that fails | Refresh the old feature spellings in promoted proposal bodies | S  |
|  design/roadmap/0297-soak-becomes-soak-test.md | six names still spell `soak` (`soak_page`, module, program, `board-image --soak`, `board-console --until soak`) and none has been put to calef | The six `soak` spellings the rename left, put to calef | S  |
|  design/roadmap/0446-the-naming-worklist-says-what-it-covers.md | public type names are uncovered by the naming worklist and the count was never run | Census of public type names against the naming record | S  |
|  design/roadmap/0494-a-driver-for-the-network-card-a-pc-actually-has.md | Intel BSD license asks binaries to reproduce its notice; no image or package ships a third-party notices file | Ship a third-party notices file with images and packages | S  |
|  design/roadmap/0522-a-boundary-drawn-by-dependency-not-by-path.md | `notes/register-of-measures.md` says the live unsafe ceiling is 94; the gate (`notes/unsafe-obligations.md`) says 88 | Correct the unsafe ceiling in the register of measures | S  |
|  notes/amd-vi.md (4th bullet) | no AMD-Vi row in `confinement-claims.md` (word budget); claims live only in the note | Add the AMD-Vi row to the confinement table | S  |
|  notes/hung-component.md | §32 (A supervisor may collect a corpse without being able to build one) and §41 (The endpoint is the broker, and a device is revoked by taking it back (miles) each carry a sentence the lane's evidence contradicts; integrator owns the edit | Correct §32 and §41 on restarting a hung service | S  |
|  notes/job-mix.md | the job-mix page has not been followed end to end by anyone but its author | Stranger pass over the job-mix page | S  |
|  notes/local-inference.md, notes/model-comparison/2026-09-27-qwen3-coder-bakeoff.md | caching claim ("billed as a miss") contradicts the spend log; every cached figure is unconfirmed | Reconcile the prompt-caching claim with the real bill | S  |
|  notes/model-comparison/2026-09-27-qwen3-coder-bakeoff.md | `notes/citations.md` still says a wrapped gloss reads as unglossed (made false by 583); correction owed at next condense | Correct the wrapped-gloss claim in citations.md | S  |
|  notes/shell.md | Sixteen Rust files still cite `components/src/swish.rs`, which moved to `swish/` | Repoint the stale `swish.rs` comment citations | S  |
|  notes/system-tests-and-the-kernel-crate.md (main.rs bullet) | Five notes still say `kernel/src/main.rs` for code now in `lib.rs` | Repoint the five `main.rs` citations | S  |
|  notes/xenon-firmware.md | two memory figures in the tree disagree and the note does not resolve which is right | Resolve xenon's two memory figures | S  |
