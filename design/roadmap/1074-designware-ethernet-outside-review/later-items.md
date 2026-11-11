# P2: later

An appendix to [the milestone](../1074-designware-ethernet-outside-review.md).

12. Literal field values in the controller sequences.
   Finding: `receive_queue_mode` writes `1 << 7`, `0x3f << 14`, `0x3f << 8`, `0x3 << 14` and
   `0x1 << 8` for the flow-control enable and its two thresholds (`src/network_interface_controller.rs:203-208`), <!-- line-citation: exception. Reason: pinned -->
   with the source's names only in a comment. `stop` polls the flush bit `10_000` times at 1 µs
   (`src/network_interface_controller.rs:610`) where the reset and the MDIO waits have named limits. <!-- line-citation: exception. Reason: pinned -->
   A test checks the jumbo bit as `1 << 16` (`src/network_interface_controller.rs:843`). The <!-- line-citation: exception. Reason: pinned -->
   module's own rule is that every bit it writes is a named constant with the source's spelling in
   an alias.
   Change: five constants in `registers` (OpenBSD's `GMAC_MTL_CHAN_RX_OP_MODE_EHFC`, `_RFD_*` and
   `_RFA_*` name them, not fetched for this review), a `FLUSH_LIMIT_MICROSECONDS`, and a named jumbo
   bit for the test.
   Door: two-way. Effort: S.

13. A caller's short buffer loses the frame.
   Finding: `receive` judges each descriptor against `out.len()` (`src/data_plane.rs:140`), so a <!-- line-citation: exception. Reason: pinned -->
   frame longer than the buffer offered is `Drop`: consumed, counted and gone. The documentation of
   `receive` says a `Drop` is "a bad frame". `components` always offers `MAXIMUM_FRAME_BYTES`, so no
   caller today meets it, and the proof that a delivered frame fits the room is what makes the
   contract safe.
   Change: say so at `receive`, in one sentence; or return the length and leave the descriptor for a
   larger buffer, as `smoltcp`'s `RxToken` hands the frame to a closure sized by the device. The
   sentence is enough before the tag.
   Door: two-way. Effort: XS for the sentence.

14. The PHY's extended-address port comes back only on success.
   Finding: `configure` and `apply_speed` save the address port first and write it back last
   (`src/motorcomm_physical_layer_device.rs:508,569,609,621`); any `?` between leaves the port on <!-- line-citation: exception. Reason: pinned -->
   the last extended register. The next access rewrites it, so the cost is one firmware access that
   lands on the wrong register, which the module says it is restoring the port to avoid.
   Change: restore in both arms, or say the restore is best effort.
   Door: two-way. Effort: XS.

15. An unknown drive current gets the part's default without a word.
   Finding: `drive_code` returns 3 for a `(voltage, microamps)` pair the table does not hold
   (`src/motorcomm_physical_layer_device.rs:392-418`), as OpenBSD does by the crate's account. <!-- line-citation: exception. Reason: pinned -->
   Linux's `motorcomm.c` refuses the probe for such a pair (from memory, not verified for this
   review). A tree with a typo in `motorcomm,rx-clk-drv-microamp` then runs at the default drive
   and nobody learns of it.
   Change: `Applied` carries which codes were defaults, or `SettingsSource` gains a variant; the
   kernel's report line can then print it.
   Door: two-way. Effort: S.

16. The fallback settings claim both trees and the test checks half of it.
   Finding: `Settings::FALLBACK_GMAC0` says both trees give these values
   (`src/motorcomm_physical_layer_device.rs:339-351`), with the transmit clock inverted at all three <!-- line-citation: exception. Reason: pinned -->
   speeds; the test that compares the trees (`src/motorcomm_physical_layer_device.rs:683-692`) <!-- line-citation: exception. Reason: pinned -->
   builds mainline's settings with `[false, true, true]` and compares delays and drives only. Which
   tree is right about 10 Mbit/s is not verified here.
   Change: compare the inversions too, or say in the fallback's documentation which tree it follows
   for the inversion and why.
   Door: two-way. Effort: XS.

17. One verdict name covers two disagreements, and one precondition is unsaid.
   Finding: `classify`'s last arm (`src/cache_coherence.rs:198`) returns `TransmittedPayloadStale` <!-- line-citation: exception. Reason: pinned -->
   for every delivered descriptor that is not a `Frame(128)` with a matching payload. That includes
   a descriptor reporting an error summary, or another length, over a payload that matches: the
   descriptor disagrees, not the payload. The module header does not say that MAC loopback needs the
   PHY's receive clock. The databook loops the transmit path back but not the clock, which is why
   the driver keeps that clock running (from the sources the crate cites, not verified here).
   Change: a `DescriptorDisagrees` verdict, or one sentence at the arm; one sentence in the header
   on the receive clock.
   Door: two-way (a new variant on a public enum before the tag). Effort: S.

18. The transport re-derives the data plane's peek.
   Finding: `components/src/designware_ethernet_transport.rs:143-148` reads the next descriptor <!-- line-citation: exception. Reason: pinned -->
   from `receive_cursor()` and judges it with `receive_verdict` to know whether a frame waits,
   repeating the first two lines of `receive`.
   Change: `DataPlane::is_frame_waiting(&self, region: &impl DirectMemoryAccessRegion) -> bool`,
   and the transport calls it.
   Door: two-way (an addition). Effort: XS.

19. `start` twice desynchronizes the cursors.
   Finding: `DataPlane::start` reposts the ring and clears the transmit ring but leaves
   `receive_next`, `transmit_next` and `transmit_clean` where they were (`src/data_plane.rs:106-124`); <!-- line-citation: exception. Reason: pinned -->
   the documentation says only to call it first.
   Change: reset the three cursors in `start`, or say it runs once after `new`.
   Door: two-way. Effort: XS.

20. Small text fixes.
   Finding: "current current" (`src/motorcomm_physical_layer_device.rs:390`); the file uses `MTL` at <!-- line-citation: exception. Reason: pinned -->
   `src/registers.rs:7` and expands it only at line 174; `Identity::direct_memory_access_address_bits` <!-- line-citation: exception. Reason: pinned -->
   is a width, and the address rule reads it as an address
   (`src/network_interface_controller.rs:75`): `…_address_width_bits` says what it is. <!-- line-citation: exception. Reason: pinned -->
   Door: two-way, the last one one-way at the tag. Effort: XS.

21. The crate root re-exports 27 descriptor items.
   Finding: `lib.rs` re-exports every descriptor bit and the two descriptor constructors at the
   crate root beside `DataPlane` and the three traits (`src/lib.rs:106-121`), while the register map <!-- line-citation: exception. Reason: pinned -->
   has its own module. A docs.rs reader meets thirteen `RECEIVE_DESCRIPTOR_WORD_3_*` constants
   before `DataPlane`.
   Change: a public `descriptor` module and a shorter root, or leave it and say why. The
   maintainer decides before the tag; after it the paths stay fixed.
   Door: two-way now, one-way at the tag. Effort: S.
