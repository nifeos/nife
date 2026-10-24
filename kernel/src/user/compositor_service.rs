use compositor::SCENE;

use super::*;
use crate::cap::{Rights, memory_region_cap, page_frame_run_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// The budget the compositor and a capture client draw their screen mapping's page tables from,
/// sized as `display_service::MAP_BUDGET_PAGES` is: the 683-page screen spans two 2 MiB windows.
const MAP_BUDGET_PAGES: u64 = 24;

// The compositor's address space. Must match components/src/compositor.rs. The compositor maps the
// screen capability where it chooses, up to 4 MiB from its `SCREEN_VA`, and these sit clear of it.
const WLIST_VA: u64 = address_space_map::pair_page(0x0000_0000_0c00_0000);
const RING_VA: u64 = address_space_map::pair_page(0x0000_0000_0c01_0000);
const CLIENT_BASE: u64 = address_space_map::pair_page(0x0000_0000_0e00_0000);
const CLIENT_STRIDE: u64 = 0x0000_0000_0010_0000;

// A client's address space, the same in every client. Must match fixtures/src/window.rs, which
// maps the screen itself and keeps `C_WLIST_VA` clear of it.
const CTL_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
const SURFACE_VA: u64 = address_space_map::pair_page(0x0000_0000_0061_0000);
const C_WLIST_VA: u64 = address_space_map::pair_page(0x0000_0000_0c00_0000);

// Client roles. Must match fixtures/src/window.rs.
pub const ROLE_INPUT: u64 = 1 << 0;
pub const ROLE_PROBE_INPUT: u64 = 1 << 1;
pub const ROLE_PROBE_NEIGHBOUR: u64 = 1 << 2;
pub const ROLE_PROBE_SCREEN: u64 = 1 << 3;
pub const ROLE_CAPTURE: u64 = 1 << 4;
pub const ROLE_VICTIM: u64 = 1 << 5;
pub const ROLE_SMALL_DAMAGE: u64 = 1 << 6;
pub const ROLE_PROBE_READ: u64 = 1 << 7;
pub const ROLE_LIE_DAMAGE: u64 = 1 << 8;
pub const ROLE_FORGE_CTL: u64 = 1 << 9;

/// The screen's frames, in the scanout's own geometry. The same run of frames rung one's driver
/// scans out, because the screen *is* rung one's surface.
const SCREEN_PAGE_FRAMES: core::num::NonZeroU64 =
    crate::cap::page_frame_run_len(graphics_contract::SURFACE_PAGE_FRAMES as u64);

/// What the kernel keeps after wiring a scene: the endpoints it can ring or listen on, and the
/// physical addresses of the frames it allocated, which it reads through the direct map as a
/// witness no process controls.
pub struct Wiring {
    /// Where clients (and the input source) ring. The kernel holds WRITE so a test can play the
    /// input driver.
    pub doorbell: RendezvousId,
    /// The compositor's report endpoint.
    pub report: RendezvousId,
    /// The screen's frames.
    pub screen: u64,
    /// The window-list page the compositor publishes.
    pub wlist: u64,
    /// The input ring page, shared with the input source and nobody else.
    pub ring: u64,
    /// Each client's frames: its control page, then its surface.
    pub client: [u64; compositor::MAX_WINDOWS],
    /// Each client's report endpoint. One per client, so the kernel knows who is speaking: the
    /// kernel is the spawner and may hold per-client channels, which is exactly the identity the
    /// compositor deliberately does not have.
    pub client_report: [RendezvousId; compositor::MAX_WINDOWS],
    /// Each focusable client's input endpoint (the compositor holds WRITE, the client READ).
    pub input: [RendezvousId; compositor::MAX_WINDOWS],
    pub n: usize,
    pub focusable: usize,
    image: &'static [u8],
    /// The display terminal's image, so a scene can hold window clients, terminals, or both.
    term_image: &'static [u8],
    ring_tail: u32,
}

/// Wire a scene of `n` windows, the first `focusable` of them taking input, and start the
/// compositor. `display` speaks the display contract (`gpu_driver`, or a kernel stand-in), and
/// `screen` is the frames it scans out.
///
/// Returns once the compositor is spawned. A caller may spawn clients before `status::COMP_UP`
/// arrives on [`Wiring::report`], since a client's first act is a content-free `HELLO`. See
/// notes/compositor/kernel-wiring.md.
pub fn start(n: usize, focusable: usize, display: RendezvousId, screen: u64) -> Wiring {
    assert!(n <= SCENE.len() && n <= compositor::MAX_WINDOWS && focusable <= n);
    let image = program("compositor").expect("no compositor program in the initial ramdisk");
    let client_image = program("window").expect("no window program in the initial ramdisk");
    let term_image =
        program("display_terminal").expect("no display_terminal program in the initial ramdisk");

    let wlist = zeroed_page_frame();
    let ring = zeroed_page_frame();
    let doorbell = crate::sched::create_rendezvous();
    let report = crate::sched::create_rendezvous();

    // One contiguous run for every client's frames, so a client's neighbour's pixels are the frames
    // physically after its own grant and the neighbour attack is a real one.
    let mut per_client = [0u64; compositor::MAX_WINDOWS];
    let mut total = 0u64;
    for (i, win) in SCENE.iter().take(n).enumerate() {
        per_client[i] = 1 + win.page_frames() as u64; // a control page, then the surface
        total += per_client[i];
    }
    // Zeroed so no client and no test ever reads a stale pixel.
    let run_base = crate::memory::alloc_contiguous_zeroed(total as usize)
        .expect("no contiguous run for the compositor's client surfaces")
        .addr();

    let mut client = [0u64; compositor::MAX_WINDOWS];
    let mut client_report = [0; compositor::MAX_WINDOWS];
    let mut input = [0; compositor::MAX_WINDOWS];
    let mut at = run_base;
    for i in 0..n {
        client[i] = at;
        at += per_client[i] * FRAME_SIZE;
        client_report[i] = crate::sched::create_rendezvous();
    }
    for ep in input.iter_mut().take(focusable) {
        *ep = crate::sched::create_rendezvous();
    }

    let (maps, m) = compositor_maps(wlist, ring, n, &per_client, &client);

    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the compositor");

    let mut grants = [rendezvous_cap(report, Rights::WRITE); MAX_COMP_GRANTS];
    grants[1] = rendezvous_cap(display, Rights::WRITE);
    grants[2] = rendezvous_cap(doorbell, Rights::READ);
    grants[3] = page_frame_run_cap(
        screen,
        SCREEN_PAGE_FRAMES,
        Rights::READ.union(Rights::WRITE),
    );
    grants[4] = memory_region_cap(budget);
    for i in 0..focusable {
        grants[COMP_INPUT_BASE as usize + i] = rendezvous_cap(input[i], Rights::WRITE);
    }
    let ngrants = COMP_INPUT_BASE as usize + focusable;

    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: n as u64,
                arg1: focusable as u64,
                arg2: 0,
                grants: &grants[..ngrants],
                maps: &maps[..m],
            },
        )
    })
    .expect("could not spawn the compositor");

    Wiring {
        doorbell,
        report,
        screen,
        wlist,
        ring,
        client,
        client_report,
        input,
        n,
        focusable,
        image: client_image,
        term_image,
        ring_tail: 0,
    }
}

