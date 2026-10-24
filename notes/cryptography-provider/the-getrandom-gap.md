# `getrandom`, which was a major-version gap rather than a missing backend

*Name: provisional.*

The first of the two build failures [`notes/cryptography-provider.md`](../cryptography-provider.md) names, in full.

`rustls-rustcrypto` and `ring` both died at `getrandom`'s `compile_error!`, exactly as §196
recorded, and `entropy_backend` was already the answer to that class. It answered 0.3 and 0.4,
whose hook is a bare `__getrandom_v03_custom` symbol. `rustls-rustcrypto` reaches `getrandom`
0.2 through `rand_core` 0.6, and 0.2's hook is a different symbol with a different signature,
emitted by the `register_custom_getrandom!` macro.

`entropy_backend`'s own `BUGS` section had named this and forecast it: *"nothing currently needs
the second shape. Something will."* It did. The crate now defines both symbols. Two details are
worth carrying:

- 0.2's hook returns a bare `u32` (zero for success), not a `Result`, so defining it needs no
  type from `getrandom`. What it needs is 0.2's `custom` feature, and only a manifest can turn
  a feature on, which is why `entropy_backend` now depends on 0.2 as well. The cost is one extra
  crate in the graph of a consumer that only ever needed 0.4.
- `use entropy_backend as _;` is still load-bearing, for the 0.2 symbol exactly as for the
  other. Upstream's documentation says a registration "can only be registered in the root binary
  crate", which is not quite the rule; the rule is that an rlib nothing references is not linked.

`ring` moved one class along as a result: it no longer fails at `getrandom`, it fails in its own
`cc` build script, which is what notes/crates-io-on-nife.md's class C predicted and what §196
already recorded from a different angle. `aws-lc-rs`, which §196 never probed, fails the same way
in `aws-lc-sys`. Neither is available to this project at any price a lane can pay, and that is
a finding rather than a disappointment: it is §46's "no C in the shipping graph" holding.

## Why a crate, and not a patch

`entropy_backend` changes nothing of `getrandom`'s, so it is not a patch. A vendored fork would avoid
the `rustflags` line every consuming workspace needs, but it would track a fast-moving crate. The
lasting fix is a `nife` arm upstream, shaped like the existing `hermit.rs`.
