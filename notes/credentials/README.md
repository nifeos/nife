# Credentials: the appendices to notes/credentials.md

[`notes/credentials.md`](../credentials.md) carries why the service hands out the operation and not
the secret, its two phases, and the Argon2id choice in brief. It also carries what a naive verifier
gets wrong, the reply and the shared page, EXAMPLES, a summary of what is proven, and BUGS. These
files carry the evidence its prose budget has no room for.

- [`argon2id.md`](argon2id.md): why the service depends rather than writes, the known-answer
  vectors, the overflow found in `argon2`, and the cost parameters with their measurements.
- [`what-is-proven.md`](what-is-proven.md): every host test, Kani harness and guest test, and what
  each pins.
- [`contract.md`](contract.md): the design behind `crates/credential_contract`: the two endpoints,
  why an opcode is not an authority, the distinct refusals, the fixtures and the proofs' coverage.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.
