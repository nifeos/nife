// This module's docs are on its `pub mod` line in `lib.rs`.

use capability_plan::Direction::{Serve, Use};
use capability_plan::{CapNeed, MapNeed, PageKind, Requirements, StateHandoff};

/// Where the handoff page sits. Clear of the three pages `line_editor` already maps.
pub const HANDOFF_VA: u64 = 0x00a0_0000;
/// One page. Measured, not assumed: a full history ring and a full queue are under 4 KiB.
pub const HANDOFF_PAGES: u64 = 1;
/// Pages one instance is built out of: its image, its `.bss` (the discipline, the queues), a
/// twelve-page stack, its page tables and revocation log: twelve image pages measured on riscv64
/// (2026-09-27), about thirty in all, so roughly one and a half times what it uses.
pub const INSTANCE_PAGES: u64 = 48;
/// The stack a supervisor builds an instance with: `system_initializer`'s `CHILD_STACK_PAGES`,
/// which is what `line_editor` has always run on.
pub const STACK_PAGES: u64 = 12;

const HANDOFF: StateHandoff = StateHandoff {
    va: HANDOFF_VA,
    pages: HANDOFF_PAGES,
};
const MAPS: &[MapNeed] = &[
    MapNeed {
        role: "sink_page",
        va: 0x0060_0000,
        kind: PageKind::Shared,
    },
    MapNeed {
        role: "client_out",
        va: 0x0080_0000,
        kind: PageKind::ReadOnly,
    },
    MapNeed {
        role: "client_in",
        va: 0x0090_0000,
        kind: PageKind::Shared,
    },
];

/// A `line_editor` printing through the console server.
pub const CONSOLE: Requirements = Requirements {
    contract: "terminal",
    caps: &[
        CapNeed {
            role: "terminal",
            direction: Serve,
        },
        CapNeed {
            role: "sink",
            direction: Use,
        },
        CapNeed {
            role: "sink_reply",
            direction: Serve,
        },
        CapNeed {
            role: "control",
            direction: Serve,
        },
        CapNeed {
            role: "supervisor",
            direction: Use,
        },
    ],
    maps: MAPS,
    pages: INSTANCE_PAGES,
    // It forwards synchronously to its sink while serving the shell, which is the one shape
    // `depends_on` exists to name.
    depends_on: &["console"],
    state_handoff: Some(HANDOFF),
};

/// A `line_editor` printing through `display_terminal`.
pub const DISPLAY: Requirements = Requirements {
    contract: "terminal",
    caps: &[
        CapNeed {
            role: "terminal",
            direction: Serve,
        },
        CapNeed {
            role: "sink",
            direction: Use,
        },
        CapNeed {
            role: "control",
            direction: Serve,
        },
        CapNeed {
            role: "supervisor",
            direction: Use,
        },
    ],
    maps: MAPS,
    pages: INSTANCE_PAGES,
    depends_on: &["display"],
    state_handoff: Some(HANDOFF),
};

