//! The `std` program spawn every `start_std*` in [`super`] shares: the heap, the stack, the shared
//! file page, and the argument and clock pages when the caller gives them. Split out of
//! `fs_service.rs` along this seam by milestone 835 (a C library, stage 1: files, clock and memory)
//! to keep that file under its §266 (a Rust source file stays under 2,000 lines) ceiling.

use super::*;

/// The spawn both of the above share: a std program whose directory is whatever `file_ep` serves,
/// with the file page it shares mapped where the PAL expects it, writing to `report`. Returns the
/// heap's region, the thread, the stack frames, and the argument page if `line` gave one; the
/// process owns none of the frames.
///
/// `line` is assembled onto a fresh page by `grant_plan::argv` and handed over the way the
/// progenitor hands it (`crates/system_initializer`'s spawn): a `READ` page frame capability at
/// `ARGS_SLOT`, the page mapped read-only at `ARGS_PAGE`. `None` leaves the slot empty, which the
/// PAL reads as no arguments at all.
///
/// BUGS: [`super::start_std_full`] drops the stack frames, so every program it spawns keeps its 32
/// for the boot. That was true before this function was split out of it and is left as it was: its
/// `std_exerciser` is spawned once and the ledger already carries it.
pub(super) fn spawn_std(
    file_ep: RendezvousId,
    badge: u64,
    file_shared: u64,
    report: RendezvousId,
    std_image: &'static [u8],
    line: Option<&[u8]>,
    clock_page: Option<u64>,
) -> (
    u64,
    crate::thread::ThreadId,
    [u64; STD_FS_STACK_PAGES as usize],
    Option<u64>,
) {
    let heap =
        crate::memory_region::create(STD_FS_HEAP_PAGES).expect("no untyped for the std fs heap");
    let args = line.map(|line| {
        let phys = page_frame();
        // SAFETY: a frame just allocated and zeroed, reached through the direct map, which nothing
        // else holds until the program below is given it read-only.
        let page = unsafe { &mut *(mmu::phys_to_virt(phys) as *mut [u8; FRAME_SIZE as usize]) };
        grant_plan::argv(line, page).expect("the harness's own command line is not an argv");
        phys
    });

    // The shared file page, then the deep stack std needs. `run` maps one stack page; std's
    // startup and formatting overflow it immediately, the same reason the other std spawns map
    // extra pages below it.
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; 3 + STD_FS_STACK_PAGES as usize];
    maps[0] = Mapping {
        va: FS_PAGE_STD,
        phys: file_shared,
        flags: Flags::user_data(),
    };
    let mut stack = [0u64; STD_FS_STACK_PAGES as usize];
    for ((k, m), phys) in maps[1..=STD_FS_STACK_PAGES as usize]
        .iter_mut()
        .enumerate()
        .zip(stack.iter_mut())
    {
        m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
        m.phys = page_frame();
        *phys = m.phys;
    }
    let mut maps_used = 1 + STD_FS_STACK_PAGES as usize;
    if let Some(phys) = args {
        maps[maps_used] = Mapping {
            va: std_runtime_protocol::ARGS_PAGE,
            phys,
            flags: Flags::user_rodata(),
        };
        maps_used += 1;
    }
    // The wall clock, when the caller has one to give: the clock service's page, read-only, where
    // the std PAL reads it (milestone 835, for a C program that times itself). A READER, and the
    // mapping is what says so, as `std_service::spawn_std` grants it.
    if let Some(phys) = clock_page {
        maps[maps_used] = Mapping {
            va: std_runtime_protocol::CLOCK_PAGE,
            phys,
            flags: Flags::user_rodata(),
        };
        maps_used += 1;
    }

    let tid = crate::sched::spawn(move || {
        // The directory capability goes in at its named slot BEFORE `run` grants in order, so
        // `run`'s two grants land at 0 and 1 and slots 2 and 3 stay empty. See `grant_at`.
        // A bound grant (milestone 606, ruling D) is the FS server's own endpoint carrying the
        // badge the grant was bound to; every other spawn holds an unbadged one.
        let dir = if badge == 0 {
            rendezvous_cap(file_ep, Rights::WRITE)
        } else {
            rendezvous_cap_badged(file_ep, Rights::WRITE, badge)
        };
        crate::sched::grant_at(FS_DIR_SLOT, dir).expect("the std fs slot was already occupied");
        // The argument page and the clock page, each a READ page at its named slot when given.
        let pages = [
            (std_runtime_protocol::ARGS_SLOT, args),
            (std_runtime_protocol::CLOCK_SLOT, clock_page),
        ];
        for (slot, phys) in pages {
            if let Some(phys) = phys {
                crate::sched::grant_at(slot, page_frame_cap(phys, Rights::READ))
                    .expect("a std page slot was already occupied");
            }
        }
        run(
            std_image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    memory_region_cap(heap),               // slot 0: the heap's budget
                    rendezvous_cap(report, Rights::WRITE), // slot 1: stdout/stderr
                ],
                maps: &maps[..maps_used],
            },
        )
    })
    .expect("could not spawn the std fs program");
    (heap, tid, stack, args)
}
