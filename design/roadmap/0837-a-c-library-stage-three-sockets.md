---
status: NOT-STARTED
raised: 2026-10-08
milestone_dependencies: 835
decision_dependencies: 25, 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 837. A C library, stage 3: sockets

*(The title and slug are provisional.)*

Stage 3 of §265 (a C library started from relibc): `socket`, `connect`, `bind`, `listen`, `accept`,
`send`, `recv`, `setsockopt` for the options the consumers set, `getaddrinfo` for literal addresses,
and `poll` or `select` over sockets and files. The platform layer speaks `socket_protocol` on slots
2 and 3, the contract `std::net` already uses under §25 (socket identity). A refused connect and a
port outside the listen grant map to `ECONNREFUSED` and `EACCES`, as `std` maps them to error kinds.

Reuse: relibc's socket, `netdb` and `poll` header code (MIT), over `crates/socket_protocol`, the
contract `std::net` and net_stack already share.

## First consumers

iperf3 (828) and netperf's TCP_RR (829), on PR #1854. iperf3 also needs stage 2;
netperf's server forks by default and runs here in its no-fork mode, which 829 already records.

## Done when

netperf's TCP_RR client and server build unmodified and complete a run between two nife processes,
and against a Linux peer, on all three architectures.

## Index row

Sockets for nife's C library, over the socket contract `std::net` already speaks, so the network
benchmarks run unmodified.
