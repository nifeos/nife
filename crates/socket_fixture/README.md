# socket_fixture

Test data for a network stack, in one place so the programs that touch it agree: the port the
inbound gate listens on, the port it probes and must be refused, the two messages, and the
addresses of the QEMU runners' echo peer and package source. `no_std`. Its one dependency is
`socket_contract`, whose grants it is checked against. It stays in nife and is not published.

It is not a wire contract. The host's copy of the inbound gate's facts is spelled again on the host
side on purpose, so the gate proves two independently written sides agree.
