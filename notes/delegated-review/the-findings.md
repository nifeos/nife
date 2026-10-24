# The findings behind the delegated-review study

*Name: provisional. calef names files.*

An appendix to [Does a delegated AI review catch what the gates and the maintainer
miss](../delegated-review.md), the run of milestone 521 (does an AI review of a pull request catch
anything the gates and the maintainer do not). It holds each arm's findings diff by diff, with the
transcripts quoted where the scoring turns on them. The page carries the tables, the verdict and
what the run says about routing.

## Arm 1: how each defect was or was not named

### D2, the consent string

`kimi-k3` named D2 in the ledger's own terms, once, unprompted. In one neutral review:

> Likewise "unpartitioned or unreadable" conflates two distinct findings. The module itself calls
> this "the single most load-bearing sentence a stranger reads"; it must not be self-contradictory.

Four other reviews reached the same string by a different and arguably better route, that
`format_disk_description` never consults `F_MBR`. Verbatim, and this is the shape of all four:

> 1. **kernel/src/user/install_service.rs — `format_disk_description` reports MBR and backup-GPT disks as
> "unpartitioned or unreadable".** The function only recognises partitions via `F_PRIMARY` (a valid
> primary GPT). The commit defines `F_MBR` and `F_BACKUP`, so the surveyor detects those cases, but the
> formatter never consults them: a disk carrying an MBR partition table — the most common Windows/Linux
> layout a user will actually meet, and the exact case the cited proposal exists to warn about — falls
> through to " (unpartitioned or unreadable)". The module's own docs call this sentence "the single most
> load-bearing sentence a stranger reads"; as written it actively tells a person that a partitioned disk
> is empty right before they type INSTALL.

Same expression, same harm, by a mechanism the ledger had not recorded.

`qwen3-coder`'s one partial flagged the same branch as misinterpreting `F_SIZE` and as
"misleading user feedback", which is the right line for a wrong reason and does not reach the
consent stakes. It is scored partial, which the registered threshold counts as a miss.

### The D1 miss is the result worth reading twice

Nothing named D1. But `kimi-k3`'s two adversarial reviews of it found the observation and
discarded it, at length, and then spent their whole budget doing so, so the answer never surfaced:

> *"In the diff, the note REPLACED the provisional note. Fine."*

> *"Is there an issue with the renumbering note replacing rather than supplementing the provisional
> note? ... That's actually a real, if subtle, point: the original note existed precisely because
> numbers minted on unmerged branches can collide, which is exactly what happened. ... Thin."*

> *"I'm confident this is fine. ... A reviewer nitpicking this would be wrong to block."*

Perception was not the failure; judgment was. A prompt that asked harder would not have fixed
it, because the model already looked harder and then argued itself out of the answer. That is a
fact about the ceiling rather than about the wording, which is exactly what milestone 521's block
said an adversarial arm was for.

And the same model recommended D1's defect on the clean diff. Reviewing C1, which kept the
provisional sentence, three of six reviews objected that the paragraph was now self-contradictory
and that the provisional parenthetical *should have been removed*. The reviewer prescribed, on the
correct diff, the exact edit that made the defective one defective.

## Arm 2: every finding on a clean diff

Fifty-four findings were adjudicated one at a time, as true, false, or unfalsifiable. The recurring
ones were verified against the files rather than judged from the prose.

### C2 was not clean, and that is the finding that outranks the score

`kimi-k3` found the following in a commit that passed every gate and that a human reviewer had
already signed off as clean. Each was checked against the file before being counted:

- `design/roadmap/526`'s justifying paragraph names the wrong statistic. Checked against the
  file and correct. The reviewer's own words, quoted whole because its punctuation is its own:

> 3. **design/roadmap/0526-….md** — "By the mean, the defect was fixed at three windows and had never been
> very bad at one" mislabels the statistic. The table reports the *median* (+0.36% at one window, +0.00%
> from three up); the *mean* at one window is dominated by the tail — the +1153% and +884% outliers alone
> contribute ~10 points over 200 boots — so the mean would have made the defect look glaring, not
> invisible. The paragraph that justifies the whole estimator choice gets its own statistics backwards.

  The sentence teaches the reverse of its own lesson, in the record the next cap decision will be
  made from.
- "An average would be a biased estimator for precisely the reason the minimum is an unbiased
  one" contradicts the same paragraph, which says every window is an upper bound and the minimum
  "converges on the truth from above". An estimator that is always an upper bound is biased upward at
  any finite sample; the property being claimed is consistency.
- The correction notice over-certifies the one comparison a reader will quote. It declares the
  debug-versus-release argument safe because the calibration "cancels exactly" within a boot, while
  the methods line four lines below it says *"Six boots debug, five release."* A ratio spanning two
  boots carries the quotient of two unknown calibration errors and does not cancel.
- A cross-reference resolves to the wrong place. "the 2026-09-21 section above records that
  CoreMark 'reports correctness, not yet a score'" points at a section that does not contain that
  sentence; it lives in an undated section, and the file has nine other sections dated 2026-09-21.
- Two deliverables in one commit contradict each other. `notes/tsc-under-tcg.md`'s `BUGS` still
  says the `qemu-bounded.sh` killer did not fire and sends the reader to another branch, while the
  milestone added in the same commit records the root cause and the runner fix.
- A paragraph dangles. `notes/tsc-under-tcg.md` still says "the calibration fix proposed above",
  after this commit deletes the proposal and marks the finding fixed.
- The milestone's own inventory of its diff omits a file the same commit changes, and the new
  heading is appended with no blank line before it.

None of that is arguable and none of it was caught. The honest consequence is that arm 2's
denominator is wrong: C2's false-positive count is zero partly because the diff was not clean, so
the corpus's clean label is the thing this run falsified first.

### Where the noise actually lives

On C1, the four-line renumber, `kimi-k3` produced 1.33 false findings per review and no true
ones. It asked for a reference sweep the bundle could not show it (counted unfalsifiable). It also
asserted three things that are simply not true of this tree:

- that the renumber should have updated `design/roadmap/README.md`, which `script/roadmap`
  regenerates and which a lane is told never to touch;
- that the new numbers needed a recorded reservation, which is what the integrator and the gate
  already do;
- and that the appended sentence broke the file's hard-wrap convention, in a paragraph whose own
  first line is 458 columns.

`qwen3-coder`'s four false findings on C1 were all of one shape. They asked the commit to prove a
premise it had already stated, and claimed the old number was unrecoverable in a repository that had
just recorded it in the text and in the rename. Its three on C2 include an arithmetic objection to a
figure that is correct. It read 3+3+4+3 windows as 130 ms where the text says 90 ms. That figure is
the delta over the one-window design, as the sentence it was objecting to says.

### The one disputed finding

`kimi-k3` called *"up to one second of boot, a hundred times this fix's cost"* wrong. It is right if
"this fix's cost" means the full sixteen-window calibration (160 ms, so about six times) and right as
written if it means one window (10 ms, so exactly a hundred). The sentence is ambiguous rather than
wrong, and it is recorded here as disputed rather than resolved in the reviewer's favor.
