# Network stack

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  components/src/net_stack.rs | no host fuzz target (about 630 lines to move into a sans-IO crate) | Sans-IO crate for net_stack dispatch, fuzzed | L  |
|  components/src/net_stack.rs | one client can take every socket; per-client quota needs a badged front door per client | net_stack: per-client socket quota | M  |
|  components/src/socket_test_client.rs | survival probe may not tell a closed copy from an open one, so the hand-off test's "reaches nothing" rests on it; "not yet checked by a lane" | Check the socket hand-off probe against an open socket | S  |
|  components/src/virtio_net_transport.rs | DMA layout constants restated in 3 places and nothing checks the copies against the crate (0410 priced out the dependency edge, not a check) | Gate that the virtio ring constants agree across copies | S  |
|  crates/socket_contract/src/lib.rs | A core stays busy while a stream flows; needs a NIC interrupt or a timed wait finer than the tick. | NIC interrupt (or sub-tick timed wait) for the stream loop | M  |
|  crates/socket_contract/src/lib.rs (bullets 1 and 3) | Ephemeral port bound is a rate, never measured across the suite; a rebooted machine reuses the last boot's ports because no per-boot offset reaches net_stack. | A per-boot ephemeral port offset shared by a boot's stacks | S  |
|  design/roadmap/0257-network-boot-radon.md | 428 KiB/s TFTP; nobody checked whether blksize or the TFTP window is the limit | Find what limits radon's TFTP rate | S  |
|  design/roadmap/0384-a-name-resolver-and-who-holds-it.md | resolver UDP path (source check, keep-listening on forged reply) has run on no boot; the gate asks over TCP | Exercise the resolver's UDP path against a test peer | M  |
|  notes/name-resolution.md | resolver's UDP path (source check, keep-listening on forged reply) has run on no boot | Gate the resolver's UDP path on a boot | M  |
|  notes/net.md | accepted stream reports peer `0.0.0.0:0` because `OPERATION_ACCEPT` reply has no peer (contract change, calef's call) | Carry the peer address in the accept reply | S  |
|  notes/net.md | `net_stack` serves one synchronous exchange at a time; listener backlog one deep | Concurrent connections in net_stack | L  |
|  notes/net.md | no artifact says which components may serve inbound ports; nothing in `crates/socket_contract` is machine-checked | Machine-check inbound-port authority | M  |
|  notes/net.md | DMA region is one 4 KiB page so MTU is 576 and full frames do not fit | Multi-page net DMA region for a full MTU | M  |
|  notes/std.md (net bullet) | `accept` gives no peer address; no non-blocking mode or timeouts; one-connection backlog; no IPv6; port reuse after fast churn | `std::net` gaps: peer address, timeouts, non-blocking, backlog | M  |
|  pinned_tls_client/src/lib.rs | no rotation story when the pinned root retires | A rotation path for the pinned TLS root | M  |
|  roadmap 0783 | how much of the net-stack seed an attacker must guess is not measured | Measure the net-stack seed's guessable share | S  |
|  roadmap 0892 | network grant is one flag, no per-destination filter; "unowned" | Per-destination network filter | L  |
|  roadmap 0892 | spawn service waits on the resolver (three 15 s waits) delaying next spawn | Spawn does not wait on the resolver | M  |
