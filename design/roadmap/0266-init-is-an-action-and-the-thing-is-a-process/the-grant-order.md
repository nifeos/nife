# One progenitor: the grant-order `cfg`, argued

An appendix to [milestone 266 (one progenitor, on all three architectures)](../0266-init-is-an-action-and-the-thing-is-a-process.md).
It holds the lane's argument for putting the one difference between the boards under a `cfg`, and
the three alternatives it considered. The tree has no such `cfg`: `components/src/progenitor.rs`
has one `GRANTS` table for all three architectures, since milestone 166 (one boot loader, reached
two inconsistent ways) unified the kernels' grant order.

## Why a `cfg`

A `cfg` is right, and it is one `cfg` over data, not over code. What the two kernels disagree about
is the *order* they grant boot capabilities in, so the slot numbers differ. aarch64's path is shared
with the test roles of milestone 19d (run a real workload). It carries a report endpoint and a test interrupt at slots 1 and 3
that the interactive system never uses, and everything after them is numbered around that.

## Three alternatives, each lost to the same objection

- Make the kernels agree. The better fix in the abstract, and it moves the aarch64 test roles' slot
  numbering, which six `spawn_progenitor` tests and `hello`'s whole 19d catalog are written against.
  That is a real milestone, not a line in this one, and the block's Follow-on proposed it.
- Probe at runtime. `system_initializer::boot` already probes for absent capabilities, so the
  machinery exists. It cannot work here. The ambiguity is not "is slot 9 empty", it is "is slot 9
  the virtio-rng transport or the graphical terminal's endpoint", and nothing distinguishes those
  from inside the process.
- Pass the table in. The kernel could write the endowment into the configuration page. That puts a
  layout two programs agree on into a runtime channel to avoid a compile-time constant. It is the
  wrong direction on `DECISIONS`' own dependency argument, and it makes the boot depend on a page
  the boot is what sets up.

So: data under a `cfg`, in one file, beside the other board's table where a reader meets both.
