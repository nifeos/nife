# Verus: the appendices to notes/verus.md

[`notes/verus.md`](../verus.md) carries the out-of-date premise, what Kani stops at and what Verus
is in brief, what a pass means, whether Verus reaches `asm!` and MMIO, the proof-effort answer in
four parts, what is unsettled, the examples and BUGS. These files carry the evidence its prose
budget has no room for. All of it was read or run on 2026-09-20.

- [`sources.md`](sources.md): the papers, documentation and tool releases, with where each came
  from.
- [`what-kani-stops-at.md`](what-kani-stops-at.md): three throwaway harnesses with verbatim output,
  and how large the residue is.
- [`what-verus-is.md`](what-verus-is.md): the tool and its releases, deductive against bounded,
  the human's side of the contract, concurrency, and what of this tree's Rust would not parse.
- [`atmospheres-trusted-base.md`](atmospheres-trusted-base.md): SOSP '25's list of trusted assembly
  and Rust, and the rest of its trusted base.
- [`proof-effort.md`](proof-effort.md): the proof-effort figures and tables, seL4's cost,
  Atmosphere's syscall figures second-hand, and the extrapolation to this kernel.
- [`examples.md`](examples.md): the parent's EXAMPLES, each command and its output whole.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `sources` Name: provisional, for the source table it holds.
- `what-kani-stops-at` Name: provisional, after the parent's "What Kani actually stops at".
- `what-verus-is` Name: provisional, after the parent's own heading.
- `atmospheres-trusted-base` Name: provisional, for the trusted computing base it quotes.
- `proof-effort` Name: provisional, after the parent's "The number that decides it: proof effort".
- `examples` Name: provisional, for the parent's EXAMPLES section.
