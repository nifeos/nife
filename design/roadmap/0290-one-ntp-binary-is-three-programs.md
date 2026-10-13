---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
---
# 290. `components/src/ntp.rs` is three programs wearing one name

Built 2026-09-14. calef ruled the split and named all three while working the unratified
worklist. *(Number provisional until the merge queue lands it.)*

One binary dispatched three roles on `arg0` (`ROLE_CLIENT`, `ROLE_SERVER`, `ROLE_PROBE_CLOCK`) and
was packed into the archive as `ntp`. It is three programs now, and two of them are test-only and
live in `fixtures/`:

| was | is | where | why there |
|---|---|---|---|
| `ROLE_CLIENT` | **`network_time_client`** | `components/` | a distribution would ship it because somebody wants its function |
| `ROLE_SERVER` | **`network_time_test_server`** | `fixtures/` | it exists to exercise the client |
| `ROLE_PROBE_CLOCK` | **`unwritable_clock_witness`** | `fixtures/` | it exists to prove one absence |

The directory split is part of the ruling rather than tidying afterwards. Milestone 175 spent 363
files drawing exactly this line, and a test server sitting in `components/` is the defect that line
exists to prevent.

## The argument that justified one binary, and why it did not survive

The file's own header: *"One binary, three roles selected by `arg0`, which is how every other
multi-part program here is packed and keeps the initrd's directory small."*

That is an argument from implementation convenience, which AGENTS.md ranks below everything else, and
the tenet it fails is the one written for this shape: *would we still choose this if both options were
the same amount of work?* No. Three `[[bin]]` entries and three files cost about an hour; the archive
gains two entries and the measured-boot manifest two lines, and nothing else in the tree noticed.

## The argument that looked load-bearing and was false, which is the half worth keeping

The module docs claimed the probe had to be the **same binary** as the client: the test gives "the
same binary" the client's endowment and watches it fault. **It does not hold, and it was believed for
six weeks.**

The fault is caused by the capability set, not by the code. Any process holding that endowment
faults at that address whatever instructions it runs, and no amount of shared machine code would make
a stale capability list fault. What actually keeps the witness honest is that both processes are
endowed by one function, and that function takes the image as a parameter:

```
start_client(...)  -> spawn_with_client_endowment(image, ip, port, stack, propose, entropy)
start_witness(...) -> spawn_with_client_endowment(image, va, 0,    stack, propose, entropy)
```

So a separate witness binary passed through the same function proves exactly as much as a shared one
did. **The invariant that matters is one grant list, not one ELF**, and it is stated at the function,
in the witness's own header, and in the test's doc comment, because it is the thing a future change
could quietly break.

Verified on the machine rather than asserted, in both directions.

- Negative control first, so the positive result cannot be vacuous: the witness's `REPORT` slot
  set to 5 instead of 0, with the client's grant list left at five entries.
  `an_ntp_client_holds_no_writable_clock_page` failed, because slot 5 held nothing and the witness's
  first `send` had nowhere to go.
- Then the client gained a sixth slot: one more `rendezvous_cap(report, Rights::WRITE)` appended
  to `spawn_with_client_endowment`'s array, `n_grants` raised to six. With the witness still reporting
  on slot 5, the test passed. The witness received a capability nobody gave it by name, because
  the only place capabilities are named for either program is the one array.
- Both changes reverted; the suite is green again on both ISAs.

That is the invariant stated as an experiment: a slot the client gains is a slot the witness
gains, and there is nowhere to forget it.

The failure this guards against is a second capability list. That is milestone 117's stranger-run
`swish` `caps` bug: a hand-maintained copy of a fact the manifest already held, which decays silently
instead of breaking a build. There is no second list in this diff and there must not be one.

## The claim under test proves the same thing, and the evidence

`an_ntp_client_holds_no_writable_clock_page` asserts that the user-fault counter rises, that the
faulting address is exactly `CLOCK_VA`, that no second report is waiting, and that the clock page did
not change. Every one of those assertions is unchanged. What changed is one line: the image the
witness is spawned from.

The fault stays a translation fault rather than a permission fault, because the process holds no
mapping of the clock page at all, and the test still deliberately does not assert the kind. Pinning
the kind would fail if the page were ever mapped read-only, which is a *weaker* system passing a
stricter-looking assertion; the comment saying so is kept.

## A defect this milestone did not go looking for, and fixed

Running one of these tests on its own hung it, on `main` as well as here. The split meant running
`script/test --test <one ntp test>` dozens of times, which nobody had done, and every exchange test
failed with *"the test server never saw a request: the client failed before it reached the network"*.

