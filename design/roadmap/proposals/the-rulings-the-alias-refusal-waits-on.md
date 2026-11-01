---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The three rulings the writable-and-executable alias refusal waits on

The lane for milestone 969 (no frame is writable and executable in one address space) wrote
this on 2026-10-31 (UTC). Milestone 969 cannot start until an architect rules three things. This file sets each one out in the block's order. Each gets its background, the recommendation
and the other options, prior art, where nife differs, an example on the tenth pass's reproduction,
the cost, and what the ruling does to 969 and to claim 11. Once an architect rules, an integrator
folds them into 969's block and disposes of this file by promotion, as the proposals README requires.

This lane read every fact about the tree below at `main` 9f62334f5. It booted nothing and
measured nothing.

Reuse: the mapping log and the pass's witness. A kernel check, so nothing outside to take.

## The mechanism, once, for all three

A program sees memory through virtual addresses. The page tables translate each virtual page to a
physical page and attach permissions to that one translation, the leaf: readable, writable,
executable. W^X ("write xor execute") is the rule that no memory is writable and executable at
once, so a bug that lets an attacker write bytes cannot also make the processor run them.

nife enforces W^X per leaf. `Flags::user_code` grants read and execute, `Flags::user_data` grants read and write, and the Kani harnesses `no_encoded_leaf_is_both_writable_and_executable` prove on all three
ISAs that no encoded leaf has both. Claim 11 in `notes/confinement-claims.md` says more: "No page
is both writable and executable."

The gap is that two leaves can name one physical page. `AddressSpace::MAP_INTO` maps the frame a
capability names at the address the caller picks, and nothing asks whether the space already maps
that frame elsewhere. The tenth pass mapped one `READ | WRITE` frame `MAP_RW` at one address and
`MAP_CODE` at the next. Each leaf is honest; the page is both. A writer stores code at one and runs it at the other.

The kernel already keeps the record a check would read. Every mapping of a frame into any space
files a `(phys, va, object)` entry in that space's mapping log (`kernel/src/revoke.rs`), so revocation can find
it. The log stores no permissions, but the page tables do, and a walk of one leaf answers that.

The reproduction, from `notes/confinement-outsider-pass-10.md`: on aarch64 the frame was writable
at `0xffffffffe000` and executable at `0xfffffffff000` in one space; riscv64 and x86_64 showed the
same pair at their own top user pages. The witness is
`tenth_pass_map_gate_tests::one_physical_page_is_never_writable_here_and_executable_there`.

### Who can reach it, read against the tree

The block says a builder reaches it. A running, confined, shipped component reaches it on itself.
`system_initializer::children` hands `net_stack` a `SPEND_ONLY` budget and `WRITE` on its own address
space, and hands `domain_name_resolver` a budget with every right and the same. `SPEND_ONLY` is every
right but `GRANT`, so a frame retyped from that budget carries `READ | WRITE`. §249 (a running address
space stays nameable) lets `MAP_INTO` reach a running space. So a compromised network stack, the
component that parses bytes off the wire, can give itself a page it writes at one address and runs
at another. The lane read this; it did not boot it.

## Question 1. How wide is the refusal?

### Background

When a second mapping of a frame arrives, the kernel compares it with the mappings the space
already has of that frame. The question is which differences refuse. Only one pair breaks W^X:
writable on one side, executable on the other. Other differences are harmless to W^X. A frame
read-only at one address and read-write at another is how a program keeps a read-only view of its
own buffer. A frame mapped twice with the same permissions is how a ring buffer makes its end wrap
to its start.

### Ruled 2026-10-31 (UTC): (a) refuse only the writable-and-executable pair

The kernel refuses a new writable mapping when the space maps the frame executable anywhere,
and a new executable mapping when the space maps it writable anywhere. "Writable" means what
`Flags::is_writable` says, which covers `user_data`, `user_device` and `user_write_combining`.

- (a) The W and X pair only. Ruled. It refuses exactly what the claim forbids and nothing a
  program could want.
- (b) Any difference in flags, the rule the block's title states. It refuses read-only beside
  read-write, and read-only beside code, neither of which defeats W^X. Its one real argument is the mismatched memory types Arm warns against. That cannot arise here: a `DeviceFrame` and a
  `PageFrame` never name the same physical page, so every `PageFrame` leaf has the same memory type.
