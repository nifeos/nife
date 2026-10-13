---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 800
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 825. A hostile client races the file server's name window

*(The title and slug are provisional.)*

## In brief

Boot the attack milestone 800's fourth outsider pass declined. A client rewrites a name in its own
window between `redoxfs_server`'s check and its use. Can a name that passed `check_component` be
swapped for one that would not? If so, it reaches outside the client's subtree.

## Why

calef ruled the refusal log's order on 2026-10-07 (UTC), option (i): this is the next probe. It is
the likeliest live escape the pass left. It is also the only item that could move risk 7 soon. The
gap is recorded at the head of `redoxfs_server/src/dispatch.rs`: "A name is checked in the client's
own window and then used from it". Claims 19 and 24 of `notes/confinement-claims.md` rest on it.
`name_resolver` copies a name out before judging it. The file server does not.

## The work

- A disk fixture, and a hostile client with two threads. One rewrites its name window while the
  other issues a name-taking request.
- A test that goes red if any request reaches a name outside the client's subtree. Boot it on every
  ISA that runs the file server.
- If it is red, that is the finding. Commit it red and opt-in, as milestone 800 did, and the fix is
  its own milestone.
- If it is green after a real try, say how many races ran and what bounds the window. Keep the test
  only if it proves something no other test does.

## Done when

The race has been booted. Its result is in the pass note's refusal log and in claims 19 and 24: an
escape with a red test, or a held claim with the evidence that it was tried.

Reuse: milestone 800's attack shape (`socket_squatter`, a role of the server's own binary) and the
file-server test fixtures already in `system_tests`.

## Index row

The fourth confinement pass's refused probe, booted. A client races its own name window against
`redoxfs_server`'s check-then-use, on every ISA that runs the file server.