It was reproduced at the base commit before anything was concluded from it, which is what kept it from
being read as this milestone's own breakage: `3c156f82`, `script/test --arch aarch64 --test
a_proposal_outside`, same failure; the same commit's full suite, all six green.

The cause, found by probing the client one stage at a time:

- `ntp_tests::machine_has_no_entropy()` called `entropy_service::ensure` and threw the `Wiring`
  away.
- The first caller of `ensure` is the one handed the service's `ready` endpoint, and the service
  announces itself with a blocking send. Discarding that wiring without draining it parks the
  entropy service inside its own startup, before its request loop.
- Every later `ensure` gets `ready: None` and cannot rescue it, so the client blocks forever in
  `call(ENTROPY, ...)`, two frames from where the failure is reported.

It never showed in a whole-suite run because `entropy_tests` sorts before `ntp_tests` and drains the
report first. The file was relying on another file's ordering, which is the shape of thing that
holds until somebody runs one test.

`machine_has_no_entropy()` is now `entropy().is_none()`, which is the same question asked through the
helper that already drains correctly, and the duplicate `ensure` call is gone.
`script/test --arch aarch64 --test ntp_tests` is 6 passed where it was a hang, and the full suite
is unchanged on all three architectures.

## A live inconsistency the split resolves

The old header claimed *"There is no test-only branch anywhere in the client"* on the same screen as
`ROLE_PROBE_CLOCK`, which is a test-only branch in the client's binary. After the split the sentence
needs no qualification, and `components/src/network_time_client.rs` says so where the old one did not.

## What was built

- `components/src/network_time_client.rs`, the client, with the role dispatch gone: `_start`
  takes the server's address and port and calls `client`.
- `fixtures/src/network_time_test_server.rs`, three slots (report, `READ` on the socket endpoint,
  a budget), holding no propose endpoint and no entropy endpoint, so its endowment shows on sight
  that the peer answering the client cannot itself reach the clock.
- `fixtures/src/unwritable_clock_witness.rs`, thirteen lines of body: report, write, report again
  (which is the failure), exit.
- `kernel/src/user/ntp_service.rs`: `spawn_role` is `spawn_with_client_endowment`
  (**function name provisional**), `start_probe` is `start_witness`, the role
  constants are gone, and the module header states the one-list invariant.
- `kernel/src/user/ntp_tests.rs`: one `ntp_image()` becomes `client_image()`,
  `test_server_image()` and `witness_image()`, so a test that spawns the witness where it meant the
  client fails to compile rather than passing for the wrong reason.
- `xtask/src/main.rs`: one archive row becomes three, in both the portable table and the aarch64
  one. The measured-boot manifest is computed over the archive's entries and sorted, so it picked the
  two new programs up with nothing to edit.
- `fixtures/Cargo.toml` takes `ntp_proto`, `socket_proto` and `clock_proto`, all three already in
  the shipping graph through `components`. Nothing external was added.
- `notes/ntp.md`, including a section on the false "same binary" argument, because that is where a
  reader goes looking.

## Deliberately not built: a crate for what the three share

Rule 7 says anything two binaries must agree on is a crate. **After the split they agree on nothing
that is not already one.** The wire format is `ntp_proto`, the socket contract and the shared frame's
offsets are `socket_proto`, and the frame's virtual address is each process's own choice in its own
address space rather than an agreement. The report words are one numbering space but the three
programs use disjoint subsets of it, and the party each of them agrees with is the kernel, which
mirrors the vocabulary in `ntp_service::rpt` the way it does for thirty other programs.

What is duplicated is about forty lines of `r8`/`w8`/`write_payload`/`read_payload` glue over
`MappedWindow`, and a four-line `stamp`. **That is code over a layout that already lives in a crate,
not a fact two binaries must agree on**, and it is what `entropy`, `mdns_responder`, `net_transport`
and `socket_test_client` each already carry: five copies before this milestone, six after. A crate
for it would be a real improvement and it is a different milestone, because it is about all six
and not about these three; it is proposed below.

## What was measured

`script/test --test _` (the kernel legs; `--test` skips the host pass, and `crates/elf` fails 20 of
25 there on this x86_64 box, which is
`design/roadmap/proposals/host-tests-that-assume-the-host-is-aarch64.md`'s and is pre-existing on
`main`):

| leg | result |
|---|---|
| aarch64 | 321 passed, 3 skipped |
| riscv64 | 328 passed, 2 skipped |
| x86_64 (BIOS, then UEFI under OVMF) | 206 passed, 69 skipped, twice |

Exit 0. The same command at the base commit `3c156f82` gave aarch64 321/3, so the split adds two
archive entries and moves no test.

The six NTP tests run alone now, which they did not before (see the defect above):
`script/test --test ntp_tests` is 6 passed on aarch64, 6 on riscv64, and on x86_64 2 passed with 4
skipped for want of a virtio-rng device, which is the documented state of that board.
`an_ntp_client_holds_no_writable_clock_page` is one of the two that runs there, so the confinement
claim is asserted on all three architectures.

`script/lint`, `script/fmt --check`, `script/names --check`, `script/citations`,
`script/roadmap --check`, `script/decisions --check`, `script/swish-check` and
`script/falsifications` all exit 0. `script/names` counts 75 programs where it counted 73, with the
three new names ratified and carrying eight refusals between them.

## BUGS

- Two entries in `design/decisions/0139-cycle-counter-authority.md` cite `components/src/ntp.rs`, a
  path that no longer exists, one of them with a line number. That one moved on 2026-10-11 to the
  appendix `design/decisions/0139-cycle-counter-authority/trap-and-emulate.md`. A lane may not edit
  `design/decisions/`, so they were named here and in this lane's report for the integrator. The
  maintainer repointed the appendix's citation to `components/src/network_time_client.rs` on
  2026-10-11 (UTC). The other sits in calef's ruling on the main page, and the ruling now names
  `components/src/network_time_client.rs` (calef, 2026-10-11 UTC: "Replace the path. Source
  control gives us a history.").
- `design/roadmap/0106-deadline-wait.md` cited `components/src/ntp.rs:44`, `:188` in its table of
  what a timed wait would fix. Fixed, checked by the maintainer on 2026-10-11 (UTC). The citation
  had already left that block on 2026-09-26, when milestone 106 (a wait that ends on either the
  interrupt or the deadline) was recorded BUILT in commit `27e501401` and the table was replaced.
  So there was nothing there to repoint. The two lines it meant are
  `components/src/network_time_client.rs:61` (the module docs' one-shot section) and `:183`
  (`RETRY_GAP_NANOS`), and the retry gap now sleeps on a timer.
- The report vocabulary is one numbering space across three programs that no longer share a
  binary. The numbers were kept exactly where they were so that a boot log from before this
  milestone still reads, and `ntp_service::rpt` is the one place that holds all eight. The cost is
  that a reader of `unwritable_clock_witness.rs` sees `RPT_PROBING = 6` with 1..5 and 7..8 declared
  elsewhere, and must look at the kernel side to see why. Renumbering per program was refused: it
  would make three private vocabularies that the kernel then has to mirror three times, which is more
  copies rather than fewer.
- **Nothing gates test isolation, and one file relying on another file's ordering is not a shape a
  gate can see.** The entropy defect above is fixed in `ntp_tests.rs`, and the same trap is still
  open anywhere else a test discards an `entropy_service::Wiring`: the first `ensure` caller owns a
  blocking report, and dropping it parks the service. The honest remedy is in `ensure` itself (it
  could drain a report nobody asked for) and that is a change to a file six test modules share, which
  is more than this lane should take on the way past. It is proposed below.
- Nothing gates the mirror. `ntp_service::rpt`, `srv` and `reject` are hand-kept copies of
  constants in three files, marked "Must match" the way thirty other kernel-side service modules are.
  That convention is the tree's, not this milestone's, and it is not made worse here; it is also not
  made better.

## Follow-on

- **Done.** The entropy-service isolation defect above, found here and fixed here, with the
  reproduction recorded at the base commit so a reader can tell it apart from this milestone's own
  work.
- **Milestone 410.** Six programs each carry their own copy of the accessors that turn an absolute
  virtual address back into a `MappedWindow` offset, and this lane's split of the `ntp` binary added
  the sixth; `multicast_dns_responder`'s retirement at milestone 298 has since left five.
- **Milestone 402.** `entropy_service::ensure` hands its first caller a `Wiring` whose `ready`
  endpoint must be drained, because the service announces itself with a blocking send, and nothing
  in the type says so. This lane hit it in `ntp_tests` and fixed that one caller; the general
  remedy, making the obligation unrepresentable rather than documented, is 402.
- **Recorded.** The four limitations above stay in this block's `BUGS`, and the two stale citations
  are the ones most likely to waste somebody's minute: a reader following §139's path finds nothing
  at all rather than something wrong.

## Index row

calef ruled the split and named all three (2026-09-14): `network_time_client` in `components/`, `network_time_test_server` and `unwritable_clock_witness` in `fixtures/`. One binary dispatched three roles on `arg0` and justified it as keeping the initrd's directory small, which is implementation convenience and ranks below everything else. The interesting half is the argument that looked load-bearing and was false: the docs claimed the clock-page probe had to be the *same binary* as the client, and the fault is caused by the capability set rather than the code. What keeps the witness honest is that one function endows both and takes the image as a parameter. `an_ntp_client_holds_no_writable_clock_page` asserts exactly what it asserted before; one line changed, the image. Milestone 265 (`_proto` is a truncation)'s `ntp`-stays-`ntp` exception is overtaken and is left for the maintainer rather than edited from a lane.
