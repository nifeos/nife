---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: test-helpers-give-their-clients-a-region-to-retype-from
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 670. Test helpers give their clients a region to retype their rendezvous from

*Title and slug are drafts.*
*Not a dependency of 608: 608 names this block as the work that finishes it, and what this block
uses from 608 is the pattern 608 already built (retyping a rendezvous from a reclaimed region).*

Raised by milestone 608 (kernel tests give back their rendezvous points), from the
rendezvous ledger at `sched::PEAK_RENDEZVOUS`, built by milestone 601 (the region table prints its
peak, and a refused split's leak is recorded). The title is provisional.

Test and service wiring; no syscall surface, no dependency.

## What was measured

Milestone 608 moved eleven of `kernel::sched`'s own IPC tests off kernel-chunk rendezvous by
retyping each test's endpoint from a one-off `memory_region::create` region reclaimed at the end
of the test. That pattern does not reach most of the ledger's other large holders. Their
`create_rendezvous()` calls sit inside a shared spawn helper, not the test body. That helper
gives its spawned client no region of its own to retype from:

| module | kernel chunks | helper |
|---|---|---|
| file-service tests (`rm_program_tests`, `dir_capability_tests`, `disk_tests`) | 51 | `fs_service::spawn_fs_client` and siblings |
| `sink_tests`, `date_tests`, `time_tests` | 40 | `spawn_writer`/`spawn_wc`/`spawn_date`-style per-file helpers |
| `ntp_tests` | 37 | `ntp_service.rs` |
| `compositor_tests`, `display_tests` | 34 | `display_service.rs` |
| `user::tests` | 45 | mixed; some rendezvous are deliberately leaked for a spinning child's whole lifetime and are not candidates |

`x86_userspace_round` (`kernel/src/user.rs`) and #1344's `fs_subtree_caretaker`-narrowed walk tests
are the worked examples. Each gives its child a `memory_region::create` region up front and retypes
every rendezvous the child needs from it with `create_rendezvous_from`. Each then reclaims the whole
region in one call once the child is reaped.

## The work

For each shared spawn helper above: give it a region (sized for the endpoints it mints), retype
from it, and return the region alongside whatever it already returns so the caller can reclaim it
once the client is reaped. Confirm first, per module, that the endpoint is not deliberately
outliving the test (a boot-lived service, like the FS server and block server behind the file
tests) before touching it; only per-test residue is in scope.

The 36 kernel-chunk rendezvous in `login_tests` need the same check before assuming they are in
scope. Milestone 601's ledger notes the other 30 there are already retyped from session regions, so
the remaining 36 may be the login service's own boot-lived endpoints rather than test residue.

## Done when

The `rendezvous:` line on aarch64 shows the kernel-chunk count down further, ideally past the
400 milestone 608 aimed at and could not reach alone, with each module either fixed or recorded as
boot-lived and out of scope.

## Index row

Most of the rendezvous ledger's large holders create endpoints inside a shared spawn helper that gives its client no region to retype from. Proposed: give those helpers' clients a region so the registry stops filling.
