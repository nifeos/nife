# The `e1000e` NIC: its appendices

[`notes/e1000e.md`](../e1000e.md) is the page to read: the driver's split, its gates and the bench
step on xenon. This directory holds what the page links to and does not carry:

- [The I219 port](the-i219-port.md): where `crates/e1000e`'s register facts come from, and how its
  `pch` module carries FreeBSD's I219 bring-up.
- [The system tests](the-system-tests.md): why the DHCP lease test is also the confinement test.

*Name: provisional, for the directory and the stems `the-i219-port` and `the-system-tests`: the
directory follows `notes/process-view/`'s shape, a page's appendices beside it. calef names
directories and files.*
