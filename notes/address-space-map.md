# The user address-space map

Every process on nife has the same low half, and several programs agree on where things go in it:
the linker script, two loaders, std's runtime, the kernel's test harnesses and every fixture that
shares a page. This page is the map they agree on, so a new page is placed by the map rather than
by looking for a gap. Milestone 206 (a program image has under 896 KiB) drew it, under DECISIONS
§171 (where a program image starts), option D, ruled by calef on 2026-09-26.
The map is a crate, [`crates/address_space_map`](../crates/address_space_map/src/lib.rs), because
it is a list of numbers several binaries agree on (AGENTS.md rule 7). This page explains it; the
crate is the rule.

## The map

| band | start | end | size | what goes in it |
|---|---|---|---|---|
| `NULL_GUARD` | `0x0000_0000` | `0x0020_0000` | 2 MiB | nothing, ever |
| `PAIR_PAGES` | `0x0020_0000` | `0x1000_0000` | 254 MiB | pages one builder agrees with the one program it builds, and a program's private windows |
| `RUNTIME_WINDOWS` | `0x1000_0000` | `0x4000_0000` | 768 MiB | windows a loader or runtime places for a program: std's runtime pages, std's socket frames, a builder's scratch cursor, the initrd window, the swap image |
| `HEAP` | `0x4000_0000` | `0x5000_0000` | 256 MiB | the heap and nothing else |
| `SERVICE_WINDOWS` | `0x5000_0000` | `0x6000_0000` | 256 MiB | large windows a service contract names: the block channel, the C seam's grant pages, the screen aperture |
| `IMAGE` | `0x6000_0000` | `0x7F00_0000` | 496 MiB | the program's code and data |
| `STACK` | `0x7F00_0000` | `0x7FFF_0000` | 16 MiB less 64 KiB | the stack, growing down; its lowest page is never mapped |
| `PROCESS_PAGES` | `0x7FFF_0000` | `0x8000_0000` | 64 KiB | pages the kernel maps into every process unasked; the current-CPU page is the last |

The bands tile the first 2 GiB with no gap. Above that the only resident is the timebase page on
`x86_64` and `riscv64` (`counter_frequency_contract::PAGE_VA`), which predates the map.

A program image may be up to 496 MiB. `ripgrep`, measured at 2.61 MiB by milestone 121 (`ripgrep`
on nife), fits with room for a hundred and ninety of it.

## Where a new page goes

Ask in order, and stop at the first yes. The crate's docs carry the same list.

1. Is it the program's code or data? `IMAGE`. A child built by hand from a stub puts its code page
   at `IMAGE_BASE` too.
2. Is it the stack? `STACK`, down from `STACK_TOP_PAGE`.
3. Does the kernel map it into every process without being asked? `PROCESS_PAGES`.
4. Is it the heap? `HEAP`.
5. Is it placed by a loader or a runtime for programs that did not choose the address? `RUNTIME_WINDOWS`,
   with a written siting argument in the protocol crate that owns it.
6. Is it a large window a service contract names for several programs? `SERVICE_WINDOWS`.
7. Otherwise it is a pair page: `PAIR_PAGES`. Two pairs never share an address space, so pairs reuse
   addresses freely.

Then write it with the band's function, so the compiler checks it:

```rust
const CTL_VA: u64 = address_space_map::pair_page(0x60_0000);
```

`pair_page`, `runtime_window` and `service_window` return their argument unchanged and stop the build
if it is outside the band or not page aligned. Every fixed window in `fixtures/`, `components/`, the
kernel's harnesses and the progenitor was brought under one of the three when the map landed, 188 of
them.

## Why these numbers

- **2 GiB is a hard ceiling for the image on `x86_64`.** The target builds with `code-model: small`
  and the static relocation model, which promise LLVM that every symbol fits a sign-extended 32-bit
  immediate. `riscv64` and `aarch64` would accept a higher image; one layout for three architectures
  is the parity rule.
- The heap and the block channel sit below the image band, and the heap's numbers are generated
  into std's PAL. The image band starts at the first address above them.
- **The stack sits directly above the image**, so the one limit on an image is the stack's base, and
  a refusal can say exactly that (below). The guard page at the bottom of `STACK` means a stack at its
  limit faults before it touches a full image.
