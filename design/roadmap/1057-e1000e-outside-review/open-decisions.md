# Open decisions

An appendix to [the milestone](../1057-e1000e-outside-review.md), which lists each decision
by name and links its brief here.

## D1. Does the first release of `e1000e` wait for a cabled I219 run?

The crate carries two device paths. Host tests, the Kani harnesses, three fuzz targets and a QEMU
run of `-device e1000e` check the 82574L data plane. The I219 bring-up is a port of FreeBSD's
Intel shared code, and only the crate's simulator checks it. It has run once on silicon, with no
cable, so nobody knows whether it brings a link up and passes frames. P1-7 is that run. It needs
a Sunrise Point I219 cabled to a switch and a person at the bench, and nobody has set a date for
it (`nifeos/e1000e#10`).

This matters now because no crate under outside review publishes until the project has delivered
all of that review's feedback. This milestone lists P1-7 under P1 and in the done-when appendix,
so 1057 cannot read as delivered until a person runs it. The first publish of the organization
then waits with it. Everything else in this block is finished or follows the same run: P2-4's
hardware half and P2-9 (the link-up port) are P2 items, after the first release, already.

What constrains it: a crates.io version is permanent, and everyone who depends on it reads what it
claims. Today the README's BUGS says that no version of the crate has run on an I219 and that the
crate does not port the link-up half. `is_supported_device` still answers yes for the Sunrise
Point rows. A release with P1-7 open publishes that honest limit and claims nothing false. It
also offers a device path nobody has seen pass a frame. Waiting costs time with no bound, since
the run needs hardware and a person, and it holds the first publish of the other crates too.

The same shape sits in 1050 (two board steps) and 1058 (a run on one real controller, and an
ordering run on a weakly ordered host). Their status tables mark those rows bench. One answer here
settles them as well.

The question: does 1057 count as delivered for the publishing hold while P1-7 stays open as a
bench step?

- A (recommended). Yes. P1-7 joins P2-4's hardware half and P2-9 on the bench list. The first
  release ships the I219 port, and the README's BUGS says it has not run on silicon. The run's
  result lands in a later version, whichever way it goes. The same rule covers the bench rows of
  1050 and 1058.
- B. No. The first release waits for the cabled run, as the done-when appendix reads now. The
  first publish of every reviewed crate waits with it.
- C. Yes, but the first release stops claiming the I219. `is_supported_device` answers no for the
  Sunrise Point rows until the run passes, and a later version turns them on. This is the
  restrictive option on the API: turning a device on later is compatible, and turning one off is
  not. It costs a code change to a function a consumer already calls. It also hides a port that
  the README already describes honestly.

The recommendation is not about effort. B costs a person's time and C a change, and A costs
neither. A wins because the README already states the limit, so the run adds a fact rather than
correcting a claim. If the run fails, the fix is a patch release to code that BUGS already warns
about.

Door: two-way for the delivery rule itself. The publish it permits is one-way.

Prior art, read for this brief. Linux keeps drivers that "may or may not work" under
`drivers/staging/`. Its `Kconfig` leaves them off by default and warns that using one taints the
kernel, so an unproven path ships with a mark at the point of use. The README of the `ixy.rs`
user-space driver names the one controller family it supports. It does not separate the models
its authors tested from the rest. From memory and not read for this brief: the HARDWARE section of
FreeBSD's `em(4)` lists supported controllers without saying which anyone ran.

What the tree already does: 1050 and 1058 list bench runs as what remains, and say none of it
needs the architect. A pull request whose only open ask is a bench boot merges without it, and the
bench step confirms the result afterwards. A follows those cases.

Blocked until answered: reading 1057 as delivered, and so the first publish of the organization,
as far as this crate holds it.
