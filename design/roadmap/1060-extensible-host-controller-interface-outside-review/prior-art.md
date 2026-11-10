# Prior art for the extensible_host_controller_interface crate

An appendix to [the milestone](../1060-extensible-host-controller-interface-outside-review.md). The
reviewer described the drivers from general knowledge of their source; the crates.io figures were
read on 2026-11-10, with recent downloads over the last 90 days.

## Drivers and crates nearby

| project | downloads (recent / all) | scope | what a user coming from it will miss here |
|---|---|---|---|
| Linux `drivers/usb/host/xhci*` | n/a | the full driver: multi-segment rings that grow, event ring segment tables, scratchpads, bulk, isochronous and streams, hubs and route strings, command timeout and abort, a quirks table per vendor | everything past a single-slot boot keyboard on a root port; TD splitting at 64 kibibytes; the Stop Endpoint, No Op and Reset Device commands; multiple interrupters |
| U-Boot `drivers/usb/host/xhci*` | n/a | a polled driver: single-segment rings, control and bulk, hubs, bounded waits on every command and port operation | bulk, hubs, the port reset sequence with its timeouts, and a driver loop a reader can copy |
| `xhci` 0.9.2 (rust-osdev) | 41k / 123k, no release since 2023-07 | typed register, TRB and context structures with field accessors, an extended-capability iterator; no ring discipline, no proofs | an accessor per field and a typed register for every operational, runtime and port register |
| Redox `xhcid` | n/a | a user-space driver behind a scheme: rings, hubs, bulk and interrupt endpoints, a tree of attached devices | a running driver, hubs and bulk |
| `usb-device` 0.3.2 | 925k / 5.4M | the device side of USB, not a host | nothing that applies: the name is what a search for "usb" finds first |
| `usb-host` 0.1.3 | 81 / 8k | traits for host drivers, no implementation | a trait to implement |

## Where this crate differs

It computes and never touches. The caller performs every volatile access and owns the direct
memory access memory, so the crate runs under `no_std` with no allocator, tests on the host in
milliseconds and sits inside a model checker's reach. Seven Kani harnesses prove the ring bounds
and cycle discipline, the capability walk's bound, the context field placement, the port write's
preserved set and the interrupt interval, and each ships with the patch that makes it fail.

The port write is the concrete example. Half the port status register is write-one-to-clear, and
one of those bits is Port Enabled, so a driver that writes back what it read with one bit added
disables the port and acknowledges every change nobody has looked at. Linux neutralizes the value
in `xhci_port_state_to_neutral` and relies on review; this crate's `port::write_value` carries a
proof that writing nothing writes no change bit, no Port Enabled bit and no reset.

What it costs: one segment per ring, one interrupter, one slot's layout, no bulk, isochronous,
streams or hubs, and the smaller surface the last column lists.

## The shapes the ruled items follow

- An `Option` for an index that may not exist (P1-11) is what `slice::get` returns, and the
  standard library's spelling of "no such element".
- Enums for a field's codes (P1-3, P1-12) are how the `xhci` crate types the endpoint type and the
  TRB type, and how Linux names them with `#define` per value; the crate keeps the specification's
  spelling as a `doc(alias)`, as its register names do.
- A refusing enqueue (P1-13) is what Linux's `room_on_ring` and U-Boot's single-command discipline
  give: neither lets a caller overwrite a TRB the controller has not consumed.