- **16 MiB of stack band** is twice Linux's default main-thread stack. The deepest stack in the tree is
  std's 32 pages. Address space that nothing maps costs nothing.
- The process pages share the stack's last-level table. `notes/benchmarks/spawn-el0.md` measured
  the current-CPU page at 1,245 ticks per spawn in a table of its own and 741 sharing two levels.
  Beside the stack it shares all three and costs only its leaf.

### What it costs in page tables

Counted, not measured. A program with no pair pages (the supervision tree's children, the
benchmark's stub) needs as many tables as before. They are the root, one upper level, one table for
the second gigabyte, one leaf table for the image, and one for the stack and the current-CPU page.
Before, the image and stack shared a leaf table in the first gigabyte and the current-CPU page had its
own. A program with pair pages pays one more table than it did, because it now touches both
gigabytes. A std program pays one fewer, because its heap was already in the second gigabyte.

## A too-large image says so

`kernel::user::load` checks the image against the band before it maps anything, with
`address_space_map::check_image`, and refuses with `LoadError::Misplaced`. The boot prints it as a
sentence:

```
refused to load a user program: the program image is too large: it ends at 0x7f001000, past the
stack's base at 0x7f000000; an image may be at most 496 MiB
```

An image linked for the old layout at `0x40_0000` is refused as outside the band, naming the
base to relink at. `supervision_protocol`'s loader makes the same check, but it reports every failure
as `Err(())`, so there the check only makes the failure early and clean.

## What changed with it

- `kernel::user::USER_STACK_VA` and `supervision_protocol::CHILD_STACK_VA` are both the map's
  `STACK_TOP_PAGE`, so neither can move alone.
- `crates/user_mode_runtime/link.ld` links at `0x6000_0000`. A linker script cannot name a Rust
  constant, so the map's host test parses the script's `. =` line and fails if the two differ.
- basalt's build of `rg` and `cryptography_exerciser/build.rs` link with the shared script, at its base.
- The screen aperture is at `0x5800_0000`, outside the heap band.
- Hand-built children in the kernel's tests and in `fixtures/` put their code at `IMAGE_BASE` and
  their stack at `STACK_TOP_PAGE`, so their page tables have a real program's shape.
- `cargo xtask` checks every program it packs into an archive against the image band.
- The progenitor's job regions grew by the one table counted above: 41 pages, and 98 for a job
  that also builds a directory caretaker.

## The counter frequency page's address

`counter_frequency_contract::PAGE_VA` is `0x0000_7000_0000_0000` on `x86_64`, seven-eighths of the
low half's ceiling (`SPLIT_SHIFT` is 47, so the low half is every address under
`0x0000_8000_0000_0000`; see `crates/paging/src/x86_64.rs`). That leaves roughly 26 TiB of headroom
below the boundary, with none of this tree's other conventions near it. RISC-V under Sv39 splits at
bit 38 (`crates/paging/src/sv39.rs`), so an address at or above `0x40_0000_0000` is not a user
address at all, and `riscv64` takes `0x38_0000_0000` by the same seven-eighths rule.

The page is far above every other low-half address because it is mapped into every `x86_64` and
`riscv64` process unconditionally, rather than opted into by one program's own wiring. Any low-MiB
address might be exactly what some program's segments or some fixture's window wants. A first
attempt at `0x60_0000` collided with `fixtures/src/window.rs`'s `CTL_VA`, which a full-suite run
under `script/test --arch x86_64` caught as `AlreadyMapped`. That collision is why this map exists
(milestone 206 (a program image has under 896 KiB), DECISIONS §171 (where a program image starts)
option D). Under the map the page belongs in `PROCESS_PAGES`, beside the stack, sharing its page
tables. The crate's test module pins that it stays above every band meanwhile.

## BUGS

- Membership, not collision. The band functions check that a page is in its band. They cannot see
  two pair pages one program maps landing on each other.
- Two protocol crates restate their numbers. `std_runtime_contract` and `counter_frequency_contract`
  are generated into std's PAL, where no crate can be named, so each pins itself to the map with a
  host test instead of deriving from it.
- The timebase page still sits far above the map, paying three tables of its own in every `x86_64`
  and `riscv64` process. It belongs in `PROCESS_PAGES`; moving it is a std farm rebuild and a
  benchmark re-baseline, proposed in milestone 206's block as a follow-up.
