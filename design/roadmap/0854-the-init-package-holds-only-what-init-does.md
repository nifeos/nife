---
status: NOT-STARTED
raised: 2026-10-06
promoted_from: the-init-package-holds-only-what-init-does
milestone_dependencies: 611
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 854. The init package holds only what init does

*(The title and slug are provisional.)*

Package names follow calef's 2026-10-06 ruling: spelled out and hyphenated, as in
`core-tools`, `disk-tools` and `process-tools`.

## The rulings

calef ruled all five forks on 2026-10-07 (UTC), between 01:32 and 02:13, in comments on pull request
#1796. Those comments are the record; his words are quoted from them.

| fork | ruling | his words |
|---|---|---|
| 1 | `system_log`, program and crate, moves to a new `system-log` package | "Yes, seems like a lot of packages would be dependent upon the service it provides, if not its actual implementation." |
| 2 | `swapper` moves from `init` to `fixtures` | "Yes, move to fixtures." |
| 3 | `broker` stays in `init`, and the program is renamed `queue_broker` | "Put it in init. Rename it to queue-broker", then "Yes" to the underscore, since programs take underscores and only packages take hyphens |
| 4 | `system_initializer` stays in `init`; the three status words move to `graphics_contract` and the installer's fetch moves to `jig` | "Yes" |
| 5 | `root_supervisor`, `spawner` and `sub_server_supervisor` stay in `init` | "Yes" |

Fork 3's rename re-rules the 2026-07-30 ratification of `broker` under §39 (a component is named for
what it is). The program's provenance line records both rulings. On Fork 5, the gap that the three
run only under `system_tests` is the thing to close, not a reason to move them. The ordered list of
follow-on lanes is at the end.

## The problem

Milestone 611 (every program and crate belongs to a package) put eight programs and three crates in
`init`. Its own file carries two dated exceptions saying one of its crates "is integration, not
init", and [notes/package-boundaries.md](../../notes/package-boundaries.md) lists the same pair as
a limitation. The maintainer's starting proposal was to keep the supervision spine and move the rest
to three new packages. This lane tested that against the code. Three of its premises did not hold.

## What the measurement overturned

1. `broker` is not a memory broker. It is the queue broker of milestone 23 (a capability-routed
   component OS with live replacement), the latency ladder's middle rung (§41 (the endpoint is the
   broker)): a process that buffers requests while a backend is being replaced. The memory broker of
   milestone 851 (a running program acquires more memory as it needs it) is a separate
   program, `memory_broker`, provisional, and not built.
2. `swapper` has nothing to do with memory or swap space. It is milestone 23's hot-swap operator,
   which replaces a running component under a talking client. Linux calls its PID 0 `swapper`
   (recalled), so the name invites the misreading, and the starting proposal made it. The name was
   ratified on 2026-07-30 under §39 (a component is named for what it is); this lane does not reopen it.
3. `system_initializer` is not integration living in init. It is init. `progenitor` is 148 lines
   whose body is one call to `system_initializer::boot`. The crate is 6,260 lines: about 1,700 build
   the boot system, about 2,650 are the spawn service and the job pool the shell runs commands
   through, and about 510 are the package installer behind the `package install` builtin.

A fourth finding reshapes the "keep" list. On today's boot, nothing starts `root_supervisor`,
`spawner` or `sub_server_supervisor`. Only `system_tests/src/user/authority_tests.rs` spawns
`root_supervisor`, which builds the other two. The same holds for `swapper` and `broker`: only
`live_swap_tests.rs` spawns `swapper`, and only `swapper` builds `broker`. The programs that do
init's work on a booted machine are `progenitor`, `system_initializer` and `job_undertaker`.

## What each member links

Measured from `components/Cargo.toml`, each crate's manifest, and the crates each program's source
names. `*` marks a crate `contracts` or `runtime` already exports, so linking it crosses no boundary.

