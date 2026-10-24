# One program and one grant table on every architecture

*Name: provisional. For why the progenitor is one program with one grant table. calef names files.*

The appendix to [`notes/progenitor-and-loading.md`](../progenitor-and-loading.md), which carries
how the progenitor builds a child. This file carries why there is one progenitor, not one per
architecture.

`components/src/progenitor.rs` is one program on aarch64, riscv64 and `x86_64`, and its `GRANTS`
table is one table, so the file carries no `cfg` at all. Two copies of the first process is the
failure mode of DECISIONS §19 (architectural parity is a tenet). That bill was
paid once: a fix that landed in one copy and not the other presented as a boot that reached
userspace and printed nothing, with no fault and no message. `script/swish-check` runs both legs,
which is what makes it the gate for that file.

The slot layout is `kernel::user::boot_progenitor`'s `assert_eq!`s read from the other side
(milestone 166 (one boot loader) unified the loaders). The one genuine per-architecture difference
is the *shape* of slot 1's console authority. It is a device page on aarch64 and riscv64 and an
`x86_64` `PortRange` (milestone 299 (the x86 port-range capability), DECISIONS §121 (x86 port
I/O)). That is the kernel's and `system_initializer`'s to know, not a slot number the table records.
