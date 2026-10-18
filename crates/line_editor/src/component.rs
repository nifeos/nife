// Moved out of `lib.rs` unchanged by milestone 878 (the line editor's inline modules move to
// their own files). Its docs stay on its `pub mod` line in `lib.rs`, where they always were.

use component_plan::Direction::{Serve, Use};
use component_plan::{CapNeed, MapNeed, PageKind, Requirements, StateHandoff};

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

/// **How `terminal_supervisor` is endowed**, by `system_initializer` on a real boot and by the
/// kernel's own test: one list both builders and the supervisor read (AGENTS.md rule 7). The
/// supervisor holds every object a `line_editor` is built from, with `GRANT`, so it can build a
/// second one; and the terminal endpoint with `WRITE` too, so it can send `OPERATION_QUIESCE`.
///
/// Start arguments: the output sink's mode (`line_editor`'s own `MODE_CONSOLE` = 0 or
/// `MODE_DISPLAY` = 1), the length of the `line_editor` image copied to [`ELF_VA`](supervisor::ELF_VA), and the slot
/// of an endpoint to receive swap requests on, `0` for none (a boot today: the trigger is
/// milestone 198 (a package manager, and the trivial install)'s installer and is not built).
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
    /// Where the `line_editor` image is copied into the supervisor. Below `supervision_protocol`'s
    /// scratch window, which starts at `0x1000_0000` and grows upward.
    pub const ELF_VA: u64 = 0x0800_0000;
    /// How many pages of budget the supervisor needs: two instances at once (the incumbent
    /// and its replacement) and its own handful of objects and page tables.
    pub const BUDGET_PAGES: u64 = 2 * super::INSTANCE_PAGES + 16;

    /// Requester → supervisor, on the swap endpoint: replace the running `line_editor`.
    pub const SWAP: u64 = 1;
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
    /// word: the refusal `NOTE_REFUSED` carried.
    pub const ROLLED_BACK: u64 = 0x524f_4c4c;
}

const _: () = assert!(CONSOLE.problem().is_none());
const _: () = assert!(DISPLAY.problem().is_none());
// The slots `line_editor` reads without being told.
const _: () = assert!(component_plan::slot_of(&CONSOLE, "terminal") == 0);
const _: () = assert!(component_plan::slot_of(&CONSOLE, "sink") == 1);
const _: () = assert!(component_plan::slot_of(&CONSOLE, "sink_reply") == 2);
const _: () = assert!(component_plan::slot_of(&DISPLAY, "terminal") == 0);
const _: () = assert!(component_plan::slot_of(&DISPLAY, "sink") == 1);
// And the one it is told: the notification endpoint follows the control endpoint in both.
const _: () = assert!(
    component_plan::slot_of(&CONSOLE, "supervisor")
        == component_plan::slot_of(&CONSOLE, "control") + 1
);
const _: () = assert!(
    component_plan::slot_of(&DISPLAY, "supervisor")
        == component_plan::slot_of(&DISPLAY, "control") + 1
);
// Two instances of INSTANCE_PAGES (48) and sixteen pages of the supervisor's own, written out
// so a changed operator in the sum is a build error. Mutation survivor triage, milestone 326
// (turn a mutation score upward), batch 3.
const _: () = assert!(supervisor::BUDGET_PAGES == 112);
