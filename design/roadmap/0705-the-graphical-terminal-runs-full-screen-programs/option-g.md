# Option G, measured: libvterm behind the C seam

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md).
It holds option G's measurements and its seven-question entry.
calef refused G on 2026-10-21 (UTC) ([the ruling](../0705-the-graphical-terminal-runs-full-screen-programs.md#ruling)).

Measured on 2026-10-18 (UTC) on patagonia, against `main` at `948806b32`, unless marked *from
memory*. The probes were throwaway and are not kept.

## What G is

`display_terminal` becomes a `std` program, as in F, and its engine is libvterm 0.3.3, a C library,
linked through nife's C library (milestone 835 (a C library, stage 1: files, clock and memory)). It
is option B's shape, a C engine behind the seam, without Zig.

## Results

| | libvterm 0.3.3 |
|---|---|
| Source | `libvterm-0.3.3.tar.gz`, released 2023-08-03; SHA-256 `09156f43...7226e0`, matching Homebrew's formula |
| License | MIT |
| Size of the source | 6,543 lines of C and headers |
| Compiles for the three nife targets | yes, with `helpers/c-library-cflags.sh` and `-std=c99 -Os` |
| Its own code | 41,804 bytes (aarch64), 37,196 (riscv64), 43,910 (x86_64) |
| Links into a nife program | yes, on all three, through `helpers/build-c-program.sh`; 165 to 184 KB of code with `std` and the C library |
| libc it needs | `malloc`, `free`, `memcpy`, `memmove`, `memset`, `strncmp`, `strncpy`, `snprintf`, `vsnprintf`, `fprintf`, `stderr`, `abs`, `abort`, `exit`; the C library has all of them |
| Part 2 checks (host) | 15 of 15 |
| vttest (host) | 23 of 24 screens; DSR 5 and 6, DA and DA2 answered ([the conformance appendix](conformance.md)) |
| Replies | its own output buffer, `vterm_output_read`, which is Part 3's `take_reply` as it stands |
| Damage | a callback per damaged rectangle, merged by the library on request |
| Heap per 80x24 terminal | 150,810 bytes, both screens, unchanged after 400 lines (a counting allocator) |
| Scrollback | the embedder's: rows leaving the screen arrive through a `sb_pushline` callback |
| Advisories | one, CVE-2018-20786: a NULL dereference on an allocation failure, fixed |

It ran on nife under QEMU on all three architectures (`script/test --test`), packed as a C program
on a local branch that was never pushed. The fifteen checks, ported to C, passed 15 of 15 on each,
and one 80x24 terminal held 150,810 bytes there as on the host. The whole process peaked at 101
heap pages (404 KB), `std` and the C library included.

`fprintf`, `abort` and `exit` are on paths that only a version mismatch, a failed resize or a debug
build reaches. Under §31 (the foreign-language seam) as first written, with no libc at all, the
component would have to supply those fourteen symbols itself. Through the C library it supplies
none.

## Against option B

Both put a C-ABI engine behind the seam. B needs Zig 0.16 in the build, a `std.Io`, an entropy
callback and an allocator, and its API is declared unstable. G needs the clang the C library
already uses, allocator callbacks (`vterm_new_with_allocator`) and nothing else. Its header checks a
version at runtime. Both have key and mouse encoders and reflow ([the upkeep appendix](option-g-upkeep.md));
G's conformance on the measured screens is already the best of the four engines.

## The cost of C here

Every program's output is this engine's input, and it is C. A memory bug in it is a bug in a
process that holds the scanout and its terminal endpoint, and no more, because that is all
`display_terminal` is granted. A compromised terminal could draw anything on its screen and forge
keystrokes into its own terminal. It could not reach another process. The CVE record above is
short. No upstream fuzzing was found.

## The seven questions

1. Considered instead: F, a Rust engine that ties on conformance and costs fifteen to twenty-seven
   times the heap. A, 500 to 900 lines of our own from 7 of 24 screens. B, the same shape with Zig
   and an unstable API.
2. The tree's analogous case: SQLite, ioping and hackbench are C linked through the C library, and
   §31 built the seam for a foreign engine.
3. Prior art read: the 0.3.3 source and header. That Vim and Neovim embed libvterm appears in the
   CVE's description for Vim; Neovim is *from memory*.
4. Is the premise true? Yes. It is sans-IO: bytes in, cells, damage and replies out, no syscalls.
5. Cost: the measurements above, and the same `std` port and slot moves as F. No crate is added.
6. Reversible as code. Taking it is §46 (thin primitives or whole subsystems)'s expensive category,
   as a vendored C tree.
7. Yes. At equal cost I would choose G over F for the heap and the size, with conformance tied. It
   is not about effort. Its real cost is C on untrusted input, bounded by what the process holds.
