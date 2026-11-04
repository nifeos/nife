# Registers and constants

*An appendix to [`design/naming.md`](../naming.md). It holds how a hardware register, field or bit
constant takes its name, and the shapes the tree's constants share.*

## A register constant says what the thing is

A new or renamed register, field or bit constant copied from a cited source takes the spelled-out
identifier that says what the thing is. The source's spelling goes in `#[doc(alias = "...")]` and
in the opening of the doc comment, with the source cited.

- Cite OpenBSD's spelling first, then FreeBSD's, and Linux's only where no BSD source names the
  thing. The Intel `E1000_*` macros read the same in all three.
- Where no source defines an abbreviation (`MULR` in `E1000_FEXTNVM11_DISABLE_MULR_FIX`), the name
  keeps it as written and the documentation says that no source expands it. A name is a claim, and
  an expansion needs a source.
- A doc comment on a register accessor names the manual that defines the register and gives no
  section number, which moves between manual revisions.
- A ratified name keeps its spelling until its own name review.

This covers hardware names only. A program, crate, command or other software name says what the
thing does and follows the tree's own pattern: [crates.md](crates.md) and
[programs-scripts-and-directories.md](programs-scripts-and-directories.md).

## The shapes constants share

- A register offset ends `_REGISTER`. A mask of several bits ends `_MASK`.
- A limit opens with `MAXIMUM_` or `MINIMUM_` and ends with its unit: [units.md](units.md).
- An assertion constant ends `_DO_NOT_OVERLAP`, with no leading `THE_`.
- An address is `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` in every location, even in a file that
  holds only one kind.
- A RISC-V cause or an Arm vector says what it is: `SUPERVISOR_EXTERNAL_INTERRUPT`,
  `VECTOR_INTERRUPT_CURRENT`. Arm's own `IRQ` and `FIQ` vector kinds keep their spelling in the
  table strings.

A crate that exports a read-only mapped page spells its constants as one family:
[crates.md](crates.md#page-contract-crates).

Abbreviations inside a constant follow
[acronyms-and-abbreviations.md](acronyms-and-abbreviations.md).