/// How `terminal_supervisor` is endowed, by `system_initializer` on a real boot and by the
/// kernel's own test: one list both builders and the supervisor read (AGENTS.md rule 7).
///
/// Start arguments: the output sink's mode (`MODE_CONSOLE` = 0 or `MODE_DISPLAY` = 1), the
/// lengths of the images copied to [`ELF_VA`](supervisor::ELF_VA) and
/// [`OWN_IMAGE_ADDRESS`](supervisor::OWN_IMAGE_ADDRESS) packed by
/// [`image_lengths`](supervisor::image_lengths), and the slot of a swap-request endpoint, `0` for
/// none. See notes/line-discipline.md ("The component's slots") for what it holds and why.
///
/// Name: provisional (the lane for milestone 23, 2026-09-27), for the module and every constant
/// in it. `terminal_supervisor` itself is calef's to name.
pub mod supervisor {
    /// The construction budget.
    pub const BUDGET: u64 = 0;
    /// The terminal endpoint: `READ | WRITE | GRANT`.
    pub const TERMINAL: u64 = 1;
    /// The output sink's request endpoint (console) or served endpoint (display).
    pub const SINK: u64 = 2;
    /// The page `line_editor` fills and the sink prints.
    pub const SINK_PAGE: u64 = 3;
    /// The client's outgoing page, which `line_editor` only reads.
    pub const CLIENT_OUT: u64 = 4;
    /// The client's incoming page, which `line_editor` writes completed lines into.
    pub const CLIENT_IN: u64 = 5;
    /// The console's reply endpoint, in console mode only.
    pub const SINK_REPLY: u64 = 6;
    /// In display mode, when the spawner hands over `display_terminal`'s image: the display
    /// driver's endpoint it presents through, `WRITE | GRANT`. The slot [`SINK_REPLY`] has in
    /// console mode, which has no display.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const PRESENT: u64 = 6;
    /// In display mode with `display_terminal`'s image: the scanout's frame run, `READ | WRITE |
    /// GRANT`. [`SINK`] is then the endpoint `display_terminal` serves and [`SINK_PAGE`] its
    /// output page, both with `GRANT`.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const SURFACE: u64 = 7;
    /// Where the `line_editor` image is copied into the supervisor. Below `supervision_protocol`'s
    /// scratch window, which starts at `0x1000_0000` and grows upward.
    pub const ELF_VA: u64 = 0x0800_0000;
    /// Where a `display_terminal` image is copied into the supervisor, when the spawner hands
    /// `display_terminal` to it. Between [`ELF_VA`]'s window and [`OWN_IMAGE_ADDRESS`]'s.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const DISPLAY_IMAGE_ADDRESS: u64 = 0x0a00_0000;
    /// Where the supervisor's own image is copied into it, so it can build the stand-in that
    /// carries its quiesce (`components/src/terminal_supervisor.rs`). Above the largest
    /// `line_editor` image [`ELF_VA`] could hold and below the scratch window.
    ///
    /// **Only a spawner that passes a swap endpoint copies it**, and grants
    /// [`SWAPPING_BUDGET_PAGES`] rather than [`BUDGET_PAGES`]. A supervisor never asked to swap
    /// never quiesces, so a boot pays nothing for the bound. A swap endpoint without the image
    /// traps at start; one without the budget traps at the first swap.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23 (a capability-routed
    /// component OS with live replacement).
    pub const OWN_IMAGE_ADDRESS: u64 = 0x0c00_0000;
    /// Pages a stand-in is built out of: its image, a four-page stack, its page tables and
    /// revocation log. Measured with `MemoryRegion::USAGE`'s committed count on 2026-10-19 (UTC),
    /// debug images: 27 on aarch64, 26 on riscv64, 33 on `x86_64`, whose image is two pages
    /// larger and whose page tables are four levels deep. So about one and a half times the
    /// largest, [`INSTANCE_PAGES`](super::INSTANCE_PAGES)'s margin.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const STAND_IN_PAGES: u64 = 48;
    /// **How long the supervisor waits for `OPERATION_QUIESCE` to be answered before it treats
    /// `line_editor` as hung**, in milliseconds of the counter `user_mode_runtime::now` reads. The
    /// measurement behind it is beside the bounded quiesce in `components/src/terminal_supervisor.rs`.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const QUIESCE_BOUND_MS: u64 = 250;
    /// How many pages of budget the supervisor needs: two instances at once (the incumbent
    /// and its replacement) and its own handful of objects and page tables.
    pub const BUDGET_PAGES: u64 = 2 * super::INSTANCE_PAGES + 16;
    /// [`BUDGET_PAGES`] plus a stand-in: what a spawner that passes a swap endpoint grants.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const SWAPPING_BUDGET_PAGES: u64 = BUDGET_PAGES + STAND_IN_PAGES;

    /// The second start argument: the lengths of the `line_editor` image at [`ELF_VA`], of the
    /// supervisor's own at [`OWN_IMAGE_ADDRESS`] and of `display_terminal`'s at
    /// [`DISPLAY_IMAGE_ADDRESS`], twenty-one bits each from the low end. `0` for an image not
    /// copied. Packed because a thread starts with three words and the other two are taken.
    /// `None` for an image of 2 MiB or more, which does not fit its field; the images this
    /// supervisor is handed are under 256 KiB.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const fn image_lengths(editor: u64, own: u64, display: u64) -> Option<u64> {
        if (editor | own | display) >> LENGTH_BITS != 0 {
            return None;
        }
        Some(editor | (own << LENGTH_BITS) | (display << (2 * LENGTH_BITS)))
    }

    /// [`image_lengths`] read back: `(editor, own, display)`.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const fn unpack_image_lengths(packed: u64) -> (u64, u64, u64) {
        let mask = (1 << LENGTH_BITS) - 1;
        (
            packed & mask,
            (packed >> LENGTH_BITS) & mask,
            (packed >> (2 * LENGTH_BITS)) & mask,
        )
    }

    const LENGTH_BITS: u64 = 21;

    /// Requester → supervisor, on the swap endpoint: replace a running child. The second word
    /// names which: [`CHILD_LINE_EDITOR`] or [`CHILD_DISPLAY_TERMINAL`].
    pub const SWAP: u64 = 1;
    /// `SWAP`'s second word: `line_editor`.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const CHILD_LINE_EDITOR: u64 = 0;
    /// `SWAP`'s second word: `display_terminal`, which only a supervisor handed its image holds.
    /// Its swap needs the swap reserve lent first ([`LEND`]).
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const CHILD_DISPLAY_TERMINAL: u64 = 1;
    /// Lender → supervisor, a `SEND_CAP` on the swap endpoint carrying a `MemoryRegion` split
    /// from the machine's swap reserve ([`SWAP_RESERVE_PAGES`]), lent for the
    /// next `SWAP` of [`CHILD_DISPLAY_TERMINAL`] (calef's ruling "D", 2026-10-19 UTC), with
    /// `GRANT`, because the replacement's map budget is split from it and delegated. The
    /// supervisor deletes its capability before it answers that `SWAP`, and the lender then
    /// `DESTROY`s the region, which revokes whatever was left mapped from it.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const LEND: u64 = 3;
    /// The machine's swap reserve, in pages (milestone 23 (a capability-routed component OS with
    /// live replacement); calef's ruling "D" of 2026-10-19 (UTC)).
    ///
    /// Sized to the largest swappable instance plus its handoff, and held to
    /// `video_terminal::component::SWAP_PAGES` by `terminal_supervisor` at compile time. It sits
    /// beside [`LEND`], the message that lends it, because every party to a swap reads this
    /// module. Lending it whole serializes swaps: a second swap finds nothing to split and is
    /// refused. See notes/line-discipline.md ("The component's slots").
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const SWAP_RESERVE_PAGES: u64 = 396;

    /// Supervisor → requester: a `SWAP` of [`CHILD_DISPLAY_TERMINAL`] with no reserve lent, or one
    /// too small. Nothing was quiesced; lend the reserve and ask again.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const NEEDS_RESERVE: u64 = 0x5245_5356;
    /// Supervisor → requester: `display_terminal`'s replacement absorbed the state and serves,
    /// but from the reserve: its move home was refused (second word: the refusal). The reserve
    /// stays held, and the next `SWAP` of it finishes the move with no new [`LEND`].
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const IN_RESERVE: u64 = 0x494e_5253;
    /// Requester → supervisor: retire the running `line_editor` (quiesce it, tell it to quit,
    /// collect it), reply [`STOPPED`], and exit. So a test's terminal can end with every
    /// region back in its budget; nothing on a boot sends it.
    pub const STOP: u64 = 2;
    /// Supervisor → requester: stopped, and exiting.
    pub const STOPPED: u64 = 0x5354_4f50;
    /// Supervisor → requester: the replacement absorbed the state and serves; the old one is
    /// gone. Second word: how many instances this supervisor has started.
    pub const SWAPPED: u64 = 0x5357_4150;
    /// Supervisor → requester: the replacement refused, and the incumbent serves on. Second
    /// word: the refusal `NOTE_NOT_ABSORBED` carried.
    pub const ROLLED_BACK: u64 = 0x524f_4c4c;
    /// Supervisor → requester: the incumbent did not answer the quiesce within
    /// [`QUIESCE_BOUND_MS`], so a replacement was started **without its state** and the incumbent's
    /// region destroyed. Every caller it was holding is answered `abi::Error::Gone`. Second word:
    /// how long the supervisor waited, in milliseconds.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub const REPLACED_HUNG: u64 = 0x4855_4e47;
}

