# One image per entry: a proposal to refuse it

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

Status: REFUSED 2026-09-26. Asked "Refuse it?", calef answered "Yes" the same day, so the item
is refused for the reason below. Written by the lane for milestone 129 (scheduled execution) for
calef's decision. The block has carried this item as outstanding since 2026-08-23. This file argues
that it should be refused rather than built, and a refusal of scoped work is calef's to make.

## What the item says

A timetable is handed an archive holding every program its plan builds, and it keeps that archive
mapped for its whole life. So a compromised timetable can load any of those images, not only the
one an entry names. The item asked for one image per entry: a `spawner.rs`-shaped helper per entry,
each endowed with exactly one image, talking to the timetable over its own endpoint.

## Is the premise true

It is not, and that is the whole argument. The premise is that reaching an image is reaching
authority. In this tree it is not:

- A child's authority is exactly what its builder endows. `fire` and `fire_with_grant` in
  `components/src/timetable.rs` hand a job two things at most: the report endpoint and a region
  split for it. Nothing reads which image is running to decide what it gets.
- A compromised timetable already runs arbitrary code with its own capabilities. Loading a second
  image adds code, and code is not authority. Whatever program X could do with the timetable's
  capabilities, the attacker's own instructions could do directly.
- The one place an image's identity does carry authority is the progenitor's vouching under §219 (how the shell names an installed program to the spawner). A digest in the installed-programs table picks the
  manifest a spawn runs with. The timetable never asks the progenitor for anything. Its images come
  from an archive the spawn site assembled, so vouching happens when the archive is built, which is
  the spawn site's job either way.

## What the options cost

| option | cost | what it buys |
|---|---|---|
| Build it: a helper process per entry | Eight entries means eight more processes, each with its own budget, stack and image, and a new request protocol between them and the timetable. | Nothing, by the argument above. |
| Refuse it (recommended) | Nothing to build. The block's item closes as `Refused.` with this file as its reason. | A shorter list, and a residual stated correctly. |
| Keep it open | A standing item nobody will build, which the roadmap's follow-on gate will keep counting. | Nothing. |

§222 (who holds a user's schedule) already narrowed the part that was real. With one timetable per
session, the archive a timetable holds is one user's plan, and a compromise reaches one user's
authority rather than the machine's.

## What a yes means, and what a no means

- **Refuse.** The block's item becomes `Refused.`, and `components/src/timetable.rs`'s `BUGS` entry
  about the union of images is rewritten to say an image is code and not authority.
- **Build it anyway.** It needs a helper program and a request format, both provisional names and a
  wire format, so it would come back as its own proposal for the format.

Would the recommendation survive equal cost? Yes. It is also the cheaper option, but the reason is
that the thing being bought does not exist.