- (c) Any second mapping of a frame in one space, whatever its flags. The simplest check (any entry
  in the log refuses) and the widest refusal: it breaks the ring-buffer double map.

### Prior art

- OpenBSD, since 6.0 (2016): `mmap` and `mprotect` refuse `PROT_WRITE | PROT_EXEC` on one mapping
  unless the binary carries `wxneeded` and its filesystem has the `wxallowed` mount option. The rule is the
  pair, not any difference. From memory, and uncertain: OpenBSD does not refuse two mappings of one
  shared memory object, one writable and one executable.
- Linux permits writable-and-executable user memory by default. SELinux `execmem` refuses executable
  anonymous memory and `execmod` refuses executing a modified file mapping (from memory). Both
  govern the pair, never read-only beside read-write. PaX `MPROTECT` refused the pair and also
  executing a page that had been writable (from memory).
- seL4 enforces no W^X at all (from memory). A frame capability carries rights, and whoever
  distributes capabilities sets the policy.
- macOS hardened runtime refuses writable-and-executable memory unless the program holds the
  `allow-jit` entitlement and asks for `MAP_JIT` (from memory). Again the pair.

No system found here refuses read-only beside read-write.

### Where nife differs

nife states W^X as a claim with a falsification, not as a hardening option, so the refusal must be
total for the pair. It also has a reverse map in every space already: the mapping log, which plays
the part of Linux's `rmap`. A check costs a scan of one log, not a new structure.

### Example

The witness maps the frame `MAP_RW` at `0xffffffffe000`, then `MAP_CODE` at `0xfffffffff000`. Under (a), (b) and (c) alike the kernel refuses the second call and the page stays writable only; the witness
goes green. The rulings differ on a third call the witness does not make. `MAP_RO` of the same frame
at `0xffffffffd000` succeeds under (a) and fails under (b) and (c).

### Cost

- Kernel: under all three, one scan of the space's log per run, comparing `phys`. Under (a) and (b),
  a leaf walk (`mmu::translate_at`) for each entry that matches, which is rare. (a) can also skip the
  scan for a read-only mapping, which cannot complete the pair; the kernel maps its initial ramdisk
  window `user_rodata`, page by page. Nobody has measured the cost. Shortening the log page from 255 entries to 170 moved
  `map_el0` by 16.9% on aarch64 (`notes/mapping-log.md`), so a full-log scan will show in `map_el0`.
  969 records it from `script/bench`.
- Shipped programs: under (a), no program in the tree depends on a writable-and-executable pair in one space,
  as far as a read finds. Under (b) and (c), the lane runs the suite to find any same-space double
  map; the lane knows of none, and it has not searched exhaustively.
- Proof harnesses: one pure predicate in `crates/paging`, which takes the existing leaves' flags
  and the new flags and returns whether to admit, with a Kani proof that no admitted set holds both
  W and X. Under (c) the predicate is "no existing leaf", which is trivial.

### What it does to 969 and to claim 11

969's size barely moves. Claim 11 becomes falsifiable by removing the check, and
the witness turns red again: (a) restores the claim exactly as written. (b) and (c) restore it and
add refusals the claim never asked for, which would want their own row.

## Question 2. Where does the check live?

### Background