| member | links | reached over IPC or by archive name from |
|---|---|---|
| `progenitor` | `system_initializer`, `user_mode_runtime*` | the kernel, as the boot process |
| `root_supervisor` | `abi*`, `address_space_map*`, `elf*`, `nifefs*`, `supervision_protocol*`, `user_mode_runtime*` | `authority_tests` only |
| `spawner`, `sub_server_supervisor` | a subset of the row above | `root_supervisor` only |
| `job_undertaker` | `abi*`, `grant_plan*`, `user_mode_runtime*` | `system_initializer`, at boot |
| `swapper` | `abi*`, `component_plan*`, `elf*`, `nifefs*`, `supervision_protocol*`, `hot_swap_contract*`, `user_mode_runtime*` | `live_swap_tests` only; it builds `broker` and the `swap_boundary_witness`, `rust_swappable` and `c_swappable` fixtures by name |
| `broker` | `component_plan*`, `hot_swap_contract*`, `user_mode_runtime*` | `swapper` only |
| `system_log` (program) | `system_log`, `system_log_contract*`, `abi*`, `address_space_map*`, `user_mode_runtime*` | `system_initializer` builds it by archive name; `console` and the kernel speak `system_log_contract` to it |
| `system_log` (crate) | `byte_sink_contract*`, `system_log_contract*` | linked by the `system_log` program and a fuzz target, nothing else |
| `system_initializer` | 23 crates, all contracts or runtime except `http_response` (`network`) and `video_terminal` (`display`) | linked by `progenitor` only |

Two more facts bear on the forks. `init`'s `depends = ["timetable"]` exists only because the
`components` crate as a whole links `timetable`. No `init` program names a `timetable` crate, so
that line goes when `components` dissolves. And pull request #1783 (`reboot` at the prompt) adds a
ninth program, `reboot`, to `init`.

## Fork 1: `system_log` leaves for a `system-log` package

What moves: the `system_log` program and the `system_log` crate.

Edges: none added. No `init` member links the `system_log` crate, and `system_initializer` reaches
the service only by archive name and through `system_log_contract`, which is in `contracts`. The new
package's `depends` is empty. The exceptions it removes: none, since it carried none.

Alternatives:

- Leave it in `init`. This matches systemd, whose journald ships in the same Debian binary package
  as PID 1 (recalled). It fits less well here. nife's log service is a separate address space with
  its own protocol crate. The service manager that milestone 853 (orderly shutdown closes the sync
  window) foresees would stop it as one stateful server among several, not as part of itself.
- Put it in a broader `logging` package that a later durable log writer would join. Nothing else
  exists to join it yet, and a package is the unit that releases together.

Prior art, recalled: Debian ships `rsyslog` apart from any init. macOS ships `launchd` and its log
daemon as separate programs in one OS release. Genode's LOG service is provided by core, not by `init`.

Ruled yes, 2026-10-07. Recommendation was: move it, to a `base` package named `system-log`. Cheap and reversible: one package
file and two lines of `init`'s.

## Fork 2: `swapper` leaves for `fixtures`

What moves: the `swapper` program, to the existing `test` package `fixtures`.

Edges: none added. `swapper` links only contracts and the runtime. `fixtures` already depends on
`init`, and nothing in a `base` package would come to depend on `fixtures`. The exceptions it
removes: none.

Why there and not a new package. Every one of `swapper`'s five roles is a test scenario: a direct
swap, a queued swap, a hung incumbent, a state handoff, and an unwarned dependent. Each builds a
fixed set of `fixtures` programs by archive name, and only `live_swap_tests` runs it.
`components/Cargo.toml` draws the line between `components` and `fixtures` as what a program is
rather than who runs it. By that test `swapper` is the harness of the hot-swap demonstration. The
swap the system will actually perform is `terminal_supervisor`'s, which has `swapper`'s shape, lives
in `terminal`, and links no part of `swapper`.

Alternatives:

- Leave it in `init`, on the grounds that replacing a running service is a service manager's job.
  That holds for the capability, not for this program, which can only swap the components it was
  written to test.
- A new `live-swap` package holding `swapper` and `broker`, on Erlang/OTP's model, where
  `release_handler` lives in SASL rather than in the application that holds the supervisor
  (recalled). It would be a `base` package nothing on a booted machine uses. If a general swap
  operator is built for milestone 198 (a package manager)'s installer, that is the time to draw it.

Ruled yes, 2026-10-07. Recommendation was: move it to `fixtures`. Reversible. The cost is that `fixtures` grows by a program
that milestone 23 calls its flagship, which is a matter for the records, not the code.

## Fork 3: `broker` stays in `init`

What moves: nothing, under the recommendation.

