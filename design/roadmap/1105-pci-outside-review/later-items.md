# P2: later

An appendix to [the milestone](../1105-pci-outside-review.md).

## P2

#### `virtio_capabilities` ignores `cap_len`
Where: `crates/pci_configuration_space/src/lib.rs` lines 485 to 499.
Why: a notify capability whose `cap_len` is 16 has no multiplier, and the walk reads one from the
next entry's header. The virtio specification fixes 16 and 20 as the minimum lengths. A function
that states less is bad data: skip it, do not decode it.
Fix: read `cap_len` from byte 2 of the head, skip an entry shorter than its type requires, and add
the case to the fake configuration space.
Effort: S. Door: two-way.

#### `intx_interrupt_number` is the root-bus swizzle, and nothing says so
Where: `crates/pci_configuration_space/src/lib.rs` lines 709 to 739; the caller at `kernel/src/pci.rs` line 702 passes `bdf.dev`
whatever the bus.
Why: since the walk follows bridges, the walk finds a function behind a root port on a device-tree
board. It computes the function's line from its own device number alone, but the PCI-to-PCI bridge
specification composes the pin through each bridge's device number (Linux
`pci_swizzle_interrupt_pin`). Both `virt` boards are flat, so no run has met it.
Fix: state the precondition in the doc comment (bus 0, or a bridge path already composed), and add
`intx_pin_through_bridge(device, pin) -> pin` so a caller can compose it. The kernel-side use is a
BUGS entry for `kernel/src/pci.rs`.
Effort: S. Door: two-way.

#### `Bdf` admits a device above 31 or a function above 7, and the ECAM proof assumes it does not
Where: `crates/pci_configuration_space/src/lib.rs` lines 22 to 41, `:806` (`kani::assume`).
Why: `ecam_offset` for `dev: 32` lands on the next bus's first device, and the mutation triage
records two equivalent mutants there because no test can pin behavior nobody wants. The fields are
public and the kernel builds `Bdf` values by hand.
Fix: mask in `ecam_offset` and `requester_id` (`dev & 31`, `func & 7`) so the proof needs no
assumption, or a checked constructor that returns `None`; both let the two mutants die.
Effort: S. Door: two-way.

#### The crate has no `unsafe` and does not say so, and fuzzes nothing
Where: `crates/pci_configuration_space/src/lib.rs` lines 15 to 18.
Why: `#![forbid(unsafe_code)]` is the one-line claim the template's `unsafe_scan` reads, and it is
what makes the weekly fuzz opt-in allowed. The two parsers of firmware and device input
(`mem32_window` over a byte string; `walk` and the capability walks over a byte-array configuration
space) are what a fuzz target is for.
Fix: add the attribute. Add two fuzz targets under `fuzz/` with a seed from each fake configuration
space. Opt in with `public-crash-inputs: true` once the repository exists.
Effort: S. Door: two-way.

#### The falsification patches carry stale hunk headers and pre-rename names
Where: `crates/pci_configuration_space/falsifications/*.patch` (`virtio_caps`, `msix_cap` in two hunk headers; every
line number).
Why: `patch` finds the hunks by context, so they apply today, and `script/crate-move` regenerates
them for the move. A reader of the patch file meets names the crate no longer has.
Fix: regenerate in place, in nife, when item 3's new patch lands.
Effort: S. Door: two-way.

#### The proof table in nife says four harnesses; the crate has eight
Where: `notes/verification.md` line 123, `notes/verification/parser-harnesses.md` lines 35 to 45.
Why: the four bus-queue and ranges harnesses are missing from the record a reader uses to learn what
has a proof.
Fix: the nife-side rows, in the pull request that moves the crate.
Effort: S. Door: two-way.

#### The mutation record names no survivor
Where: `.cargo/mutants-baseline.txt` line 74; the staged `.github/mutants-exempts`.
Why: the baseline row carries counts only. The triage page classifies the 11 as equivalent but names
them by function, not by line. The template's gate needs `<file>:<line> # <reason>` and
will list them after the first weekly run.
Fix: after the first full run in the repository, exempt each of the 11 with the triage page's
reason. Alternatively kill the two `ecam_offset`/`requester_id` ones through item 11.
Effort: S. Door: two-way.
