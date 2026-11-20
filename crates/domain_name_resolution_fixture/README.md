# domain_name_resolution_fixture

Test data for a name resolver, in one place so the programs that touch it agree: the zone the boot
test grants its client, the address of the QEMU runners' name server peer, the codes the test
client reports its echo with, and the ten names the client asks with the reply each must get.
`no_std`. It depends on `domain_name_resolution_contract`, whose reply statuses the cases carry,
and on `domain_name_system`, whose reject reasons they name. It stays in nife and is not
published.

It is not a wire contract. The peer that answers, `helpers/name-server-peer`, spells its zone again
in Python on purpose, so the test proves two independently written sides agree.

## EXAMPLES

```rust
use domain_name_resolution_contract::status;
use domain_name_resolution_fixture::{CASES, ZONE};

assert_eq!(ZONE, "nife.test");
assert_eq!(CASES[0].status, status::OK);
```

## What is proven

Nothing is proven. A test checks that the table names each case once.

## BUGS

- None known.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
