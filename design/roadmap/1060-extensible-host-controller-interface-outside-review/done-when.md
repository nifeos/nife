# Done when, on a host, in CI and on hardware

An appendix to [the milestone](../1060-extensible-host-controller-interface-outside-review.md).

- P0-1 has closed: `Cargo.toml` describes what the crate holds, carries `xhci` and `usb` among its
  keywords, and has no comment that points outside the repository.
- The weekly `mutation` verdict passes on `main`, with `src/verification.rs` excluded and each
  equivalent mutant exempted with the constant that proves it equivalent (P1-1).
- The README has a section a stranger can follow to build, test, prove and falsify, the Kani
  install line copies as one line, and `docs/stranger-test.md` carries a date with no open finding
  against the README (P1-2).
- `context::endpoint` takes an `EndpointType`, keeps the mask, and its harness and patch prove the
  mask (P1-3).
- `EventRing`, `ProducerRing` and the README state the read order, the write order and the fences
  a driver needs (P1-4).
- `EventRing::new` and `ProducerRing::new` refuse sizes outside the bounds read from the
  specification text, through `MINIMUM_SEGMENT_ENTRIES` and `MAXIMUM_SEGMENT_ENTRIES`, and the
  harnesses and the fuzz range cover only the accepted sizes (P1-5).
- Every code the crate hands back has a public name, `completion::name` exists, and `cargo doc`
  shows them (P1-8).
- `#![forbid(unsafe_code)]` is in force and the fuzz caller publishes crash inputs (P1-9).
- The callers pin a template release that carries the mutation stack, `check / mutation-diff` is a
  required check that reports, and `mutation.yml` has one green full run on `main` (P1-10).
- `Capabilities::port_status_and_control` returns `Option<u64>`, and the fuzz target asserts
  `None` for port 0 and past `maximum_ports` (P1-11).
- `PortDebounce::sample` and `TransferRequestBlock::setup` use the two enums, `ProducerRing` has
  `try_enqueue` and `complete`, and `cargo run --example fake_controller` prints each TRB it
  consumes (P1-12, P1-13).
- P1-6 and P1-7 have closed, and each P1 item closed by a pull request that cites this block by
  number. The release review has ratified or renamed every provisional name of P1-3, P1-8, P1-12
  and P1-13.
- `cargo test`, `cargo kani`, `cargo mutants` (none missed, none unexempted) and the three fuzz
  targets pass at the commit any tag lands on, and every new harness arrives with its patch.
- Each P2 item has closed or become an entry in the README's `BUGS`, with its door and size.
- Nothing goes to crates.io, and nobody places a tag without a separate go of the architect.

## What needs hardware

The review ran none of these. Each needs a machine with an xHCI controller and a person at it, and
closes with a dated bench record or a README `BUGS` line naming it as unrun.

- The legacy support handoff on a controller whose firmware holds the BIOS semaphore: the semaphore
  clears, the system management interrupt enables clear, the events acknowledge, and the controller
  runs afterward.
- The port debounce on silicon: ports reappear after the controller reset within the stable
  interval, the driver believes a plugged device after its last bounce, and the timeout fires on
  nothing.
- A device on a USB 3 root port, speed id 4 or 5, and the pairing of that port with its USB 2 twin
  (P2-6).
- A controller with 64-byte contexts (`HCCPARAMS1.CSZ` set) and a nonzero scratchpad count,
  neither of which QEMU's controller presents.
- An event ring wrap on silicon, with the busy bit cleared on each dequeue pointer write (P2-4).
- A command ring wrap, so real hardware follows the link TRB and the toggled cycle.
- A stall on endpoint 0, and the Reset Endpoint and Set TR Dequeue Pointer recovery.
- A controller whose Supported Protocol capability defines speed ids other than the defaults.