Four kinds of code install a user leaf. `AddressSpace::MAP_INTO` maps a frame into a space a capability
names. `PageFrame::MAP` maps a frame into the caller's own space, read-only or read-write.
`MemoryRegion::MAP` retypes a fresh page and maps it read-write. The kernel's own loaders map pages into spaces they build. All
of them take the mapping hold (`revoke::MappingHold`, one lock across every space's log), and all of
them file their record through `MappingHold::record_mapping`.

The block says the hold's entry is `map_physical_held`. That is true of `MAP_INTO` and the loaders,
but not of `PageFrame::MAP`, which installs through `mmu::UserMapBatch::map` and records directly,
nor of `MemoryRegion::MAP`. The one point all four reach is `record_mapping`, and today it runs after
the leaf is live.

### Ruled 2026-10-31 (UTC): (b) the hold, with the record filed before the leaf

`MappingHold::record_mapping` gains the check and moves ahead of the leaf, the order
`MappingHold::retype_table` already uses for page tables. Every path records, then maps, then
forgets the record if the map fails. A path cannot install a recorded leaf without passing the check.

- (b) The hold. Ruled, because the dispatcher alone leaves a reachable hole (the example below).
- (a) The dispatcher, in `address_space_map_into`. One call site. It sees every mapping already in
  the log, however made, so it catches writable-then-code. It misses code-then-writable when the
  writable half comes through `PageFrame::MAP`.
- (c) Both. More code for the same guarantee as (b).

A check in `record_mapping` without the reorder leaves the new leaf live for an instant, and
another core running the same space could fetch through it.

### Prior art

Linux runs its W^X policies as LSM hooks (`security_mmap_file`, `security_file_mprotect`) that
every mapping path reaches, not in each system call (from memory). OpenBSD's check sits in
`uvm_map` and `uvm_map_protect`, the two places a mapping gains permissions (from memory). Both check where every path converges.

### Where nife differs

nife's paths converge on one record function, not one install function, so the record becomes the gate.

### Example

The reverse order, on the reproduction's addresses. A holder of `WRITE` on its own space (the
progenitor at slot 28, `net_stack`, `domain_name_resolver`) maps the frame `MAP_CODE` at
`0xfffffffff000` through `MAP_INTO`, then calls `PageFrame::MAP` writable at `0xffffffffe000`. Under
(a) the second call never meets the check and succeeds: the page is writable and executable. Under (b) the hold refuses it. 969 adds this order as a second witness beside the pass's.

### Cost

- Kernel: under (b), `record_mapping` takes the new mapping's flags, and four call sites reorder to
  record first and forget on failure. `MemoryRegion::MAP` maps a page nobody else can name, so it
  can pass a flag that skips the scan. The loaders gain the check at boot, at the scan's cost.
  Under (a), one function changes.
- Shipped programs: none under either.
- Proof harnesses: the same predicate as question 1. The system test gains the reverse-order witness
  on three ISAs. The falsification patch removes the check from `record_mapping` and turns both
  witnesses red.

### What it does to 969 and to claim 11

(b) makes 969 larger, by four call sites and one reorder, and restores claim 11 for every path. (a)
keeps 969 small but leaves the claim falsified by the reverse-order witness, so the row would have to
narrow to "through `MAP_INTO`". That narrowing has no principle behind it.

## Question 3. Does claim 11 cover an alias across address spaces?

### Background

A check inside one space cannot see the same frame writable in a second space and executable in a
first. Loading a program works exactly this way, on purpose. `process_builder::fill_and_map` maps a
fresh frame writable in the builder's own space, copies the segment in, and maps the frame into the
child with `MAP_CODE`. Only a builder that named its own space then unmaps its writable view, and among shipped
programs that is the progenitor alone. It unmaps after the code mapping, not before. Every other builder keeps that view until a `DESTROY` of the child's region takes it back.

Two fixtures go further. `os_primitives_benchmarker` and `job_mix_task` write
`child_stub::SEND_THEN_EXIT` once into a frame mapped writable in their own space and alias it as
code into every child they spawn, with the writable view live while the children run.

Whether this matters depends on who writes. A builder already holds `WRITE` on its child's whole
space, so a writable alias gives it nothing new. It matters when
the writer is less trusted than the runner: a peer sharing a buffer that the runner maps as code.
`MAP_CODE` needs only `READ` on the frame, so a component holding `READ` on a peer's buffer and `WRITE`
on its own space can do that.

### Recommendation: (a) claim 11 is per address space, and the cross-space alias is a matter of authority

Claim 11 reads "no page is writable through one mapping and executable through another in the same
address space". The row states, beside it, what it does not claim: a holder of `WRITE` on a frame can
change code that another space runs, and the capability, not the kernel's W^X, decides who that is.
The claim already has a silent limit of this kind. The kernel's own direct map is a writable alias of
every user code page, at the kernel's privilege.

- (a) Per space, stated. Recommended.
- (b) Global: no frame is writable in any space while it is executable in any space. The kernel scans
  every space's log on each writable or executable map. Builders must unmap their writable view
  before `MAP_CODE`, which needs a capability to their own space; `process_builder` reorders, and
  every builder without one fails to spawn.
