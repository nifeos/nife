# Prior art

An appendix to [the milestone](../1099-machine-statistics-contract-outside-review.md).

This table shows how other systems hand a program the machine's counters. Recent downloads are the last 90 days at
the time of the review. Details marked unverified come from memory of those sources.

| system | mechanism | consistency | who may read | shape |
|---|---|---|---|---|
| Linux `/proc/stat`, `/proc/meminfo` | text files, one `cpu N` line per processor, `ctxt`, `intr`, `procs_running` | each line gathered without a global lock; readers accept skew (unverified in detail) | every process, ambient | text a reader parses; grows by appending columns |
| Linux vDSO data page | a read-only page beside the vDSO, found through the auxiliary vector | a sequence counter; readers retry across an update (unverified in detail) | every process | binary; one layout per kernel, matched by the vDSO it ships with |
| FreeBSD `kern.cp_time`, `kern.cp_times`, `vm.stats` | `sysctl` | one call returns one coherent array | every process | binary structs, versioned by the `sysctl` name |
| OpenBSD `KERN_CPTIME2`, `struct uvmexp` | `sysctl` | one call | every process | binary struct |
| illumos `kstat` | named counters with a class and a type, read through `libkstat` (unverified in detail) | a snapshot per `kstat_read` | every process | self-describing names, so a reader need not know the layout |
| Plan 9 `/dev/sysstat` | one text line per processor: context switches, interrupts, system calls, faults, load, idle share | none across lines | every process in the namespace | text |
| Fuchsia `ZX_INFO_KMEM_STATS`, `ZX_INFO_CPU_STATS` | `zx_object_get_info` on a handle with the right | one call fills one struct (unverified in detail) | the holder of the handle | binary struct, versioned by topic |
| `sysinfo` 0.40 | reads the above per platform | per refresh | the process | 61.7M / 244.7M downloads |
| `procfs` 0.18 | parses Linux `/proc` | per read | the process | 23.4M / 95.5M |
| `systemstat` 0.2 | reads the above per platform | per read | the process | 1.13M / 6.04M |
| `psutil` 5.4 | ports Python's `psutil` | per read | the process | 1.06M / 4.81M |

Where this crate differs, on purpose, in four ways. First, it is a fixed-address, read-only page. The kernel writes it in place, and a program is granted it, never given it by default. Second, it uses binary `u64` words behind a magic and a version
word that a reader checks before trusting anything. Third, it has one writer per word with relaxed atomics and no
sequence counter, which is the bargain `/proc/stat` makes and the vDSO page does not. Fourth, it has one 64-byte
line per possible cpu, so no two cores write one cache line. Fuchsia is the closest shape, since
the right to ask travels with a handle. What it costs: the words are never a consistent snapshot
of each other. The fixed address and the line count are part of the identity once a program
outside this tree compiles them in. There is no user and system split. The layout's line count
comes from another crate (item 9).

The concrete example the page exists for: `free` on a machine with 32,768 frames of 4,096 bytes
and 30,000 free prints `Mem:` from four header words and nothing else. It needs no file to open and
no text to parse. A frame nobody prepared reads as no page rather than as zero bytes free.
