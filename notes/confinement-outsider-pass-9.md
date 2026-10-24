# Confinement outsider pass 9 (2026-10-13 UTC)

The ninth outsider pass at risk 7 (the confinement claim is false), milestone 914 (a ninth outsider
pass attacks the confinement claim), whose number is provisional. It is the Claude cloud-runner half
of the pair calef ruled on 2026-10-13 (UTC), recorded in milestone 913 (an eighth outsider pass
attacks the confinement claim): two passes at one pinned commit, one of them non-Anthropic.

Blind, milestone 633 (an outside agent attacks the confinement claim)'s posture. The maintainer
started a fresh cloud session with the one prompt `briefs/confinement-outsider-pass-launch.md`
assembles: the reviewer's brief, the claims table inline and the runner section. The lane did not
attack and did not hint.

**Status: no escape, one near miss that is already tracked. 20 of the 27 bootable rows count as
booted, short of the floor. It was served by Claude Opus 4.8, not the Claude Fable the milestone
names. It does not count toward criterion (c).**

## Why it does not count

Two reasons, either one enough.

- **The model.** 914 is a Fable pass. The session was configured for `claude-fable-5-1` and ran as
  `claude-opus-4-8`: the session's `session_context.model` and its `last_served_model` both say so.
  An Opus pass is still an Anthropic pass, so it could only ever be criterion (c)'s Anthropic half,
  but it is not the pass calef ruled on.
- **The floor.** The brief's floor is 27 of 34 booted. By the brief's own counting this pass booted
  20. The report's own total is 23 boot-grade rows plus rows 1 and 3, because it counts rows 19, 20,
  24, 25 and 34, where the reviewer reran the row's existing hostile client. The brief says a rerun
  of the row's own cited test is not an attack and does not count. Rows 31 and 32 the reviewer graded
  untestable.

| Counts as booted | Rows | Count |
|---|---|---|
| Fresh hostile test, first round | 2, 4 to 11, 26 | 10 |
| Fresh hostile test, second round | 21, 22, 23, 27, 28, 29, 30, 33 | 8 |
| Pass 7's tests, accepted in writing | 1, 3 | 2 |
| Booted | | 20 of 27 |
| Rerun of the row's existing test, not counted | 19, 20, 24, 25, 34 | 5 |
| Untestable, not counted | 31, 32 | 2 |
| Host proofs, exempt | 12 to 18 | 7 |

Rows 1 and 3 count as the brief allows, though the written acceptance is thin: it names what each
pass-7 test attacks and says little about why that is enough.

## The run

- Sessions. Cloud session `session_0198ZNJVhj6z8jdsXyKHPH64`. An earlier launch,
  `session_01LfGexQwi8oVcvbnU2QmNZj`, went out with a placeholder prompt and was abandoned before it
  did any work. The session cost about $33.
- Two rounds. The first round booted fresh tests for 10 rows, accepted rows 1 and 3, read the
  rest and pushed at 20:55 UTC. On the person's instruction, the maintainer then sent the same
  session one follow-up asking it to boot rows 19 to 25 and 27 to 34. The follow-up named rows and
  carried no finding. The second round finished at 22:36 UTC.
- Commit. `643e0bef86a626d3be9e77e5369ee4c73e47d814`, the merge of #2035 that brings milestone
  824 (a retype mints no right its budget lacks) to main, the commit 913 attacks.
- Results. Eight modules under `system_tests/src/user/`, named `ninth_pass_*` on main (names
  provisional). This note carries the reviewer's report; the report itself is not kept in the tree.
- Trailers. Every reviewer commit on main ends with `Co-Authored-By: Claude Opus 4.8` and the
  cloud session's `Claude-Session` link, naming the model that served it.
- Host. Claude's cloud runner: `uname -m` x86_64, `nproc` 4, 28 GB free on `/`. Every leg ran
  under TCG. Setup matched 914's dry run: `script/ci-qemu` about 420 s after one `apt-get update`,
  bootstrap 47 s, and the cold `seventh_pass` aarch64 leg 405 s. No leg timed out and no QEMU was
  left running.

