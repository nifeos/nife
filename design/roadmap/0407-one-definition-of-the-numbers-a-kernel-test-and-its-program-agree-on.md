---
status: NOT-STARTED
raised: 2026-09-14
milestone_dependencies: none
decision_dependencies: 187
machine_requirements: none
specific_machine: none
needs_person: no
---
# 407. One definition of the numbers a kernel test and its program agree on

Filed 2026-09-14 as an unnumbered proposal by milestone 293's lane, which
reduced one instance of this and then measured the rest of it. Numbered 2026-09-19 by milestone
433's drain of the proposal pile. Premise re-read against the tree on 2026-09-19 and still true,
and the tree's counter-example is still in it. `system_tests/src/user/live_swap_tests.rs` still
re-declares `ROLE_DIRECT`, `ROLE_QUEUED` and `ROLE_HUNG` locally although `crates/hot_swap_contract`
publishes all three. An independent re-count on 2026-09-19, over `ROLE_*`, `RPT_*`, `F_*` and
`TERM_MAGIC` declared on both sides of a spawn, found **25 kernel/program file pairs sharing 79
constant names**. That is a similar method rather than the same one, so it is not byte-comparable
with the 19 pairs and 76 names below, and what it establishes is that the shape has not shrunk.
Three crate names in the table have moved and the work has to read the current ones.
`swap_proto` is `crates/hot_swap_contract`, `login_proto` is `crates/login_contract`, and
`components/src/ntp.rs` was split into three programs by milestone 290, so that row's program side
is now `network_time_client` and its siblings. *(Number provisional until the merge queue lands
it.)*

It is §187 (one crate)
*(number
provisional)*, written up 2026-09-19 by milestone 435's slice-c lane because this gate named no
section. The sweep itself is mechanical and a lane could start it today. What it cannot
settle is the crate names, and there is no useful half of this work that does not create one. See
"What a lane cannot decide" below, which prices the two shapes so the question arrives answered.

In brief. AGENTS.md rule 7 says anything two binaries must agree on is a crate, never a copy.
Seventy-six constants across nineteen file pairs are copies, each held in line by a `// must
match` comment: role numbers, report tags, and report flag bits shared between a kernel test's
wiring (`kernel/src/user/*_service.rs`, `*_tests.rs`) and the program it spawns
(`fixtures/src/*.rs`, `components/src/*.rs`). `login_service.rs` and `login_test_client.rs` alone
share fifteen.

## The measurement

Constants named `ROLE_*`, `RPT_*`, `F_*` or `TERM_MAGIC`, declared **twice** under the same name,
once on each side of the spawn:

| Kernel side | Program side | Shared names |
|---|---|---|
| `kernel/src/user/login_service.rs` | `fixtures/src/login_test_client.rs` | 15 |
| `kernel/src/user/disk_tests.rs` | `components/src/disk_partitioner.rs`, `disk_surveyor.rs`, `redoxfs_server/src/bin/mkfs.rs` | 7, 7, 2 |
| `kernel/src/user/compositor_service.rs` | `fixtures/src/window.rs` | 7 |
| `kernel/src/user/credential_service.rs` | `fixtures/src/credentialer_test_client.rs` | 5 |
| `kernel/src/soak.rs` | `fixtures/src/soaker.rs` | 4 |
| `kernel/src/user/identity_provisioner_service.rs` | `components/src/identity_provisioner.rs` | 4 |
| `kernel/src/user/ntp_service.rs`, `pipeline_service.rs`, `disk_service.rs` | `components/src/ntp.rs`, `swish.rs`, `disk_surveyor.rs` | 3 each |
| six more pairs | | 2 each |

Nineteen pairs, seventy-six names, zero shared definitions.

## Why a comment is not a mechanism

It is rung four of AGENTS.md's ladder for a fact that has a rung-one answer sitting next to it. The
failure it permits is silent and specific: a role number changed on one side spawns a *different*
role, and a flag bit changed on one side makes a test assert against a bit nothing sets. Both read
as a bug in the code under test rather than as a mismatch, which is the same misdiagnosis
`credential_contract::fixture`'s own module doc warns about for the SMB account.

**This is not hypothetical, and the counter-example is in the tree.** `crates/hot_swap_contract` already
publishes `ROLE_DIRECT`, `ROLE_QUEUED` and `ROLE_HUNG` with real doc comments, and
`system_tests/src/user/live_swap_tests.rs` **re-declares all three locally anyway** under a four-word
comment. So even where the crate exists, one side was written without it. That is the strongest
argument for a gate rather than a convention.

## Prior art inside this tree

The shape is built and works, twice. `crates/hot_swap_contract` and `crates/job_mix` both publish the role
numbers their two binaries agree on, and `crates/timetable_store` publishes the file names
`session_reviver` and `fs_test_client` both depend on, for exactly this reason and with the reason
written down. Milestone 293 did the same for the credential values three files were copying
(`credential_contract::fixture::PEOPLE`). What is missing is not a design, it is the sweep.

## What a lane cannot decide

The crate names. The numbers a kernel test and its test client agree on are not part of any
shipped wire contract. Putting `login_test_client`'s six behavior numbers into `login_proto` would
widen the protocol a real login client is written against with something no real client needs. So
each pair either gets a small crate of its own or a shared one, and both are an architect's call.
That, and not the work, is what this waits on.

Two shapes worth pricing before asking, because the question should arrive answered:

1. One crate per pair (`login_test_contract`, `compositor_test_contract`, ...), which follows
   `swap_proto`'s precedent, keeps each contract readable in one file, and costs nineteen names.
2. One crate for all of them, which costs one name and makes the crate a grab bag of unrelated
   numbers, which is the shape §46 and the generic-name rule both push against.

## Where it came from

Milestone 293's lane, which was briefed to "grep the values, not just the constant names" after
the sibling lane on `fixtures/src/sink.rs` (292, unmerged as this is written) found one fact
hand-copied in three files. 293 removed the credential half of
its own instance (three copies of three identity/secret pairs, one of which said in a comment that
it had *chosen* to match another), and left the behavior-number half. Fixing one pair inside
a hotspot three other lanes were in would have been a partial fix in the worst possible place.

## Index row

AGENTS.md rule 7 says anything two binaries must agree on is a crate and never a copy.
Seventy-six constants across nineteen file pairs are copies, each held in line by a `// must match`
comment: role numbers, report tags and report flag bits shared between a kernel test's wiring and
the program it spawns. `login_service.rs` and its test client share fifteen on their own. A
comment is rung four of the ladder for a fact with a rung-one answer sitting beside it, and the
failure it permits is silent and specific. A role number changed on one side spawns a different
role, and a flag bit changed on one side makes a test assert against a bit nothing sets. Both read
as a bug in the code under test rather than as a mismatch. It is not hypothetical, and
the counter-example is the strongest argument for a gate rather than a convention:
`crates/hot_swap_contract` already publishes three role numbers with real doc comments and
`system_tests/src/user/live_swap_tests.rs` re-declares all three locally anyway. The shape is built and
works three times over, in `hot_swap_contract`, `job_mix` and `timetable_store`, so what is missing
is not a design but the sweep. What a lane cannot decide is the crate names, and there is no useful
half of this work that does not create one. These numbers are not part of any shipped wire
contract, so putting a test client's behavior numbers into a protocol crate would widen what a real
client is written against with something no real client needs. Two shapes are priced so the question
arrives answered: one crate per pair, following the precedent and costing nineteen names, or one
crate for all of them, costing one name and making it a grab bag of unrelated numbers.
