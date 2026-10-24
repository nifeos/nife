# A TLS crypto provider: the appendices to notes/cryptography-provider.md

[`notes/cryptography-provider.md`](../cryptography-provider.md) carries the build table, the two
build failures and the ring-3 hazard, what runs, how to run it, the refusal of `rustls-rustcrypto`,
what the pricing of the glue path found, and BUGS. These files carry the evidence its prose budget
has no room for.

- [`the-getrandom-gap.md`](the-getrandom-gap.md): why `getrandom` 0.2 needed a second symbol in
  `entropy_backend`, and where `ring` and `aws-lc-rs` fail instead.
- [`pricing-the-glue-path.md`](pricing-the-glue-path.md): the graph measured the same way for both
  paths, quality per primitive, the `rsa` argument and the `deny.toml` suppression, and the test
  of §46 (thin primitives or whole subsystems).
- [`the-glue.md`](the-glue.md): why the provider is glue only, TLS 1.3 only, and why the record
  layer and the RSA verifiers are shaped as they are.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `the-getrandom-gap` Name: provisional.
- `pricing-the-glue-path` Name: provisional. After the parent's own heading.
- `the-glue` Name: provisional.