Leg wall times, per module, warm:

| Module | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `ninth_pass_confinement_tests` | 29 s | 79 s | 130 s |
| `ninth_pass_supervision_tests` | 33 s | 33 s | 69 s |
| `ninth_pass_mapping_tests` | 31 s | 31 s | 66 s |
| `ninth_pass_revocation_tests` | 21 s | 71 s | 117 s |
| `ninth_pass_elf_tests` | 26 s | 33 s | |
| `ninth_pass_el0_tests` | 28 s | 32 s | |
| `ninth_pass_port_tests` | | | 72 s |
| `ninth_pass_authority_tests` | 34 s | 33 s | 75 s |

## Reproduced on patagonia

Every booted row's test reran on patagonia (Apple Silicon, macOS, QEMU 11.1.2, every leg under
TCG), one ISA at a time with `script/test --arch <isa> --no-host --test <filter>`, first at the
pinned commit and then on main. Every test held on both.

Wall times include the build.

| ISA | Tests | Wall time, pinned commit | Wall time, main |
|---|---|---|---|
| aarch64 | 13 of 13 | 132 s | 127 s |
| riscv64 | 13 of 13 | 47 s | 35 s |
| x86_64 | 18 of 18, on the direct and the OVMF leg | 62 s | 56 s |

The reviewer graded rows 21 and 22 on aarch64 and riscv64 only, and their modules' comments said
x86_64 packs no initial ramdisk. It packs one, so the three ELF forgeries and the kernel-text read
run on x86_64 as well, and hold there.
The x86_64 fault is a page fault whose error code says a user-mode read of a present page. The four
port-range tests (rows 27 to 29 and 33) and the port-range arm of row 30 are x86_64 only.

## The claims

Grades: `booted` means a hostile test written for this pass ran under QEMU, on the ISAs named.
`accepted` means pass 7's test, accepted in writing. `rerun` means the row's existing hostile client
booted green, which the brief does not count. `read` and `host` mean source review only.

| # | Attack | Outcome | Grade |
|---|---|---|---|
| 1 | Pass 7's `SEND_CAP` widening test, read and accepted | held | accepted |
| 2 | A badge mints no wider right; an empty or out-of-range slot names nothing | held | booted, three ISAs |
| 3 | Pass 7's `SPLIT` and `RETYPE` test, read and accepted | held | accepted |
| 4 | Retype a frame, slice once, delete, re-invoke: `NoSuchSlot` | held | booted, three ISAs |
| 5 | File three, delete the middle; the others still resolve and invoke | held | booted, three ISAs |
| 6 to 9 | A real supervised child, reaped and surveyed through an unrelated `READ \| ENUMERATE` endpoint: refused; through its own: allowed; a made-up tid leaks nothing | held | booted, three ISAs |
| 10 | Confined `MemoryRegion::MAP` at a kernel-half and an unaligned address: `BadPointer` | held | booted, three ISAs |
| 11 | No mapping `Flags` is both writable and user-executable | held | booted, three ISAs |
| 12 to 16 | The VT-d encoder and the DMA validator, reviewed | held | host |
| 17 | `shadow_one_head`'s two unsynchronized stores, reviewed | near miss, tracked | host |
| 18 | The component plan's mapping of Use and Serve to rights, reviewed | held | read |
| 19 | `dir_capability_tests`: parent, sibling and `..` from a `sub` capability | held | rerun, aarch64 |
| 20 | `c_seam_tests`: a C out-of-bounds write faults | held | rerun, aarch64 |
| 21 | The `outlaw` reads a kernel code address, not the rodata static the suite reads | held | booted, three ISAs |
| 22 | Two-segment ELF forgeries (benign, then W+X or kernel-half) and a wrapping segment | held | booted, three ISAs |
| 23 | A full budget builds, is deleted, and the same builds answer `NoSuchSlot` | held | booted, three ISAs |
| 24 | `shell_navigation_tests`: two roots cannot name each other's files | held | rerun, aarch64 |
| 25 | `compositor_tests`: no neighbor's pixels, no screen | held | rerun, aarch64 |
| 26 | `BADGE` is `GRANT`-gated, write-once, and mints no wider right | held | booted, three ISAs |
| 27 | A holder of exactly the COM1 range writes the port one past it: #GP | held | booted, x86_64 |
| 28 | A parked holder revoked by the sparing `revoke_port_range_from_others` faults on wake | held | booted, x86_64 |
| 29 | A holder that deletes its own port capability faults on its next `out` | held | booted, x86_64 |
| 30 | A `DeviceFrame`, and on x86_64 a `PortRange`, parked in the hand-off slot through a revoke | held | booted, three ISAs |
| 31, 32 | The `script/swish-check` censuses; no hostile program written | held by the census | untestable |
| 33 | A `READ`-only holder writes a port inside its range: #GP | held | booted, x86_64 |
| 34 | `net_confinement_tests`: a socket squatter at a shared stack | held | rerun, aarch64 |

