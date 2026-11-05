---
status: NOT-STARTED
raised: 2026-09-19
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 437. A witness bit set is held together by a hand-written list, and the list has gone stale twice

Everything it needs is in `crates/filesystem_contract` and a `macro_rules!`.

Milestone 326 (nobody has been assigned to turn a) repaired the two lists it found stale, which is rung four again.

## In brief

The in-QEMU confinement tests report their outcome as a bitmask. `fixture::navscape` has 32 bits,
`fixture::dirscape` 17, `fixture::attrs` 9, `fixture::escape` 8, `fixture::globscape` 7,
`fixture::twodir` 6, each declared as `pub const NAME: u64 = 1 << n;` and each module carrying an
`EXPECTED` constant that ors together the ones a correct run reports.

A bit that is zero is a probe that reports nothing while its boot passes. Nothing in the
declaration stops two bits sharing an `n`, and nothing stops a constant being wrong. What stands
between the tree and that is one host test per module, each a hand-written list of the module's
names, checked for distinctness and non-zero.

The list is rung four of AGENTS.md's ladder and it has failed twice in the one module that logs
it. `the_navigation_bits_are_distinct`'s own body carries this comment, written on 2026-08-24:

> These six were added after this list was last touched (touch's create half on 2026-08-22, its
> mtime half on 2026-08-24) and neither addition updated it; adding them now rather than leaving
> the gap for a seventh.

The seventh, eighth and ninth arrived on 2026-08-30 (`BIND_REACHED_TARGET`,
`BIND_ASCEND_REACHES_REAL_PARENT`, `BIND_STOPS_AT_TRUE_ROOT`) and the list was not updated. This
time nobody read it either: it was the mutation run of 2026-09-19 that said so, because `1 << 29`
under `>>` is zero and no test noticed. And `fixture::twodir` has never had such a test at all, so
five of its six bits could each be zero, two of them the structural finding
`notes/dir-capability.md` records.

## What to build

A declaration macro, so the list cannot be separate from the constants. Something of this shape,
in `crates/filesystem_contract`:

```rust
witness_bits! {
    /// It opened the file inside the first grant, through the first grant's own endpoint.
    OPENED_A,
    /// It opened the file inside the second grant, through the second grant's own endpoint.
    OPENED_B,
    ...
}
```

expanding to the same `pub const NAME: u64 = 1 << n;` declarations it does today, numbered by
position, plus `pub const ALL: &[(&str, u64)]` naming them. Then one generic test per module walks
`ALL`, and **a bit that is not in the list cannot exist**, because the list is where bits come from.
That is rung one for the numbering (a position cannot collide with itself) and rung two for the rest.

The names stay exactly as they are. This is a change to how they are written down, not to what
they are called. Every constant keeps its spelling, its doc comment and its value. `EXPECTED`
keeps being written by hand, because which bits a correct run reports is a claim rather than a
derivation.

## What this does not do, and the honest limits

- **It does not check that a bit means what its name says.** Only the in-QEMU boot can do that, and
  a witness that sets the wrong bit is still a witness that lies. This closes "a bit is zero or
  shared with another", which is the failure that has actually happened.
- **It does not reach the other crates.** `crates/swish` and the kernel's own test vocabularies have
  bit sets of the same shape; whether the macro should live somewhere both can reach is a question
  for whoever takes this, and putting it in `filesystem_contract` first is the cheaper start.
- A macro is a thing to read, and AGENTS.md's elegance tenet refuses machinery for tidiness. The
  argument for this one is that it *removes* a thing to remember rather than adding an abstraction:
  six hand-maintained lists become zero.

## Prior art in this tree

`crates/filesystem_contract`'s `verb::TABLE` already does the stronger version of this for opcodes:
two `const _: () = { .. }` blocks assert at compile time that the table is complete and in opcode
order, so a verb added to `fs` without a row fails the build. The bit sets are the same problem with
no such block, and the reason is only that they are constants rather than a table.

## BUGS

- It cannot be gated into existence. A module that declines the macro and writes its constants
  by hand compiles fine, and a lint that looked for `1 << n` in a `fixture` module would have to
  know which ones are witness bits and which are ordinary flags. So this is rung one for the modules
  that adopt it and rung zero for a module that does not, which is worth knowing before anyone calls
  it solved.

## Index row

The confinement fixtures' witness bit sets are kept honest by six hand-written lists. Milestone
326's mutation triage found the navigation one stale for the second time and `twodir` never having
had one at all. Five witness bits could each be silently zeroed and no test would notice. Two of them
are the structural finding `notes/dir-capability.md` records. A `witness_bits!` declaration macro would
make the list the place the bits come from, which moves the numbering from rung four to rung one,
with `verb::TABLE`'s compile-time blocks as the prior art already in this tree.
