# The incremental path: what was read, and when

*Name: provisional. After the parent's "What was read, and when". calef names files.*

An appendix of [the incremental path](../incremental-path.md). The parent quotes the paper and
the sources below; this file is the record of where each came from and the day it was read.

## What was read, and when

| source | what it is | read |
|---|---|---|
| Li, Miller, Zhuo, Chen, Howell, Anderson. *An Incremental Path Towards a Safer OS Kernel.* HotOS '21, Ann Arbor MI, May 31 - June 2 2021, pp. 183-190, 8 pages. DOI 10.1145/3458336.3465277. | the paper | 2026-09-21, full text from `https://sigops.org/s/conferences/hotos/2021/papers/hotos21-s09-li.pdf` |
| `https://rust-for-linux.com/rust-kernel-policy` | what the Rust-for-Linux project states as kernel policy | 2026-09-21 |
| `https://goals.rust-lang.org/2026/roadmap-rust-for-linux.html` | the Rust project's own 2026 roadmap item for the kernel | 2026-09-21 |
| `https://rusted-kernel.com/` | a per-release line count of Rust in the kernel tree | 2026-09-21, for the 7.2.4 figures only |
| Corbet. *A process for handling Rust code in the core kernel.* LWN, 2025-03-27, `https://lwn.net/Articles/1015409/` | the maintainer-side process question | 2026-09-21 |
| Edge. *Rust for filesystems.* LWN, 2024-06-21, `https://lwn.net/Articles/978738/` | the LSFMM+BPF 2024 session where filesystem maintainers answered | 2026-09-21 |
| `https://api.github.com/repos/smiller123/bento` | the paper's own exemplar implementation | 2026-09-21, for `pushed_at` only |
| `design/fatal-risks/README.md` at `b6d8fa2d` | this project's falsification list | 2026-09-21 |

Everything the parent quotes from the paper is quoted from that PDF. The title on the PDF's own
title line is *An Incremental Path Towards a **Safer** OS Kernel*. Search results disagree, and
several say "Safe". `script/citations` validates in-tree numbered records and can check none of the
external material. So the table is the only mechanism here, and the discipline is the author's.
