# Lookup tables, and the one line nothing covers

An appendix of [ISA discovery](../isa-discovery.md). It holds two code-shape decisions in
`crates/machine_discovery` that a reader editing the crate needs and a reader of the discovery
design does not.

## Four lookup tables, and why none of them is a `match`

`TABLE` (extensions), `SBI_TABLE`, `IMPLEMENTATIONS` (SBI firmware) and `aarch64::IMPLEMENTERS`
(chip vendors) are all arrays with a `const` assertion over them. All four could have been a
`match`. The reason they are not is a bug class a `match` cannot be checked for: a duplicated key
compiles, silently, and makes the second arm unreachable forever. In a vendor table that is a chip
whose name can never be printed. In the extension table it would be a fact the kernel can never
report.

As arrays the duplicate is a compile error. And because the lookup is a loop over data, a test can
walk the rows rather than copy them into a test file. That is the difference between checking a
table and restating it.

The SBI extension ids go one better and are derived from their tags (`eid("TIME")`). The only
thing anybody can get wrong is a four-character string rather than a hex constant. That one is
worth the extra step, because a mistyped id probes an extension that does not exist, gets "no",
and refuses to boot on correct firmware.

The vendor codes are not derivable. Most are the ASCII of the vendor's initial, but enough are not
(`0x50`, `0x56`, `0xc0`) that the rule would have more exceptions than cases.

## One line nothing covers, on purpose

`bit(n)` is the helper that turns an index into an `Extensions` bit. Every caller is a `const`
initializer, so the compiler folds it and the body never runs. A coverage report will always show
it unreached. The only assertion available is that `bit(3)` is `1 << 3`, which restates the body.
So it has a comment saying why it is uncovered instead of a test that moves a number.

`eid` is the instructive contrast. It is const-only too, and it *is* called at runtime by a test.
There the thing that can be wrong is the tag rather than the arithmetic. Const evaluation and
runtime evaluation are two implementations of one function that should agree.
