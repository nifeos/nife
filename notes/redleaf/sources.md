# What the RedLeaf comparison read, and what it could not establish

*Name: provisional. For the source table it holds.*

An appendix to [`notes/redleaf.md`](../redleaf.md). It lists the seven documents and one clone
the comparison rests on, with where each was fetched and the date it was read. It also lists what
the reading could not establish.

## What was read, and when

| source | what it is | read |
|---|---|---|
| Levy, Campbell, Ghena, Pannuto, Dutta, Levis. *The Case for Writing a Kernel in Rust.* APSys '17, Mumbai, Sep 2 2017, 7 pages. DOI 10.1145/3124680.3124717. | the ancestor of the argument | 2026-09-20, full text from `https://patpannuto.com/pubs/levy17rustkernel.pdf` (the ACM DOI returns 403 to an automated fetch) |
| Narayanan, Huang, Detweiler, Appel, Li, Zellweger, Burtsev. *RedLeaf: Isolation and Communication in a Safe Operating System.* OSDI '20, Nov 4-6 2020, pp. 21-34. | the paper | 2026-09-20, full text from `https://www.usenix.org/system/files/osdi20-narayanan_vikram.pdf` |
| `https://github.com/mars-research/redleaf` | the implementation | 2026-09-20, depth-1 clone of `master` |
| Burtsev, Appel, Detweiler, Huang, Li, Narayanan, Zellweger. *Isolation in Rust: What is Missing?* PLOS '21, Oct 25 2021. | the authors' own retrospective on what RedLeaf cost them | 2026-09-20 |
| Chen, Li, Mesicek, Narayanan, Burtsev. *Atmosphere: Towards Practical Verified Kernels in Rust.* KISV '23, Oct 23 2023. | what the group did next | 2026-09-20 |
| Chen, Li, Zhang, Narayanan, Burtsev. *Atmosphere: Practical Verified Kernels with Rust and Verus.* SOSP '25, Oct 13-16 2025. | the same, grown up | 2026-09-20 |
| Heiser. *The seL4 Microkernel: An Introduction.* seL4 Foundation whitepaper, Revision 1.4.1 of 2026-07-17. | the reference point this project is measured against | 2026-09-20, for the TCB definition and size only |

Everything quoted below is quoted from one of those seven. Nothing here is recalled. `script/citations`
validates in-tree numbered records and cannot check any of this, so the discipline is the author's
alone, which is why the table exists.

## What could not be established

- Why RedLeaf stopped. Nothing in the repository, the papers, or the group's pages says. The 35
  open issues were not read. Unknown, and it should stay unknown in this note rather than be
  inferred from a commit date.
- Whether the safe-Rust-only invariant was ever enforced anywhere. The measurement in
  [`the-trusted-build.md`](the-trusted-build.md) is of the public tree at its last commit. A stricter internal build may have existed. `redIDL` was not
  fetched and is the most likely place for such a check to live.
- Whether RedLeaf boots on anything current. Not attempted. It wants `nightly-2021-12-15`, `nasm`,
  `grub-mkrescue`, and CloudLab-class Intel hardware for anything but QEMU.
- The SOSP '25 Atmosphere evaluation beyond Tables 1-3. Its driver and application benchmarks were
  not read in full; only the verification-effort and syscall-latency tables were.
- How Atmosphere's authors would characterize the relationship between the two systems. Not asked,
  and not inferable from the text.
- Whether anyone has independently reproduced RedLeaf's 124-cycle figure. Not searched
  exhaustively.