/// The compositor's `Spawn::maps`: the window list, the input ring, and each client's control page
/// and surface. Returns the table and how many entries are used.
///
/// The screen is not here: as one-page mappings it would overflow the 1 KiB a spawn closure may
/// capture (`kernel/src/thread.rs`), so it is one `PageFrame` grant the compositor maps itself.
fn compositor_maps(
    wlist: u64,
    ring: u64,
    n: usize,
    per_client: &[u64; compositor::MAX_WINDOWS],
    client: &[u64; compositor::MAX_WINDOWS],
) -> ([Mapping; MAX_COMP_MAPS], usize) {
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; MAX_COMP_MAPS];
    let mut m = 0;
    maps[m] = Mapping {
        va: WLIST_VA,
        phys: wlist,
        flags: Flags::user_data(),
    };
    m += 1;
    maps[m] = Mapping {
        va: RING_VA,
        phys: ring,
        flags: Flags::user_data(),
    };
    m += 1;
    for i in 0..n {
        for k in 0..per_client[i] {
            maps[m] = Mapping {
                va: CLIENT_BASE + i as u64 * CLIENT_STRIDE + k * FRAME_SIZE,
                phys: client[i] + k * FRAME_SIZE,
                flags: Flags::user_data(),
            };
            m += 1;
        }
    }
    (maps, m)
}

impl Wiring {
    /// Spawn window client `i` in `role`. Its whole authority: a report endpoint, the doorbell, its
    /// own control page and surface, an input endpoint if it is focusable, and (only for
    /// [`ROLE_CAPTURE`]) a read-only grant of the screen and a mapping of the window list.
    ///
    /// BUGS: the slot's frames are mapped as they are, not scrubbed, so a client spawned into a
    /// slot an earlier client used would see that client's last pixels until it painted. Nothing
    /// respawns a client today. Read from this code, not run. See notes/compositor-claim-25.md.
    pub fn spawn_client(&self, i: usize, role: u64) {
        self.spawn_client_probing(i, role, self.neighbour_probe_va(i));
    }

