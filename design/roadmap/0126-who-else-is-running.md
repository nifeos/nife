---
status: BUILT
raised: 2026-08-14
built: 2026-09-26
---
# 126. The `procps` package, now `process-tools`: who else is running, and who is allowed to ask

Minted 2026-08-14 by calef, from a design conversation about what ambient
authority utilities become on this system. Scoped to the whole package by calef the same day, for
consistency with how milestone 123 (somebody else's software, running narrow) approaches popular
packages. The corpus is chosen by an external ordering and taken in the units that ordering uses,
which is packages rather than programs we like. Re-swept and condensed 2026-09-26 by
`milestone/126-procps`, which found three claims here stale and one premise false (see "Corrections,
2026-09-26"). `free`, `vmstat` and `slabtop` built the same day by `milestone/126-free`; the
three rows that needed a ruling moved out that day (the table says where).

Renamed `process-tools` by calef on 2026-10-06 (UTC); this account keeps `procps`.

A program does one and only one thing (calef, 2026-09-26: *"One thing I like about unix is that a
program does one and only one thing."*). It decided `pidwait`, and it is the test for every row this
package still has open. Where upstream folds two jobs into one binary or one flag, this package
ships two programs, provided they hold different authority (milestone 281's rule).

## Where the package stands

The package is what `dpkg -L procps` installs, checked rather than remembered on 2026-08-24: `podman
run --rm ubuntu:24.04`, then `apt-get update`, `dpkg -L procps` and `dpkg -s procps` against the
live archive (`procps 2:4.0.4-4ubuntu3.2`, `arm64`, matching this project's `ubuntu-24.04-arm` CI
runners). It installs eighteen names under `/usr/bin` and `/usr/sbin`. `pkill` is a symlink to
`pgrep` and `snice` to `skill`; `pidwait` is a distinct binary; `sysctl` is under `/usr/sbin`. The
package's long description omits `pidwait`, which is how the first table here missed it.

| name | state | where the account lives |
|---|---|---|
| `ps` | built 2026-08-16; a `TIME(ms)` column since milestone 282 | `crates/ps`, notes/process-view.md |
| `pgrep` | built 2026-08-17; no pattern can be typed yet (milestone 47 (navigation and naming)) | `crates/pgrep`, notes/process-view.md |
| `pmap` | built 2026-08-23 (§114 (`ENUMERATE` extends to the address-space object)); reaching it from the prompt moved out, pull request #1365 | `crates/pmap`, notes/process-view.md |
| `uptime` | built 2026-08-26, needing no capability | `crates/uptime` |
| `top` | built 2026-09-21 by milestone 282 (a thread's CPU time) | `crates/top` |
| `watch` | built 2026-08-24, cut 2026-09-13 by milestone 281 (`watch` holds exactly what `ps` holds) | notes/process-view.md |
| `sysctl` | declined, §115 (no `sysctl`) | this block |
| `kill`, `pkill`, `skill`, `snice` | refused, milestone 455 (the signaling stratum of `procps`) | `design/roadmap/0455-the-signalling-stratum.md` |
| `pwdx` | declined 2026-09-26, §224 (no `pwdx`): only the shell has a working directory | `design/decisions/0224-no-pwdx.md` |
| `w` | moved out: waits on a tid's name and a second session | `design/roadmap/0681-w-shows-who-is-logged-in.md` |
| `free`, `vmstat` | built 2026-09-26 under §225 (`free` sees the machine and your share) | `crates/free`, `crates/vmstat`, the-machine-and-your-share.md |
| `slabtop` | built 2026-09-26: no slab since milestone 14 (kernel objects from untyped), so it breaks down a job budget by object kind | `crates/slabtop` |
| `tload` | built 2026-09-26 as a line in `top`'s summary, not a program | `crates/top` |
| `pidwait` | ruled 2026-09-26 by §226 (`pidwait` takes tids); moved out, because its wait primitive is a new kernel method | `design/roadmap/0667-pidwait-waits-on-a-named-tid.md` |

## The wire it added, and the table it grew

calef ratified the numbers and names this milestone minted on 2026-09-27 (UTC). Three moved at
rebase because `main` had taken them: the machine page's progenitor boot slot from 17 to 23, the
session's slot from 21 to 20, and `spawnproto::MACHINE_BIT` from bit 42 to bit 44. The boot slot
needed the capability table raised from 24 slots to 32, which calef ruled the same day. What
moved and why, and the Kani timings the raise touched, are in
`notes/process-view/the-machine-and-your-share.md`.

The raise put the stack-frame and fastpath-footprint gates red; calef chose a free-slot bitmap for
`CapabilityTable::insert` on #1360, and that note has the before and after numbers.

A bit-scan's trip count is data-dependent, so the two
table harnesses never terminated under bounded unwinding: a CI shard spent its 45-minute cap on
them twice, and a local run passed iteration 1669 after four hours. Both now carry
`#[kani::unwind(4)]`; the soundness argument sits beside each attribute in
`crates/capability/src/lib.rs`, and the crate's 14 harnesses prove in 17 s.

## Why this package, and why the package rather than the program

What these programs want is enumeration of the process namespace, and enumeration is the authority
this system is built to refuse. Milestone 121 (`ripgrep` on nife) makes the same argument for
directories: a program that can list learns what exists, which is a larger power than reading
something it was handed.

On Linux the answer comes from `/proc`, which is ambient. Any process reads it with no grant from
anyone, so `ps aux` prints every command line on the machine, including the ones with secrets in
`argv`. Nobody defends that design; they live with it, and `hidepid` exists because enough people
stopped wanting to.

`procps` (upstream `procps-ng`) is Priority: important, so it is on essentially every Ubuntu
install. As the unit the distribution ships, it tests whether the approach generalizes; a port that
picked the two programs with the tidiest capability story would prove nothing about typical software.

Once `/proc` is replaced by a held capability, the package stops being one thing. It stratifies by
what each program actually needs: reading the process namespace, signaling a process, machine-wide
statistics, writing kernel tunables, and nothing at all. That was the build order, `ps` first
because a snapshot needs no clock and no accounting.

## The design: a view over a supervision domain

The scope is the supervision domain, because the kernel already maintains it: the threads one
endpoint directly supervises, one level deep (§223 (the process view is the supervision domain)). A
shell holds a domain; the programs it spawns are in that domain; a `ps` launched from that shell
sees exactly those and nothing else. It is the same move `rm -r` makes with a directory it was
handed: authority is held, not global. A scope the system already keeps cannot drift out of
agreement with reality.

The view is `abi::rendezvous::SURVEY`, a method on the supervision endpoint and no new syscall
number. Membership is `capability::survey_includes`, the same relationship that authorizes a reap
under §32 (a supervisor may collect a corpse without being able to build one). So the domain a
monitor sees and the domain a supervisor may collect from cannot diverge; three Kani harnesses hold
that. It is gated by `Rights::ENUMERATE`, which was added for it: `READ` on a supervision endpoint
is also what `RECV` and `REAP` take, so a view on `READ` could have collected the children it
listed. §204 (how userspace asks where a thread runs) later made `SURVEY` answer one selected record
per call, and milestone 282's `CPU_TIME` was the first record added that way.

A wide grant is fine and must be nameable. An operator's `top` genuinely wants the whole machine;
the point is to make that visible, so `caps top` prints the difference between a `top` over one
shell's children and one over everything. On Linux there is no such distinction to print.

A viewer run against a domain it was not granted is refused loudly rather than shown an empty list.
Refused, empty and populated are three different answers where `/proc` has one, and `pgrep` adds a
fourth (matched nothing in a domain that has members). A monitor that silently reports nothing
because it could not look is the worst failure available to this tool.

## What each built program settled

`ps` and `pgrep` share one manifest, field for field, and the sameness is the claim. On Unix `pgrep`
and `pkill` are one lookup with two endings. Here a survey returns a tid, and a tid is a name and
not a capability: there is no path from it to authority over the thread. calef ruled on 2026-08-17
that a domain names its members and does not act on them, which abolished most of the signaling
stratum before it was built. The kernel test filters a domain down to its corpse and then shows that
the capability which named the tid is refused the reap
(`kernel::user::survey_tests::a_filter_names_members_and_tells_its_four_answers_apart`).

`pgrep` also found a property of the boundary rather than of the program. Nothing here delivers
bytes from a command line to a program: `Endowment::arg` is one `u64`, and every string-shaped
designation a person types arrives as a capability. So a pattern cannot be typed at this prompt, and
the shipped `pgrep` names every member. That lifts when `ArgSpec` grows milestone 47's positional
arity.

`pmap` extended `ENUMERATE` to the address-space object (§114) as `abi::address_space::LIST`, a
method and no new syscall number. It reads the space's revocation log rather than walking page
tables. A capability holding `ENUMERATE` alone lists every mapping and is refused `MAP_INTO`, proved
both ways by `kernel::user::pmap_tests`. The delegation audit §114 required found nothing that hands
an address-space capability to a second program. Building it also found that `CONFIGURE` removes a
space from the registry `LIST` reads, so no live space anywhere can be viewed by anyone; see BUGS.

`uptime` needed no capability, because `monotonic_ns` is granted to every process by each
architecture's timer `init`. That grant is a documented, deliberate exception to §10 (process model:
capability-based, microkernel): a monotonic counter lets a program observe time and affect nothing;
the argument is in `crates/uptime`.

`top` waited on per-thread CPU accounting that did not exist, dead or live. This block's 2026-08-26
fork laid out three meanings of "CPU time"; calef chose scheduled on-CPU time, tick-sampled, as §150
(how does a thread's CPU time reach userspace?) on 2026-09-13, and milestone 282 built it.
`crates/top` states the open question of whether `top` should be a program at all, since it holds
exactly `ps`'s three slots.

`watch` redrew `ps`'s listing in place. Milestone 281 deleted it on the rule that two programs are
two programs when they hold different authority, and it held exactly what `ps` holds. Its
half-second interval was a yield-spin, because the kernel has no timed wait (milestone 106 (a wait
that ends on either the interrupt or the deadline)).

## `sysctl` is declined (DECISIONS §115)

It writes machine-global kernel tunables, and it ships in the same package as `ps`, which says
something about what Unix packaging bundles: `apt install procps` gets you process listing and the
ability to retune the kernel. calef decided on 2026-08-23 that there is no `sysctl` here. Authority
stays with whoever already holds a resource and is never centralized into a generic tool, which is
`pkill`'s ruling one layer over. Each subsystem that grows a runtime tunable carries its own control
surface, the shape Plan 9's per-resource `ctl` files take and
`notes/net/prior-art-and-the-contract.md` already built.

## Corrections, 2026-09-26

The machine overruled this block in four places, found by re-reading the tree rather than the block.

- The `top` fork was still written up as open, and `top` as unbuilt. §150 decided it on 2026-09-13
  and milestone 282 built it on 2026-09-21.
- `watch` was still listed as built. Milestone 281 deleted it and `crates/watch` on 2026-09-13.
- `pwdx` was filed as "print a name for a tid", blocked on a display name. Upstream `pwdx` prints
  another process's current working directory. Here only the shell has one, as a value in its own
  `grant_plan::Holdings`. It was waiting on the wrong fork for a month.
- The block's own display-name fork is withdrawn in favor of §148 (a supervisor restarts by
  asking, and resolves by asking the kernel). `w` and `ps`'s missing `CMD` column are consumers it
  does not list.

The same sweep found two members of the statistics row the block never examined. `slabtop` has
nothing to report, because milestone 14 removed the kernel heap and its slab. `tload` needs a load
figure no one computes. Both are placed in what-is-left.md, section 3.

## Prior art

A design to copy: Fuchsia's job handles. A Fuchsia process lives in a job, jobs nest, and listing
processes requires a handle to the job whose children you want; their `ps` needs a handle to the
root job to see everything. That is this milestone's shape, already shipped. It is worth reading for
how they handle a process that dies mid-enumeration, which `ps`'s two-walk join answers only by
printing `-` for the figure it missed.

A mistake to avoid: `/proc` as an ambient filesystem. Plan 9 made `/proc` cleaner than Linux did and
still put process state in a namespace a program reaches by naming rather than by holding.

## BUGS

- `pmap` cannot be run from the prompt against any real process. Every address-space capability is
  minted and consumed within the thread that builds it, and `CONFIGURE` removes the space from the
  registry the moment it binds to a thread (`take_user_address_space`). So there is no address
  space, held by anyone other than its builder, that a second program could be handed a live view
  of. The fix is a spawn-protocol and object-lifecycle question. The declared retention of §142
  (what a spawner retains over a child after `START`) is now where it would be expressed.
  What-is-left.md, section 5.
- `pmap` shows one row per mapped page with no range coalescing, because the revocation log records
  one entry per page and nothing about adjacency. It also cannot tell a device mapping from ordinary
  read/write memory, since `paging::Flags` carries no device bit the syscall handler can see.
- `procps` ships without `sysctl` (§115) and without its signaling stratum (milestone 455). A
  reader who expects `kill` or `sysctl` to be a program will not find one.
- Aggregate statistics are a side channel, and capabilities do not close it. CPU time per thread,
  which `top` now shows, leaks information about work the viewer was never shown. A capability
  bounds who may ask; it says nothing about what the numbers reveal to whoever may.
- A supervision-derived view cannot express a set that is not a union of domains. A monitor over
  unrelated services holds `ENUMERATE` on each service's endpoint and sees whole domains at a time,
  never one member picked out of a domain it was not handed. §223 (the process view is the
  supervision domain) decided this.
- The comparison against Linux is not apples to apples. Ours lists a domain; theirs lists a machine.
  That is the point, and a side-by-side table that did not say so would break the map "tie"
  caveat's rule.

## Follow-on

- **Decision.** `pwdx` is not built and will not be: `design/decisions/0224-no-pwdx.md` (calef,
  2026-09-26). Upstream prints another process's working directory, and here only the shell holds
  one (`grant_plan::nav::Cwd`), which it already prints with `pwd`.
- **Milestone 681.** Milestone 681 (`w`: who is logged in, and what they are running). `w` waits on §148, because a
  tid has no name, and on a second session existing:
  `design/roadmap/0681-w-shows-who-is-logged-in.md`.
- **Decision.** How `free` and `vmstat` learn about memory is ruled in
  `design/decisions/0225-free-sees-the-machine-and-your-share.md` (calef, 2026-09-26): a
  `MemoryRegion` method under `ENUMERATE` for the caller's share, and a machine memory page granted
  to every login by default and withholdable by the owner.
- **Done.** `free`, `vmstat`, `slabtop` and `top`'s machine line, on `milestone/126-free`
  (2026-09-26): `MemoryRegion::USAGE` and the machine statistics page. The proposed method number,
  page layout and names are in `notes/process-view/the-machine-and-your-share.md`.
- **Recorded.** The owner's switch for the machine page is one boot-time constant,
  `system_initializer::GRANT_MACHINE_PAGE`, not a per-login policy; marked as an exception in
  `crates/system_initializer/src/lib.rs` where it is defined.
- **Recorded.** `free` prints no `Swap:` line and `vmstat` no swap columns, because nife refuses
  paging out for now (pull request #1356); each says so in its `BUGS` in `crates/free/src/lib.rs`
  and `crates/vmstat/src/lib.rs`.
- **Decision.** `pidwait` takes tids, not a pattern, and composes as `pidwait $(pgrep foo)`:
  `design/decisions/0226-pidwait-takes-tids.md` (calef, 2026-09-26). `pgrep --wait`, one binary with
  two names, and a pattern-taking `pidwait` are refused there.
- **Milestone 667.** Milestone 667 (`pidwait`: a way to wait on a named tid with less authority than `pgrep`'s). `pidwait` needs a new kernel method to wait on a named tid with less authority
  than `pgrep`'s, which is the syscall surface and calef's: the options are in
  `design/roadmap/0667-pidwait-waits-on-a-named-tid.md`.
- **Recorded.** `pmap` is unreachable from the prompt, because `take_user_address_space` in
  `kernel/src/user.rs` deregisters a space at `CONFIGURE`. calef moved it out of this milestone on
  2026-09-26 into a milestone of its own, proposed in pull request #1365.
- **Decision.** The process view is the supervision domain, one level of direct supervision:
  `design/decisions/0223-the-process-view-is-the-supervision-domain.md` (calef, 2026-09-26, "A, and
  refuse B"). A separate process namespace is refused.
- **Milestone 47.** A pattern still cannot be typed at `pgrep`, because its manifest in
  `crates/grant_plan` is `ArgSpec::Forbidden` and positional arity is 47's.
- **Decision.** `sysctl` is not built and will not be: `design/decisions/0115-no-sysctl.md`.
- **Refused.** The signalling stratum (`kill`, `pkill`, `skill`, `snice`) stays unbuilt: a survey
  returns a tid, a tid is not a capability, and killing stays with whoever holds the child's region.
  Milestone 455 carries the refusal and the condition that would change it.
- **Recorded.** `pmap` prints one row per page with no coalescing and cannot tell a device mapping
  from ordinary memory, both stated in `crates/pmap`'s module docs.
- **Recorded.** `uptime` prints no load average and no logged-in-user count; the reason is in
  `crates/uptime`'s module docs.

## Index row

The sharpest ambient-authority case in the utility set, because what these programs want is
enumeration of the process namespace, and `/proc` hands it to anyone. Taken as a whole package for
consistency with 123's corpus approach. Replacing `/proc` with a held capability stratifies it.
`ps`, `pgrep`, `pmap`, `uptime` and `top` are built over `rendezvous::SURVEY` and `ENUMERATE`.
`sysctl`, `pwdx` and the signaling programs are declined, and `watch` was built and cut. `free`,
`vmstat` and `slabtop` read a region method and a machine page (§225). `pidwait`, `w` and `pmap`
from the prompt moved out as proposals of their own.
