# The `Frame` rename, as built

An appendix to [milestone 158 (build the eleven kernel object and identifier
renames)](../0158-kernel-object-rename-build.md). It holds the lane's account of `Frame` becoming
`PageFrame`: the senses of "frame" it had to tell apart, what did rename, the `DeviceFrame` call and
how it was verified.

## The senses of "frame" left alone

Built 2026-08-24. The compositor collision §113 (eleven kernel object and identifier names move to the plain, standard term) named is real. It was checked file by file rather
than trusted from a capitalized-token grep. "frame" (lowercase) is a common English word with several
genuinely distinct in-tree senses, and all had to be told apart before touching anything.

- `crates/compositor`'s own sense, a rendered screen update (the collision §113 named). It was left
  untouched throughout `crates/compositor/src/lib.rs`, `kernel/src/user/compositor_service.rs`,
  `components/src/compositor.rs` (`serve_frame`, "Serve one frame. Read every client's control
  page, composite whatever changed"), and `crates/watch`/`kernel/src/user/watch_tests.rs`
  (`a_second_frame_erases_the_first_rather_than_leaving_it_on_screen`, a terminal redraw, the same
  sense; both deleted on 2026-09-13). Two of these instances sit on lines immediately adjacent to
  genuine `PageFrame` renames in the same file. `kernel/src/user/compositor_service.rs` has its
  `zeroed_page_frame()` helper beside
  its own "the compositor has processed the frame" prose two functions over. That is exactly the
  fine-grained case the milestone doc's own warning anticipated.
- A CPU call frame (`TrapFrame`, compiler stack-size accounting), not named or anticipated by §113.
  `kernel/src/user.rs`'s `enter_frame`/`enter_at` builds the trap frame a thread's first
  `eret`/`sret` restores from. That, every architecture's `exceptions.rs`, and `notes/frames.md`'s
  own stack-overflow postmortem section all use "frame" for this unrelated concept. They were left
  alone.
- A raw network frame (Ethernet/ARP/mDNS), also not anticipated by §113. `crates/virtio::send_frame`
  (a virtio-net transmit), `components/src/virtio_net_transport.rs`'s `VnetRxToken { frame: Vec<u8>
  }`, and `xtask/src/main.rs`'s `arp_request_frame`/`mdns_query_frame` packet builders. None
  renamed.
- Arbitrary example text, unrelated to any of the above. `crates/manual/src/index.rs`'s search-index
  tokenizer test used the literal string `` `Frame` `` as stand-in content to exercise a generic
  tokenizer, not as a citation of the kernel object. The initial blanket capitalized-token pass
  touched it by accident, and it broke a test. It was reverted to `Frame` once the test failure
  traced it back. This is recorded because it is the one miss this lane's own build caught rather
  than caught in review.

## What did rename

These renamed once the four collisions above were excluded.

- The type: `Frame` -> `PageFrame`, and `FrameAllocator` -> `PageFrameAllocator` in
  `crates/page_frames`. That crate is this object's own underlying physical-page allocator. It was
  swept at the same identifier-only rung as `crates/page_frames` itself, not full prose.
- The `abi::frame` -> `abi::page_frame` ABI module. Its methods `MAP`/`REVOKE` keep their own names,
  unchanged, matching how `Endpoint`'s `SEND`/`RECV` were left alone.
- A long tail of lowercase functions, constants, fields and module paths that spell the concept out:
  - `frame_cap` -> `page_frame_cap`, `delete_frame_caps` -> `delete_page_frame_caps`,
    `revoke_frame` -> `revoke_page_frame`, `frame_map`/`frame_revoke` ->
    `page_frame_map`/`page_frame_revoke`;
  - `is_frame_used`/`bring_up_frames`/`free_frames` -> their `page_frame` equivalents;
  - `map_current_user_frame` (all three architectures) -> `map_current_user_page_frame`;
  - `OutOfFrames` -> `OutOfPageFrames` (`crates/paging`);
  - `zeroed_frame` -> `zeroed_page_frame`, in three IOMMU files plus `compositor_service.rs`'s own,
    unrelated to that file's screen-frame prose;
  - the `FRAME_REPORT_MIN`/`SUITE_FRAME_BUDGET`/`rank::FRAMES` test-ledger and lock-rank constants;
  - `SURFACE_FRAMES`/`SCREEN_FRAMES`/`DMA_FRAMES` (`graphics_proto`, `compositor_service.rs`,
    `display_service.rs`) -> their `_PAGE_FRAMES` forms;
  - `OP_ATTACH_FRAME` (`socket_proto`, a shared-memory attach opcode, not a network frame) ->
    `OP_ATTACH_PAGE_FRAME`;
  - the `grant_plan::jobframe` module -> `job_page_frame`;
  - the `kernel/src/user/frame_service.rs` module (a kernel-side Frame-producer/consumer test
    wiring) -> `page_frame_service.rs`, now in `system_tests/src/user/`.

`FRAME_SIZE` was checked and left alone deliberately. Every use names the size of one physical page
frame, which is the correct, unambiguous sense already.

## `DeviceFrame`, left unrenamed

`DeviceFrame` was left unrenamed. That was a judgment call rather than something §113 decided. Its
own doc comment already disambiguates it from the compositor ("a device's MMIO page"). It was not
among §113's eleven names or the `CSpace` amendment. Companion-renaming an unlisted sibling is the
kind of scope creep `AGENTS.md`'s naming section asks a lane not to do on its own initiative.
`DeviceFrame`'s own doc citations of bare `` `Frame` `` were updated to `` `PageFrame` ``, since
those are citations of the type that did rename.

## Verified

- `script/lint` clean.
- The full host test suite (`script/test`, all three architectures: aarch64, riscv64, x86_64) green.
- Both Kani-proof-bearing crates that reference the renamed type (`crates/capability` and
  `crates/page_frames`) verify successfully post-rename, satisfying the proof obligation of milestone 69 (split `kernel/src/user.rs` by service).

`notes/frames.md` was rewritten with a provenance note for a reader who remembers `Frame`, following
`notes/ipc-naming.md`'s precedent from the `Endpoint` rename. Its stack-frame postmortem section (a
CPU call frame investigation, unrelated to this object) is called out explicitly as deliberately
untouched, rather than silently left inconsistent.