    /// [`spawn_client`](Self::spawn_client) with the address a probing role reaches for chosen by the
    /// caller rather than fixed at its neighbour's first pixel frame, so one test can aim the same
    /// attacker at the neighbour's control page and at each of its pixel frames.
    pub fn spawn_client_probing(&self, i: usize, role: u64, probe: u64) {
        let frames = SCENE[i].page_frames() as u64;
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; MAX_CLIENT_MAPS];
        let mut m = 0;
        maps[m] = Mapping {
            va: CTL_VA,
            phys: self.client[i],
            flags: Flags::user_data(),
        };
        m += 1;
        for k in 0..frames {
            maps[m] = Mapping {
                va: SURFACE_VA + k * FRAME_SIZE,
                phys: self.client[i] + (1 + k) * FRAME_SIZE,
                flags: Flags::user_data(),
            };
            m += 1;
        }
        if role & ROLE_CAPTURE != 0 {
            maps[m] = Mapping {
                va: C_WLIST_VA,
                phys: self.wlist,
                flags: Flags::user_rodata(),
            };
            m += 1;
        }

        let mut grants = [rendezvous_cap(self.client_report[i], Rights::WRITE); 3];
        grants[1] = rendezvous_cap(self.doorbell, Rights::WRITE);
        let ngrants = if i < self.focusable {
            grants[2] = rendezvous_cap(self.input[i], Rights::READ);
            3
        } else {
            // Slot 2 must stay empty: this client's attempt to receive input is `NoSuchSlot`, not
            // anyone's refusal. See ROLE_PROBE_INPUT.
            2
        };

        // The screen (slot 3, `READ` only) and its map budget (slot 4) go only to `ROLE_CAPTURE`, by
        // `grant_at`, since a sequential fill cannot leave slot 2 empty. See
        // notes/compositor/kernel-wiring.md.
        let capture_budget = if role & ROLE_CAPTURE != 0 {
            Some(
                crate::memory_region::create(MAP_BUDGET_PAGES)
                    .expect("no map budget for a capture client"),
            )
        } else {
            None
        };
        let screen = self.screen;

