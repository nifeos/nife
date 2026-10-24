# What the Verus comparison read and ran, and when

*Name: provisional, for the source table it holds.*

An appendix to [`notes/verus.md`](../verus.md). It lists the papers, documentation and tool
releases the comparison rests on, with where each was fetched and the date it was read or run.

| source | what it is | read or run |
|---|---|---|
| Chen, Li, Mesicek, Narayanan, Burtsev. *Atmosphere: Towards Practical Verified Kernels in Rust.* KISV '23, Oct 23 2023, 8 pages. DOI 10.1145/3625275.3625401. | the first Atmosphere paper | 2026-09-20, full text from `https://mars-research.github.io/doc/2023-kisv-atmo.pdf` |
| Chen, Li, Zhang, Narayanan, Burtsev. *Atmosphere: Practical Verified Kernels with Rust and Verus.* SOSP '25, Oct 13-16 2025. DOI 10.1145/3731569.3764821. | the grown-up one, CC-BY | 2026-09-20, from `https://mars-research.github.io/doc/2025-sosp-atmo.pdf`. **That copy is 10 pages and the ACM reference block says 16**; it ends at Section 5 with no evaluation, no related work and no bibliography. `dl.acm.org` returns 403 to an automated fetch of both the PDF and the full-text HTML. |
| `notes/redleaf.md` (branch `maintainer/redleaf-comparison`) | the sibling lane, which read the full text | 2026-09-20, for Table 3 only, and attributed as second-hand below |
| Lattuada, Hance, Cho, Brun, Subasinghe, Zhou, Howell, Parno, Hawblitzel. *Verus: Verifying Rust Programs using Linear Ghost Types.* OOPSLA 2023, 7(OOPSLA1):286-315. | the tool's own paper | 2026-09-20, from `https://matthias-brun.ch/assets/publications/verus_oopsla2023.pdf` |
| The Verus guide, reference, `vstd` docs, and the `verus-lang/verus` source | what the project says about itself | 2026-09-20, `https://verus-lang.github.io/verus/guide/` and the GitHub tree |
| Verus release `0.2026.09.20.aef82ed`, `verus-arm64-macos` | the tool itself | 2026-09-20, downloaded, installed and run on this Mac. Every Verus error message quoted below is output this lane produced. |
| `cargo kani` 0.67.0, in this worktree, against `kernel/src` | the tool we have | 2026-09-20, three throwaway harnesses, reverted. Every Kani message quoted below is output this lane produced. |

`script/citations` validates in-tree numbered records and cannot check any of the external half, so
the discipline is the author's alone, which is why this table exists. Nothing here is recalled.
