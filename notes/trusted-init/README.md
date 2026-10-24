# Trusted init: the appendices to notes/trusted-init.md

[`notes/trusted-init.md`](../trusted-init.md) carries the gap, what the kernel does, the hash, how
the build composes, what is proven and the rejected alternatives, with a summary of each phase
since. These files carry the phases in full.

- [`supervision-tree.md`](supervision-tree.md): phase B.2, the supervision tree that shrinks what a
  broken progenitor can do.
- [`interactive-boot.md`](interactive-boot.md): the same discipline applied to the boot path a
  person types at, with its BUGS, and the one progenitor and one loader of
  milestone 96 (one init: the spawn service written twice).
- [`chain-past-the-progenitor.md`](chain-past-the-progenitor.md): milestone 104 (the measurement continues past init), the progenitor
  measuring what it loads, with its BUGS and what is still not covered.
- [`measured-boot.md`](measured-boot.md): the design behind `crates/measured_boot`: one hash on
  both sides, a digest's type and text, the manifest, and the loader's verdict.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `supervision-tree` Name: provisional.
- `interactive-boot` Name: provisional.
- `chain-past-the-progenitor` Name: provisional.
- `measured-boot` Name: provisional. After the crate it explains.