        let image = self.image;
        crate::sched::spawn(move || {
            if let Some(budget) = capture_budget {
                crate::sched::grant_at(
                    3,
                    page_frame_run_cap(screen, SCREEN_PAGE_FRAMES, Rights::READ),
                )
                .expect("client slot 3 was occupied");
                crate::sched::grant_at(4, memory_region_cap(budget))
                    .expect("client slot 4 was occupied");
            }
            run(
                image,
                Spawn {
                    arg0: role,
                    arg1: probe,
                    arg2: 0,
                    grants: &grants[..ngrants],
                    maps: &maps[..m],
                },
            )
        })
        .expect("could not spawn a window client");
    }

    /// The address, in client `i`'s own address space, where its neighbour's first pixel page
    /// would be: two pages past the end of `i`'s surface, after the neighbour's control page. The
    /// attacker is handed it so the test can assert on the exact faulting address.
    pub fn neighbour_probe_va(&self, i: usize) -> u64 {
        SURFACE_VA + (SCENE[i].page_frames() as u64 + 1) * FRAME_SIZE
    }

    /// The physical frame the probe address would reach if it were mapped. The test asserts this is
    /// the neighbour's first pixel frame, which is what makes the attack a real one.
    pub fn neighbour_probe_phys(&self, i: usize) -> u64 {
        self.client[i] + (SCENE[i].page_frames() as u64 + 2) * FRAME_SIZE
    }

    /// Client `i`'s surface, digested by the kernel through the direct map: a witness that
    /// belongs to nobody in userspace.
    pub fn client_surface_digest(&self, i: usize) -> u64 {
        let base = mmu::phys_to_virt(self.client[i] + FRAME_SIZE);
        compositor::surface_checksum(SCENE[i].w, SCENE[i].h, |k| {
            // SAFETY: inside the frames this kernel allocated for client `i`'s surface, reached
            // through the direct map.
            unsafe { core::ptr::read_volatile((base + (k * 4) as u64) as *const u32) }
        })
    }

    /// The first twelve words of client `i`'s control page (everything the contract defines), read by
    /// the kernel through the direct map: a witness of what the compositor and the client did to
    /// that page, taken by a party neither of them controls.
    pub fn client_control_words(&self, i: usize) -> [u32; 12] {
        let base = mmu::phys_to_virt(self.client[i]);
        let mut out = [0u32; 12];
        for (k, w) in out.iter_mut().enumerate() {
            // SAFETY: inside the control frame this kernel allocated for client `i`.
            *w = unsafe { core::ptr::read_volatile((base + (k * 4) as u64) as *const u32) };
        }
        out
    }

    /// The composed screen, read by the kernel through the direct map.
    pub fn screen_pixel(&self, x: u32, y: u32) -> u32 {
        let at = mmu::phys_to_virt(self.screen) + (y * compositor::SCREEN_W + x) as u64 * 4;
        // SAFETY: inside the scanout frames, reached through the direct map.
        unsafe { core::ptr::read_volatile(at as *const u32) }
    }

    /// Write `v` into the screen at `(x, y)`: the poison a damage test needs, so that "the
    /// compositor did not touch this" is an observation rather than an inference.
    pub fn poison_screen_pixel(&self, x: u32, y: u32, v: u32) {
        let at = mmu::phys_to_virt(self.screen) + (y * compositor::SCREEN_W + x) as u64 * 4;
        // SAFETY: as above; the kernel owns these frames and no device is reading them in the
        // tests that poison (the display is a kernel stand-in there).
        unsafe { core::ptr::write_volatile(at as *mut u32, v) };
    }

    /// Which window the compositor says has focus, read out of the page it publishes. The
    /// compositor's decision, witnessed rather than asked for.
    pub fn focused(&self) -> u32 {
        let at = mmu::phys_to_virt(self.wlist) + compositor::proto::wlist::FOCUSED;
        // SAFETY: inside the window-list frame this kernel allocated.
        unsafe { core::ptr::read_volatile(at as *const u32) }
    }

    /// Play the input driver: put `bytes` in the ring and ring the doorbell.
    ///
    /// The authority exercised is the ring mapping, not the doorbell, which any client can ring.
    /// The CALL returns once the compositor has processed the frame, so a test needs no polling.
    pub fn type_bytes(&mut self, bytes: &[u8]) {
        let base = mmu::phys_to_virt(self.ring);
        for &b in bytes {
            let at = base
                + compositor::proto::ring::BYTES
                + (self.ring_tail % compositor::proto::ring::CAPACITY) as u64;
            // SAFETY: inside the ring frame this kernel allocated and shares with the compositor.
            unsafe { core::ptr::write_volatile(at as *mut u8, b) };
            self.ring_tail = self.ring_tail.wrapping_add(1);
        }
        // The bytes must be visible before the tail that advertises them.
        //
        // PAIR: `drain_input` in components/src/compositor.rs, which reads `TAIL` and then the
        // bytes, the single reader of every writer of the ring. The `ipc_call` below also orders
        // this producer on its own (the compositor is blocked in `receive_cap` on the doorbell);
        // see notes/memory-ordering.md.
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        // SAFETY: inside the ring frame.
        unsafe {
            core::ptr::write_volatile(
                (base + compositor::proto::ring::TAIL) as *mut u32,
                self.ring_tail,
            );
        };
        let w0 = compositor::proto::req(compositor::proto::COMMIT, 0);
        crate::sched::ipc_call(self.doorbell, [w0, 0]);
    }

    /// Ring the doorbell without typing anything: "look at the surfaces". Returns the reply's `r0`.
    ///
    /// The milestone tour uses it; the compositor tests all type something first.
    #[cfg_attr(any(test, feature = "system_tests"), allow(dead_code))]
    pub(crate) fn ring_doorbell(&self, operation: u64) -> u64 {
        crate::sched::ipc_call(self.doorbell, [compositor::proto::req(operation, 0), 0])[0]
    }

    /// Spawn a display terminal as window `i`, with exactly a window client's authority plus an
    /// output page an application writes `OPERATION_WRITE` bytes into (§10 (process model:
    /// capability-based, microkernel)).
    ///
    /// Its input endpoint is also the endpoint it serves, since it has one wait point (§33 (the
    /// compositor's authority is memory, not messages)). Returns the output page and that endpoint.
    /// Panics unless `i` is below `focusable`, since otherwise the terminal would hold no endpoint
    /// to serve. See notes/compositor/kernel-wiring.md.
    pub fn spawn_terminal(&self, i: usize) -> TermClient {
        assert!(
            i < self.focusable,
            "a display terminal must be focusable: its input endpoint is the endpoint it serves",
        );
        // At least one character cell, not a whole number of them: the terminal paints the strip
        // no cell covers as background on its first frame.
        assert!(
            SCENE[i].w >= video_terminal::CELL_W && SCENE[i].h >= video_terminal::CELL_H,
            "window {i} is too small for one character cell",
        );

        let out = crate::memory::alloc_zeroed()
            .expect("no output-page frame for a display terminal")
            .addr();

        let frames = SCENE[i].page_frames() as u64;
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; MAX_CLIENT_MAPS];
        let mut m = 0;
        maps[m] = Mapping {
            va: T_CTL_VA,
            phys: self.client[i],
            flags: Flags::user_data(),
        };
        m += 1;
        maps[m] = Mapping {
            va: T_OUT_VA,
            phys: out,
            flags: Flags::user_data(),
        };
        m += 1;
        for k in 0..frames {
            maps[m] = Mapping {
                va: T_SURFACE_VA + k * FRAME_SIZE,
                phys: self.client[i] + (1 + k) * FRAME_SIZE,
                flags: Flags::user_data(),
            };
            m += 1;
        }

        let grants = [
            rendezvous_cap(self.client_report[i], Rights::WRITE),
            rendezvous_cap(self.doorbell, Rights::WRITE),
            rendezvous_cap(self.input[i], Rights::READ),
        ];
        let image = self.term_image;
        crate::sched::spawn(move || {
            run(
                image,
                Spawn {
                    arg0: video_terminal::status::MODE_WINDOW,
                    arg1: 0,
                    arg2: 0,
                    grants: &grants,
                    maps: &maps[..m],
                },
            )
        })
        .expect("could not spawn a display terminal");

        TermClient {
            out,
            ep: self.input[i],
        }
    }
}