The case for staying. The broker is a lifecycle tool. It holds a channel open while the service
behind it is replaced or restarted, so a producer never blocks on an absent consumer. That is the
same job systemd's socket activation does from PID 1, which keeps the listening socket while a
service restarts (recalled). It links only contracts and the runtime, it has a benchmark
(`broker_rtt`) and a ruled decision behind it, and it is the piece a service manager would place on
a channel.

Edges if it moves anywhere: none added, since it links nothing internal. Exceptions removed: none.

Alternatives:

- Follow `swapper` to `fixtures`. Defensible on today's evidence, since only `swapper` builds it.
  But the broker is a general mechanism, not a scenario, and a fixture is the wrong record of that.
- A new `memory` package, as first proposed. Refused on the premise: `broker` does not manage memory.
  The future `memory_broker` is the natural first member of a `memory` package. The gate refuses a
  package with no members, so that package waits for the program.

Ruled yes, 2026-10-07, with a rename to `queue_broker`. Recommendation was: keep it in `init`.

## Fork 4: `system_initializer` stays, and its two exceptions are retired where they arise

What moves: nothing between packages. Two targeted changes retire the exceptions instead.

The proposed move, and why it fails. An image or integration package does not exist; `host-tools`
builds images from `boot` and `display` on the host, and no `packages/` file covers the target side.
A new `base` package holding `system_initializer` would leave `progenitor` in `init` linking it, so
`init` would depend on the new package and own only a 148-line slot table. And the display
exception would come back under rule 4, since a `base` package may not depend on the `optional`
`display`. The move relocates both exceptions and removes neither.

Where they arise:

- `video_terminal`: three status words (`MODE_DISPLAY`, `TERM_UP`, `KEYBOARD_UP`) that
  `system_initializer` reads while building a graphical session. Status words two programs agree on
  are a contract under rule 7. They belong in `graphics_contract`, which `system_initializer`
  already links and whose manifest comment already calls them "the two graphical programs' status
  words". Moving them retires this exception. It is the same shape as milestone 689 (contracts
  leave implementation crates).
- `http_response`: used only by `fetch`, the package installer's download. calef ruled on
  2026-10-06 that the package client becomes a program, `jig`
  ([milestone 809 (the package client becomes a program)](0809-the-package-client-becomes-a-program.md)). Once `jig`
  fetches, the progenitor no longer does, and this exception goes with the code.

Alternatives:

- Move `system_initializer` and `progenitor` together into a new package. Then that package is init
  under another name, and `init` keeps only test-only programs.
- Split the crate: boot composition into one package, the spawn service into `init`. The spawn
  service and the boot share the job pool, the endowment table and the measured-image check, and
  splitting them is a design change to nife's first process, not a packaging decision.
- Declare `http_response` an interface of `network`. Cheaper than waiting for `jig`, and honest
  about a pure parser. It does nothing for the display exception.

Prior art, recalled: Genode's `init` reads a configuration and builds the whole component tree, so
composition is init's job there. Fuchsia's `component_manager` both composes and starts the system;
its memory monitor is a separate component. Linux's `kswapd` is a kernel thread, which puts memory
reclaim beside the kernel rather than beside init.

Ruled yes, 2026-10-07. Recommendation was: keep it in `init`. Move the three status words into `graphics_contract` as a small
lane of its own, and let `jig` retire `http_response`. Correct the "integration, not init" wording in
both exception reasons and in the package-boundaries limitation, since it misdescribes the crate.

## Fork 5: the B.2 supervision tree stays in `init`

`root_supervisor`, `spawner` and `sub_server_supervisor` are test-only today. By the "what a program
is" test they are init's design: a first process that gives its authority away, a builder that can
build one thing, and a restart policy that holds no memory. Milestone 853 (orderly shutdown closes
the sync window) names them
as nife's supervision tree. Moving them to `fixtures` would be accurate about who runs them and wrong
about what they are.

Ruled yes, 2026-10-07, with the test-only gap recorded as one to close. Recommendation was: keep them. Record in `init`'s header comment that the booted system does not yet
run them, so a reader of the package file does not assume it does.

## What blocks `components` from dissolving

None of the forks above waits on it. Each changes which package file lists a program, and the
gate already checks `components` per program.

