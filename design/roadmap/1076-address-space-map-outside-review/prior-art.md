# Prior art

An appendix to [the milestone](../1076-address-space-map-outside-review.md).

How the crate sits against what a reader of other systems already knows. Download figures are the
crates.io API's recent (last 90 days) and all-time counts at the time of the review.

| where | what fixes the user layout | the analogue of `PROCESS_PAGES` | what a reader coming from it will miss here |
|---|---|---|---|
| FreeBSD amd64, `sys/amd64/include/vmparam.h` | `VM_MAXUSER_ADDRESS`, `USRSTACK`, `SHAREDPAGE` as macros, one per architecture; `USRSTACK = SHAREDPAGE = VM_MAXUSER_ADDRESS - PAGE_SIZE` (verified at head) | the shared page, one page below the top of user space, holding the signal trampoline and the timekeeping page | randomization of stack and mmap base; a layout that differs per architecture |
| Linux x86-64, `arch/x86/include/asm/{elf,processor,page_64_types}.h` | `TASK_SIZE_MAX`, `ELF_ET_DYN_BASE`, `TASK_UNMAPPED_BASE` as macros (verified at head); the per-architecture memory map is prose in `Documentation/arch/x86/x86_64/mm.rst` (unverified) | the vDSO and vvar pages, placed by the loader at a randomized address; the fixed vsyscall page is legacy | ASLR; a 47-bit or 56-bit user half; a layout the kernel owns and user space discovers at run time through the auxiliary vector |
| seL4 | none: the root task and its loader decide every address (from memory, unverified) | none; the IPC buffer is wherever the thread's creator put it | a fixed map at all |
| Fuchsia / Zircon | none fixed; the process's root VMAR is randomized and the vDSO is mapped by the loader (from memory, unverified) | the vDSO | the same |
| Redox, `redox_syscall` 0.9 (193M / 857M) | user-space constants live in `redox-rt` and the kernel's `PML4` split (unverified detail) | none fixed | compile-time checked window placement |
| rust-vmm `vm-memory` 0.18 (3.2M / 9.8M) | `GuestAddress` plus `GuestMemoryRegion`s, ranges as data checked for overlap when the memory is built | none | a `const fn` check at the use site; a fixed table |
| `x86_64` 0.15 (1.0M / 5.9M) | `VirtAddr::new` refuses a non-canonical address at construction | none | bands; anything about where a page belongs |
| `memory_addr` 0.4 (14.7k / 184k) | `VirtAddr`, `PhysAddr`, alignment helpers, no layout | none | the same |

Where this crate differs: one layout, identical on three architectures, that tiles the first 2 GiB with no gap and carries a proof for each address. A `const fn` places a page and stops the build when the page is out of its band. A program declares an interface identity, and a kernel can check it at spawn. A loader refusal names the size limit instead of the first page that collided. What it costs: no randomization (item 12), and a 496 MiB image ceiling that `code-model: small` on `x86_64` imposes on all three architectures. The concrete example is the one that produced the crate. A timebase page placed by hand at `0x60_0000` collided with a fixture's control page at the same address. Only a full test run found it, as `AlreadyMapped`. Under the map the same placement fails to compile.