// A display terminal's address space. Must match components/src/display_terminal.rs, which uses
// these addresses in both `MODE_DISPLAY` and `MODE_WINDOW`, so they sit clear of the full scanout.
const T_SURFACE_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
const T_OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_0a00_0000);
const T_CTL_VA: u64 = address_space_map::pair_page(0x0000_0000_0a01_0000);

/// A display terminal running as a compositor client, from the spawner's side: the page it reads
/// an application's bytes out of, and the endpoint it serves.
pub struct TermClient {
    pub out: u64,
    pub ep: crate::sched::RendezvousId,
}

impl TermClient {
    /// Play the application: `OPERATION_WRITE` this text and return when it is on the screen.
    pub fn print(&self, text: &[u8]) {
        super::term_print(self.out, self.ep, text);
    }
}

/// The most mappings a compositor can need: the list, the ring, and every client's control page
/// and surface. The screen is a `PageFrame` grant instead.
const MAX_COMP_MAPS: usize = 2 + compositor::MAX_WINDOWS * (1 + MAX_SURFACE_PAGES);
/// The most page frames a client's surface may take: one `Mapping` each, beside its control page.
/// The tables are sized from it, and they must fit the 1 KiB a spawn closure may capture
/// (`kernel/src/thread.rs` asserts that), so a window too big for it is refused below, at build
/// time, rather than as an index out of bounds when the compositor is wired.
const MAX_SURFACE_PAGES: usize = 4;
const _: () = {
    let mut i = 0;
    while i < compositor::SCENE.len() {
        assert!(
            compositor::SCENE[i].page_frames() as usize <= MAX_SURFACE_PAGES,
            "a SCENE window's surface needs more page frames than MAX_SURFACE_PAGES",
        );
        i += 1;
    }
};
/// The most a client can need: its control page, its surface, and (capture only) the window list.
/// A capture client maps the screen itself, out of its `SCREEN_FRAME` and `BUDGET` grants.
const MAX_CLIENT_MAPS: usize = 1 + MAX_SURFACE_PAGES + 1;
/// Report, display, doorbell, the screen `PageFrame`, its map budget, then one input endpoint per
/// focusable client starting at [`COMP_INPUT_BASE`].
const MAX_COMP_GRANTS: usize = COMP_INPUT_BASE as usize + compositor::MAX_WINDOWS;
/// The first of the compositor's per-client input-endpoint grant slots. Must match
/// `components/src/compositor.rs`'s own `INPUT` constant.
const COMP_INPUT_BASE: u64 = 5;

/// A fresh zeroed frame, for a page the kernel hands two processes to share.
fn zeroed_page_frame() -> u64 {
    crate::memory::alloc_zeroed()
        .expect("no frame for the compositor's shared pages")
        .addr()
}
