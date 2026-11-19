# network_stack

The network stack's host-testable logic, kept out of the programs that use it so each claim is a
host test and a machine-checked proof. `no_std`, no dependency, no `unsafe`. It stays in nife and
is not published.

- `ephemeral::Ports` allocates outbound local ports from the private range 49152 to 65535 with a
  cursor that never falls behind the boot clock, so a stack started a moment after another hands
  out none of the ports an earlier one used.
- `polled_wait::next` decides whether a stack may sleep while it waits on a network card that
  raises no interrupt.

## EXAMPLES

```rust
use network_stack::ephemeral::Ports;

let mut ports = Ports::new();
let port = ports.next(0, |_| false).unwrap();
assert!((49152..=65535).contains(&port));
```

## What is proven

Three Kani harnesses cover the cursor: every position maps to a port in range and the clock never
runs backwards; an allocation never moves the cursor back and never returns a held port; and a
stack started four milliseconds later starts past every port used. Each has a falsification patch
under `falsifications/`.

## BUGS

- The ephemeral bound is a rate, not a proof: a stack replaced quickly can hand its successor a port.
- `polled_wait` keeps a core busy while a stream flows, and for a tick after it ends.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
