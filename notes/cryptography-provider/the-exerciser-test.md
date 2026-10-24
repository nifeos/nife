# The exerciser test

An appendix to [notes/cryptography-provider.md](../cryptography-provider.md). The kernel test in
`system_tests/src/user/cryptography_tests.rs` boots `cryptography_exerciser` and reads what it
prints. This page holds why the test is shaped as it is.

*Name: provisional, for the stem `the-exerciser-test`, after the program the test boots. calef names
files.*

## Why a build is not enough

`script/crypto-probes` shows that each crate builds for the nife targets. A build is not a
computation. The x86_64 target sets `+soft-float` and switches off every SSE level. Each crate then
runs its portable fallback rather than the path almost every other machine takes. Nothing about a
green build says that fallback produces the numbers the specifications print. The test runs the
published vectors on each architecture to find out.

All three architectures run it, under DECISIONS §19 (architectural parity is a tenet). Here parity
is not a formality. x86_64 is the only one of the three whose build forced a different
implementation, so its answers are the least predictable from the other two.

## Whose code it is

The program is ours and its dependencies are not this repository's, under DECISIONS §46 (thin
primitives or whole subsystems; we write everything in between).
`helpers/build-cryptography-exerciser.sh` builds it, and `cargo xtask test` runs that build for
every leg it boots, so `script/test` and CI carry it. A boot that skips the build carries no
program, and the test skips.

The program's heap is started with `start_reclaimable`, the same call `ripgrep` makes. A permanent
charge would fail the suite's frame ledger only for whoever happened to build the program.

## What the transcript check pins

The test checks that each expected line appears, rather than matching the whole transcript. One
line carries the provider's cipher-suite and key-exchange counts, which move when the provider's
version moves. Pinning them would turn an upstream release into a failure that says nothing. A
vector cannot drift that way, so each vector line is pinned exactly.

Two certificate chains are in the list because the hosts that serve a release asset use RSA 2048
and RSA 4096. `github.com` is ECDSA P-256 throughout, so a client without `rsa` reaches the site
and not the file. The refusals line guards against a verifier that returns `Ok` unconditionally. A
flipped signature bit and a certificate for another host's name must both be refused.

The test also checks that the program exited without a fault. Every check above is an `assert!`
inside the program, so a failure ends it with a missing line and a fault. Reading both tells a
wrong vector apart from a program that died on the way. The program's panic hook prints and exits
cleanly, so the failing assertion's message reaches the sink.

## What it does not show

`provider ok` asserts what the provider offers, not that it can talk to anyone. No handshake
happens. A client speaking TLS 1.3 to a peer is separate work, and milestone 442's block says so.