- (c) Global once the runner starts: the writable side may exist while a builder fills the child, and the
  thread-start method refuses while one survives. It matches loading best.
- (d) Per space now, global later as its own milestone, after `map_el0` and `spawn_reap` say what the
  global scan costs.

### Prior art

- seL4: a frame capability maps into many VSpaces with different rights, and the kernel imposes no
  W^X across them (from memory). Authority decides.
- Capsicum on FreeBSD: `CAP_MMAP_W` and `CAP_MMAP_X` are rights on a descriptor, so two descriptors
  of one shared memory object can map a writable view and an executable view (from memory). The
  rights on the handle are the policy, the same shape as nife's frame rights.
- Fuchsia: executing needs `ZX_RIGHT_EXECUTE` on the VMO handle, minted through
  `zx_vmo_replace_as_executable`, and a VMAR created with `ZX_VM_CAN_MAP_EXECUTE` (from memory).
  Whether Zircon refuses a writable mapping of a VMO another process maps executable: uncertain,
  and believed not.
- Linux's `text_poke` writes kernel code through a temporary separate address space while it
  stays executable in the kernel's own (from memory): a brief cross-space alias. On arm64,
  `rodata=full` makes the linear-map alias of kernel text read-only, the same-space fix.
- macOS on Apple Silicon: a `MAP_JIT` region switches per thread with `pthread_jit_write_protect_np`,
  so one thread can write a page while another executes it (from memory). Apple accepts writable and
  executable at once across execution contexts, and refuses it within one.

The pattern: W^X holds inside one context, and authority governs sharing across contexts.

### Where nife differs

nife can see across spaces, which most of these cannot cheaply: the mapping log already records every
mapping of a frame in every space, under one lock. So (b) and (c) are buildable, not hypothetical.
The question is whether the guarantee is worth a scan of every log inside the global lock.

### Example

Take the reproduction across two spaces. Space S1 maps the frame `MAP_RW` at `0xffffffffe000`; space S2
maps it `MAP_CODE` at `0xfffffffff000`. Under (a) both succeed and claim 11, as restated, holds. Under (b) the kernel refuses the second. Under (c) both succeed, and S2's first thread fails to start until S1 unmaps.
`fill_and_map` is this example with S1 the builder and S2 the child.

### Cost

- Kernel: (a) nothing beyond questions 1 and 2. (b) a scan of every live space's log per writable or
  executable map, inside the global hold, unmeasured, and it lengthens a critical section every core
  contends for. (c) the same scan at thread start instead of at map.
- Shipped programs: (a) none. (b) `process_builder::fill_and_map` unmaps before it maps code, and
  every builder needs a `WRITE` capability to its own space; the two fixtures restructure and the
  spawn benchmark is re-measured. (c) the builder unmaps before start, a smaller change, and the
  fixtures still restructure.
- Proof harnesses: (a) the row's wording. (b) and (c) a cross-space witness on three ISAs and its
  falsification.

### What it does to 969 and to claim 11

(a) leaves 969 as questions 1 and 2 size it. Claim 11's falsification is the witness and its patch,
and the row gains its stated limit. (b) roughly doubles 969, adds a published-number change to
`spawn_reap`, and keeps the claim's wording. (c) is between them, and it touches the thread-start path,
a syscall-surface change an architect rules on separately.

## The order to ask them

Question 2 first: the reverse-order example settles it from the tree, and it corrects the block's
premise. Question 1 second: it is a short choice once the reader understands the alias. Question 3 last: it is
the only one about what the claim means, and its answer leaves the other two standing.

## A finding this lane does not build

A component holding `WRITE` on its own space and a `READ | WRITE` frame can write code and later run
it without any alias. It maps the frame writable, writes, unmaps, then maps it `MAP_CODE`. Claim 11 is
about one instant and survives this. OpenBSD's rule does not cover it either; PaX `MPROTECT`
refused it (from memory). `net_stack` and `domain_name_resolver` hold that authority today.
[`a-confined-component-holding-its-own-space-can-load-code`](a-confined-component-holding-its-own-space-can-load-code.md)
proposes the question for its own ruling.