The dissolution itself is a precondition of milestone 691 (packages move out of this repository,
one per pull request): a crate cannot live in two repositories. Measured on 2026-10-06, `components`
holds 59 binaries and 61 path dependencies, and 20 lines in `xtask`, `script/` and `helpers/` name it
or its manifest. Those include `xtask`'s initrd builder, `script/names`, `script/audits` and lint's
program list. What blocks it:

- The homes. 611's question 3 (one repository per package, or one per division) is unanswered, and a
  split shaped by the answer should not be done twice.
- The gates cannot yet build a package outside this tree; 691 names #1389's base image list as the
  missing piece.
- Collision cost. Every lane adding a program edits `components/Cargo.toml`, so a split is a quiet-queue job.

Splitting in place, one crate per package inside this repository, needs none of the three. It is a
possible first step for 691, not a fork of this block.

## Pending members

- `reboot` is placed in `init`, which milestone 805 (`reboot` at the prompt) landed. That matches
  Debian, where `reboot` ships with the init
  system (recalled). This block agrees.
- `memory_broker`, when built, goes to a new `memory` package; see Fork 3.

## Names

`system-log` was the package name in the question calef answered yes to (Fork 1). `queue_broker` is
his, by ruling (Fork 3). `memory` and `live-swap` stay provisional and unused. The slug of this file
is a draft, like every roadmap title.

Reuse: the existing `fixtures` package for Fork 2 and the existing `graphics_contract` contract for
Fork 4; no new crate is proposed. The package format, gate and table are milestone 611's, unchanged.

## The worklist

One lane per item, in this order. Each package move is one pull request that edits
`packages/` and regenerates the table in `notes/package-boundaries.md`, with no code change.
#1793 (the `disk-tools` rename), which edits those two files, merged before the rulings, so no
item waits on it.

1. Fork 1: `system_log` to `system-log`. A new `base` package file holding the program and the
   crate, two lines out of `init`, and the regenerated table. Waits on nothing.
2. Fork 2: `swapper` to `fixtures`. One line out of `init`, one into `fixtures`. Waits on nothing.
   `fixtures` already depends on `init`, and `swapper` links only contracts and the runtime.
3. Fork 3: rename `broker` to `queue_broker`. A code change, performed as design/naming.md says a
   ratified rename is performed. Its scope, measured 2026-10-06: the `[[bin]]` and source file in
   `components`, the `init` package line, and the archive name `swapper` builds it by. Then the
   `contract` string in `hot_swap_contract` and `component_plan`, and the kernel's `broker_rtt`
   comments. The provenance line keeps the 2026-07-30 ruling and adds this one. Best after item 2, so
   the file it edits in `swapper` has settled in its new package. The `broker_rtt` benchmark name is
   keyed in the three `bench/baseline-*.txt` files and the recorded radon logs; the program rename
   does not require it, so the lane leaves it unless calef says otherwise.
4. Fork 4a: the three status words (`MODE_DISPLAY`, `TERM_UP`, `KEYBOARD_UP`) move from
   `video_terminal::status` to `graphics_contract`, and the `video_terminal` exception leaves
   `init`. A code change; it waits on nothing.
5. Fork 4b: the package installer's fetch moves to `jig`, and the `http_response` exception leaves
   `init`. Waits on `jig` existing, which is
   [milestone 809 (the package client becomes a program)](0809-the-package-client-becomes-a-program.md),
   not yet built. This item is that program's work, not a lane of its own.
6. Fork 5: say in `init`'s package header that the supervision tree runs only under
   `system_tests`, and close the gap. The record is one comment and can ride with item 1 or 2;
   wiring the tree into the real boot is a milestone of its own, to be proposed by whoever takes it.
7. Fix the stale wording. The two exception reasons and the package-boundaries limitation call
   `system_initializer` "integration, not init". Items 4 and 5 delete the exceptions, and the
   limitation's wording goes with the last of them.

`reboot` joined `init` when milestone 805 (reboot at the prompt) landed, and `memory_broker`
starts a `memory` package when it is built.

## Index row

The init package holds only what init does: `system_log` leaves for a `system-log` package, `swapper` for `fixtures`, `broker` stays and is renamed `queue_broker`, and `system_initializer` stays with its two exceptions retired where they arise. Five forks ruled by calef on 2026-10-07; the worklist is one lane per item.
