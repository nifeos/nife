# Scheduled execution: the appendices

[`notes/scheduled-execution.md`](../scheduled-execution.md) is the page to read. The files here hold
work that page points at but does not need to carry whole.

- [registration.md](registration.md): the runtime registration protocol, proposed on 2026-09-26
  (UTC) and ruled by calef the same day as §222 (who holds a user's schedule).
- [one-image-per-entry.md](one-image-per-entry.md): why one image per entry was refused on
  2026-09-26 (UTC): an image is code, not authority.
- [calendar-and-wall-clock.md](calendar-and-wall-clock.md): the grammar and the clock-step rule for
  time-of-day entries. Both are settled in calendar-grammar-g5.md (2026-09-26, UTC).
- [calendar-grammar-g5.md](calendar-grammar-g5.md): G5, a calendar grammar in words, checked
  against its sources and priced against G3 and G4. calef ruled "G5 full" on 2026-09-26 (UTC).
- [calendar-grammar-g5-sources.md](calendar-grammar-g5-sources.md): what each primary source G5
  was checked against actually says, and the corrections that followed. Moved from
  calendar-grammar-g5.md under §212 (a prose budget), 2026-10-13 (UTC).
- [mem-entries.md](mem-entries.md): why a `--mem` grant nests inside its instance's region, why
  one is outstanding at a time, and what that costs the other entries. Moved from the program's
  BUGS by milestone 862 (comments state the constraint as it is now), 2026-10-09 (UTC). The
  parent's restatement of it became a link on 2026-10-11 (UTC), and its extra facts moved here.
- [archive-narrowing.md](archive-narrowing.md): why the program set the timetable's archive is
  narrowed to is a written list held by a host test, not computed in the kernel, and the audit
  line that measures it. Moved from the parent under §212 (a prose budget), 2026-10-11 (UTC).
- [next-after-proofs.md](next-after-proofs.md): why `next_after` snaps onto the beat rather than
  counting periods, and why phase preservation is tested rather than proved. Moved from the parent
  under §212, 2026-10-11 (UTC).
- [durable-store.md](durable-store.md): how boot-time re-derivation finds the schedules to
  restore, through a manifest rather than a directory listing.

*Name: provisional, minted 2026-09-26 (UTC) by the lane for milestone 129 (scheduled execution), for the directory and every
stem in it. `calendar-grammar-g5` was minted the same day by the lane `proposal/129-calendar-grammar`. Naming is calef's; `script/names --unratified` lists each stem.*

- `archive-narrowing` Name: provisional, minted 2026-10-11 (UTC) by lane/final-longest-notes, for
  the parent's "Where the set is computed, and why not at the spawn site". calef names files.
- `next-after-proofs` Name: provisional, minted 2026-10-11 (UTC) by lane/final-longest-notes, for
  what the parent said about `next_after`'s Kani harnesses. calef names files.
- `calendar-grammar-g5-sources` Name: provisional, minted 2026-10-13 (UTC) by
  lane/prose-cap-notes-c, for G5's "The premise, checked against primary sources". calef names
  files.