Row 30 was the sharpest new attack. `revocation_in_flight_tests`' own `BUGS` says its `DeviceFrame`
and `PortRange` hand-off-slot sweeps are reasoned from the code, not driven. Pass 9 drove both, and
the receiver was handed nothing on every ISA.

## The near miss

Claim 17. `shadow_one_head` in `crates/direct_memory_access_validator` publishes a validated
descriptor as two 64-bit stores with no barrier, so a device that still holds a republished head can
see a new address with an old length. The reviewer found it by reading the crate, whose `BUGS`
section records it and names milestone 799 (the shadow descriptor is published in two stores). A
`BUGS` section is fair game under the blind posture, so it is a re-discovery of a tracked hazard,
not a hint the posture forbids. Nothing was booted.

## Refusal log

None. The report says no step was declined in either round and no safety system or classifier
stopped the session. The reviewer chose not to write new userspace programs for rows 19, 20, 24,
25, 31, 32 and 34, saying a new program role is a naming decision. That is scope, not a refusal,
and it is why those rows fall short.

## What remains

- A Fable pass. 914's pass is still to run, on Fable, held to the same floor at the same pinned
  commit.

## The kernel tests

The pass's booted rows live in eight modules under `system_tests/src/user/`, each named
`ninth_pass_*_tests.rs`. Each module's doc names its claims, and each test's doc says which angle it
adds to the suite. They share one shape.

Most reduce a hostile program to the kernel operations it would issue, driven through the real
dispatcher (`syscall::invoke`) the way a program's `svc` or `ecall` reaches it. The ELF tests call
the loader `exec` uses, and the EL0, port and supervision tests boot real ring-3 children. Nothing
blocks: no test issues a `SEND`, `RECEIVE` or `CALL` with no peer, and `SLICE`, `BADGE`, `REAP` and
`SURVEY` do not park. So a regression is red at an assertion rather than a watchdog hang. Every
capability is deleted and every region reclaimed, so the suite's frame and region ledgers are flat
across each module.

The x86 port children are built in one region each, the way `x86_port_tests` and
`supervision_tests` build theirs. They reuse the architect-ratified stub builders in `x86_programs`,
so the pass minted no ring-3 machine code, only new parameters and scenarios. `x86_port_tests`
itself drives the paths found by milestones 299 (the x86 port-range capability), 313 (userspace
confinement, read adversarially) and 768 (a read-only port range grants nothing).

## BUGS

- The reviewer graded rows 19, 20, 24, 25 and 34 booted for a rerun, against the brief, and the
  report's own totals rest on that. Read its counts with this note's table.
- Rows 27 to 29 and 33 apply to x86_64 only.
- Blind is honor only in the runner: `git fetch` reaches every branch. The report says it kept to
  its own branch, and nothing enforced that.
