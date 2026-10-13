---
status: BUILT
raised: 2026-09-15
built: 2026-09-15
---
# 298. Retire the multicast DNS responder and its two crates, which advertised a goal that is gone

calef, 2026-09-15: *"Retire all three."*

## How it came up, because it came up as a naming question

`multicast_dns_responder`, `multicast_dns_protocol` and `multicast_dns_config` came up for
ratification together: calef ruled their `multicast_dns` stem on 2026-09-13 and had not been shown
the whole names. Shown the program as *"what makes this machine appear in a Mac's Time Machine
list"*, calef asked whether it did more than Time Machine, since Time Machine had been dropped. It
does not, and the naming question dissolved into a retirement.

## What the three actually were, measured 2026-09-15

- The wire crate hardcoded exactly three services: `_smb._tcp`, `_adisk._tcp` (Time Machine's
  backup-disk flags) and `_device-info._tcp` (the icon a Mac shows).
- The configuration grammar was Time Machine's: `host`, `smb-port`, `model`, `sys-flags`, and one
  `disk` line per Time Machine share. It could not describe any other service.
- The shipped `components/multicast_dns_responder.conf` was calef's router, captured 2026-08-15,
  advertising two family backup disks.
- The SMB share it advertised no longer exists. Milestone 54 (a network file service a Mac can
  mount) was removed from the tree on 2026-08-30, the day milestone 55 (Time Machine) was retired.
- Nothing ran it but tests. Two kernel tests spawned it beside the network stack, and `xtask`'s
  test run counted a multicast check among the conditions for passing. It was not in the real boot.
- No live work wants it. Of the six live blocks mentioning multicast DNS, milestones 129, 131 and
  260 cite its configuration document as the shape to copy, 144 and 146 mention it in passing, and
  milestone 384, `design/roadmap/0384-a-name-resolver-and-who-holds-it.md`, says outright it does
  not need it.

## What to remove

1. **The program**: `components/src/multicast_dns_responder.rs`, its `[[bin]]` in
   `components/Cargo.toml`, and `components/multicast_dns_responder.conf`.
2. The two crates: `crates/multicast_dns_protocol` and `crates/multicast_dns_config`, their
   workspace members, and every dependency on them (`components`, `xtask`).
3. The tests that spawn it, in `kernel/src/user/tests.rs` and
   `kernel/src/user/riscv_virtio_tests.rs`. Those tests also exercise the network stack and a second
   socket client, and that coverage must survive: remove the responder from them, not them.
4. `xtask`'s multicast prober and its term in the test run's pass condition, together with the
   constants and the `include_str!` of the configuration file. The inbound and scanout checks stay.
5. The archive entries for the program on every architecture.
6. The proof and falsification records for `multicast_dns_protocol`: its row in `script/verify`'s
   table, its entries in `script/falsifications`, and any counted claim a gate checks against the
   harness total.

## What to do with the records

- `BUILT` and `REMOVED` blocks are accounts and keep their words, milestone 55 and milestone 265
  included.
- Live blocks that cite the configuration document as a shape (129, 131, 260) keep the lesson and
  lose the path: the shape outlives the file, and the pointer should become the git history or a note.
- `notes/mdns.md` follows the precedent milestone 54's removal set for `notes/smb.md`, whatever that
  was, and says plainly that the responder was retired and why. It must not keep presenting Time
  Machine as current, which it did until this milestone.
- `SECURITY.md` and the untrusted-input notes stop listing a parser that no longer exists.
- `notes/README.md` keeps its index honest.

## The one thing retirement loses, recorded so it is not rediscovered

`multicast_dns_protocol` carried general DNS message and name parsing, Kani-checked at the parser,
beside the Time Machine records. A future unicast resolver, milestone 384 in
`design/roadmap/0384-a-name-resolver-and-who-holds-it.md`, would want that half. calef ruled to
retire all three knowing this; git keeps the code, and the resolver proposal should say where to find
it.

## What was built

- **Removed**: the program, its configuration document, both crates (and their workspace members and
  dependencies in `components` and `xtask`), both archive entries, xtask's multicast prober with its
  DNS decoder and its term in both pass conditions (the TCG legs and `--hvf`), and
  `multicast_dns_protocol`'s row in `script/verify` (three harnesses; the tree now carries 148 across
  25 packages).
- **Also removed, and not on the list above**, because each existed only for the responder and its
  prober and nothing could exercise it afterwards: the runners' frame-level injection hub
  (`NIFE_MCAST_PORT`), `net_stack`'s join of 224.0.0.251, and smoltcp's `multicast` feature. So
  nothing in the tree proves multicast receive or send now, recorded in notes/mdns.md and
  notes/net.md. Reversible: all three are at commit `0652c981`.
- Kept, and still proved: the accept test moved from `start_shared_net_stack` (deleted, since it
  had no second client left) to `start_net_stack` with the same listen and UDP bind grants, so the
  inbound rounds and the UDP bind grant's refusal and exclusivity checks run unchanged. A UDP `RECV`'s
  source endpoint is still proved by the TFTP exchange.
- **Records**: `notes/mdns.md` follows `notes/smb.md`'s precedent (kept in full under a past-tense
  header naming the last commit). Live blocks 129, 131, 146 and 260 and four proposals keep their
  lessons and lose the dangling paths; the resolver proposal names where the general DNS parsing is.
  `BUILT` accounts, including 55 and 265, keep their words.

## BUGS

- This block's reference counts are from one grep on 2026-09-15 (about 26 code and configuration
  files, 26 markdown files), and the lane should enumerate rather than trust them.

## Follow-on

- **Recorded.** Nothing in the tree proves multicast receive or send now, and what a future multicast
  client must put back (smoltcp's feature, the group join, a host-side peer below slirp) is written in
  `notes/mdns.md` and `notes/net.md`.
- **Recorded.** `NET_CLIENT_STACK_PAGES` stays at six, a number measured for the retired responder's
  call frame rather than for the socket client that is left; lowering it wants a stack-depth reading,
  written beside the constant in `kernel/src/user/virtio_service.rs`.
- **Recorded.** The general DNS parsing the retirement loses is pointed at by commit in
  `design/roadmap/0384-a-name-resolver-and-who-holds-it.md`, milestone 384 since 2026-09-19.

## Index row

Retired the multicast DNS responder and its two crates on calef's 2026-09-15 ruling. They advertised
Time Machine and an SMB share, both removed on 2026-08-30, nothing but tests ran them, and no live
work wants them. Came up as a naming question and dissolved into a retirement.