const _: () = assert!(CONSOLE.problem().is_none());
const _: () = assert!(DISPLAY.problem().is_none());
// The slots `line_editor` reads without being told.
const _: () = assert!(capability_plan::slot_of(&CONSOLE, "terminal") == 0);
const _: () = assert!(capability_plan::slot_of(&CONSOLE, "sink") == 1);
const _: () = assert!(capability_plan::slot_of(&CONSOLE, "sink_reply") == 2);
const _: () = assert!(capability_plan::slot_of(&DISPLAY, "terminal") == 0);
const _: () = assert!(capability_plan::slot_of(&DISPLAY, "sink") == 1);
// And the one it is told: the notification endpoint follows the control endpoint in both.
const _: () = assert!(
    capability_plan::slot_of(&CONSOLE, "supervisor")
        == capability_plan::slot_of(&CONSOLE, "control") + 1
);
const _: () = assert!(
    capability_plan::slot_of(&DISPLAY, "supervisor")
        == capability_plan::slot_of(&DISPLAY, "control") + 1
);
// Two instances of INSTANCE_PAGES (48) and sixteen pages of the supervisor's own, written out
// so a changed operator in the sum is a build error. Mutation survivor triage, milestone 326
// (turn a mutation score upward), batch 3.
const _: () = assert!(supervisor::BUDGET_PAGES == 112);
// A stand-in more than that, and no more, written out for the same reason: a graphical session's
// pool has few pages to spare, so a spawner that passes a swap endpoint pays for exactly what the
// bounded quiesce builds.
const _: () = assert!(supervisor::SWAPPING_BUDGET_PAGES == 160);
#[cfg(test)]
mod tests {
    use super::supervisor::{image_lengths, unpack_image_lengths};

    /// All three lengths come back from the packed start argument `system_initializer` and the
    /// kernel's test write and `terminal_supervisor` reads, at sizes past a 16-bit field and at
    /// the 21-bit maximum, so a shift or mask of the wrong width shows. (`|` becoming `^` is the
    /// same function here: the lengths' bits never overlap.)
    #[test]
    fn every_image_length_survives_the_packed_start_argument() {
        let max = (1 << 21) - 1;
        for lengths in [
            (160_872, 155_600, 0),
            (160_872, 0, 0),
            (160_872, 155_600, 98_304),
            (max, max, max),
            (0, max, 0),
        ] {
            let (e, o, d) = lengths;
            assert_eq!(
                image_lengths(e, o, d).map(unpack_image_lengths),
                Some(lengths)
            );
        }
        // One bit too long for its field is refused rather than spilling into the next length.
        assert_eq!(image_lengths(0, max + 1, 0), None);
    }
}
