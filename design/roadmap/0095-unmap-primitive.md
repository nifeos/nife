---
status: BUILT
raised: 2026-08-04
built: 2026-10-05
milestone_dependencies: none
decision_dependencies: 162, 249
machine_requirements: none
specific_machine: none
needs_person: no
---
# 95. An unmap primitive, and the mappings init never lets go

Raised 2026-08-04 from milestone 22's closing lane, which named it as the
largest residual left standing after the interactive boot gave away its authority.

An unmap method is a system-call-surface addition, which the block calls a design fork for calef before
it is a task. Does unmap belong on the address space or on the frame? What does it do to a mapping
another holder also has? Does restructuring the loader to map one
page at a time avoid the new method entirely? **It is
§162 (whether a holder can give up a mapping)**, decided as A, `AddressSpace::UNMAP(va)`, on
2026-10-05. It was written up 2026-09-19 by the lane of milestone 435 (forty-five milestones are
gated on a decision nobody wrote down), which found this gate naming no
decision though the block had
called it a fork since 2026-08-04.

**The finding.** `build_child` maps each page it lays down for a child into init's own address space
to write it. It never unmaps it, because nothing in the ABI can: there is no unmap.
Reaping a job hides the problem for jobs, since the revoke of §13 (capability revocation and untyped
reclamation) takes mappings with the region. But the
boot servers are never reclaimed, so init keeps a writable window onto every page of
every server it built, for the life of the machine.

That is a real hole in the story milestone 22 otherwise tells. Init drops its construction budget,
its device capability and its interrupt, and the kernel itself confirms `RETYPE` now answers
`NoSuchSlot`. And then init can still write into the filesystem server's text. The lane recorded it
rather than shrinking a budget to make the number look better. That is the right call, and it leaves
the work here.

What this milestone is. A way for a holder to give up a mapping it made. It is an unmap method on
the address-space object, symmetric with the map that created the window. That is a system call surface
addition, so it is a design fork for calef before it is a task. (CLAUDE.md: a new method is fine
within the model, but its semantics are recorded in `design/decisions/`, and a brand-new system call
number is a fork.) The fork has three questions to answer. Is unmap a method on the address space or
on the frame capability? What does it do to a mapping some other holder also has? (§13 already
decides revoke's answer, and this must not contradict it.) Is unmapping enough, or does the scratch
window want a narrower shape: mapping one page at a time and releasing it before the next? That
needs no new method at all if the loader is restructured instead.

That last possibility is why this is not obviously a system call. The cheapest fix may be a loader that
never holds more than one page. The measurement that decides it is how much slower a
one-page-at-a-time loader boots.

## Built, 2026-10-05 (UTC)

`AddressSpace::UNMAP(va)` is built on all three architectures (`abi::address_space::UNMAP`,
`kernel/src/system_call.rs`'s `address_space_unmap`), proven by `system_tests/src/user/unmap_tests.rs`
with a replayable falsification for each of its four tests. The two semantics §162 left owed are
built provisionally and argued in `notes/unmap.md`: a `va` with nothing mapped answers
`BadPointer`, and no capability is consumed. They are asked of calef on pull request #1678.

**It did not close this block's hole then** (kept as written; the second half below closed it). The
reason was a finding, not a gap in the build.
No capability names a running address space: `CONFIGURE` retires the name, and the kernel grants
none at boot. The progenitor's scratch window is in its own running space, so it has nothing to
invoke `UNMAP` on. That is a fork on the system call surface, written up as a proposal (below) and not
built.

## Built, the second half, 2026-10-05 (UTC): the windows are closed

On §249 (a running address space stays nameable), which calef ruled the same day as option A with
amendments (a) and (b). Its 2026-10-05 amendment put init's capability at slot 28. Lane
`lane/space-naming-build`, pull request #1692.

- A running space stays nameable. The address-space registry owns every space, bound or not.
  `CONFIGURE` consumes the capability it is passed and binds the space in place. So a copy made
  first keeps naming it while the thread runs. Each thread keeps a copy of its root, tag and
  current-CPU page (`user::BoundSpace`; provisional), so the context switch takes no registry lock.
  A space dies when its thread is reaped or the region sweep takes it, and every capability then
  fails. The two removals are one take-once `Table::remove`. `MAX_USER_SPACES` is now the
  revocation registry's `MAX_SPACES` (288), the most spaces that can exist at once. So a full
  registry is an impossible state rather than an error. It costs 21,904 bytes of `.bss`.
- A second bind is refused with `WrongObject`, from a bound mark on the registry entry, so §105
  (`std::thread::spawn` stays declined) stands.
- The kernel grants the progenitor its own space at slot 28, `WRITE` alone, on every boot
  (`boot_progenitor`'s `assert_eq!(s28, 28)`, `components/src/progenitor.rs`'s `own_space: 28`).
- The progenitor gives up each scratch page with `UNMAP` the moment the page is in the child
  (`supervision_protocol::give_up_own_page`, provisional). So do the shell's output page and the two
  DMA pages it used to keep for the life of the boot. The scratch window is never wider than the one
  page being filled. That is §162's optional C (a one-page loader) reached through A rather
  than built as a loader restructure.

The proof is `system_tests/src/user/running_space_tests.rs`: six tests, green on aarch64, riscv64
and x86_64 (the multicore one skips on x86_64's one-core UEFI image). Each has a replayable
falsification under `system_tests/falsifications/`, and all six replayed red on aarch64 on
2026-10-05. Two of them are what this block owed. One is the negative control: a
builder holding its own space, as the progenitor does, writes to the page it filled for a child and
faults at that address. Without its own space the same write lands. The other is the multicore test
`notes/unmap.md`'s `BUGS` said could not exist. A reader spinning on one core faults when another
core unmaps its page. `script/swish-check` passes on aarch64 and riscv64 with the boot giving every
window back. The capability-slot peak is one higher on every boot (32, 29, 25), which
`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` records. The costs are in `notes/unmap.md`:
`spawn_el0` +1.6% on aarch64, the switch rows unchanged.

## Follow-on

Checked against the tree on 2026-10-05 (UTC), on `lane/space-naming-build`.

- **Done.** How a running process names its own address space: §249, built above.
- **Done.** The progenitor giving up each scratch page and the negative control: above, and in
  `system_tests/src/user/running_space_tests.rs`.
- **Done.** The amendment to §162 recording the two semantics: §162's amendment of 2026-10-05.
- **Milestone 765.** A thread that can still run on a space rooted in a destroyed region, and the
  narrower windows registry ownership leaves. Recorded in the block of milestone 765 (a destroyed
  region cannot free the root a running thread walks), which builds the refusal.
- **Recorded.** The shell's job-frame window and `login`'s connect window still never shrink, since
  neither holds its own space. See `notes/unmap.md`'s `BUGS`, and each file's own comment.
- **Recorded.** One page per call, and the multicore falsification being aarch64's alone:
  `notes/unmap.md`'s `BUGS`.

## Index row

The largest residual of milestone 22 (trusted init): `build_child` mapped every page it wrote for a
child and nothing could unmap it. So init kept a writable window onto every boot server for the life
of the machine.
Closed 2026-10-05 (UTC): `AddressSpace::UNMAP` (§162) and a running space that stays nameable
(§249). Also init's own space at slot 28, and a loader that gives each page back the moment it is in the child.
