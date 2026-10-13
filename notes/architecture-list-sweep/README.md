# The architecture-list sweep: the appendices to notes/architecture-list-sweep.md

[`notes/architecture-list-sweep.md`](../architecture-list-sweep.md) carries the short answer, the
disposition of each finding, the classification rule, the pattern behind what stayed complete, the
recommendation, where the work went and the limits. These files carry the evidence its prose budget
has no room for.

- [`silent-gaps.md`](silent-gaps.md): the eleven silent gaps as the sweep found them on 2026-08-27,
  with the evidence for each, the severity notes and the two gaps another lane held.
- [`virtio-ring-barrier.md`](virtio-ring-barrier.md): finding 9's five copies, and why x86_64's
  barrier is a compiler fence.
- [`not-findings.md`](not-findings.md): the recorded gaps, the legitimately single-architecture
  things, the lists that stayed complete, and one hazard that is not a finding today.
- [`design-options.md`](design-options.md): options A, B and C, each priced.
- [`methods.md`](methods.md): the four methods and what each cannot see.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `silent-gaps` Name: provisional. After the main page's own heading.
- `virtio-ring-barrier` Name: provisional. After the function finding 9 became.
- `not-findings` Name: provisional.
- `design-options` Name: provisional.
- `methods` Name: provisional. After the main page's own heading.
