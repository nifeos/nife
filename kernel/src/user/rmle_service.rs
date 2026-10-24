use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::{self, RendezvousId};
use crate::user::holding::Holding;

/// The VAs `components/src/rmle.rs` hardcodes. Must match that file.
const TERM_OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_0080_0000);
const FS_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);

/// Stack pages beyond the one `run` maps. `rmle`'s `Editor` is a few kilobytes (32 rows of 100
/// bytes plus bookkeeping) and it has no allocator, so it all lives on the stack, with margin.
const RMLE_STACK_PAGES: usize = 4;

/// The file channel's width, straight from the contract `rmle`'s FS calls speak, the source
/// `fs_service::FILE_PAGES` is defined from.
const FILE_PAGES: usize = filesystem_contract::fs::TRANSFER_PAGES;

/// A fresh, zeroed frame ([`crate::memory::alloc_zeroed`]) for `rmle`'s extra stack pages.
fn page_frame() -> u64 {
    crate::memory::alloc_zeroed()
        .expect("no frame for rmle's stack")
        .addr()
}

/// A running `rmle`, wired with a real terminal ([`raw_mode_service::start`]'s wiring) and a real
/// filesystem (a directory narrowed under `fs_subtree_caretaker`, [`fs_service::narrow_dir`]'s
/// wiring). Composing the two is all this module adds.
pub struct Wiring {
    /// The terminal endpoint, played by the fake console + real `line_editor` underneath. A test
    /// wanting to observe the screen reads [`Wiring::term_out_phys`] after driving keystrokes
    /// through the same input-driver role [`raw_mode_service`]'s own tests use.
    pub term: RendezvousId,
    pub term_out_phys: u64,
    /// The narrowed directory `rmle` holds. A test may call it directly once `rmle` has exited,
    /// to read the file back as an independent witness.
    pub dir: RendezvousId,
    pub file_shared: u64,
    /// `rmle`'s one closing report: `(STATUS_QUIT | STATUS_OPEN_FAILED, dirty-or-errno, 0)`.
    pub report: RendezvousId,
}

/// Spawn `rmle` against `dir_name` (a directory already in the test fixture tree, granted with
/// [`filesystem_contract::dir::ALL`]) and `file_name` (the file inside it `rmle` edits, created if
/// absent). `None` if there is no RedoxFS disk attached to this run.
///
/// Returns a [`Holding`] the caller must release, for [`raw_mode_service::start`]'s reason: the
/// fake console and `line_editor` block forever. `rmle` is not in it; the test drives it to a clean
/// exit over `^Q`.
///
/// BUGS: when the filesystem half refuses, the terminal half's two threads are already spawned and
/// nothing releases them. It is unreachable in a run with a RedoxFS server.
pub fn start(dir_name: &'static str, file_name: &str) -> Option<(Wiring, Holding)> {
    let image = program("rmle").expect("no rmle program in the initial ramdisk");

    let TerminalHalf {
        ep_region,
        term,
        term_out_phys,
        console_tid,
        line_editor_tid,
    } = start_terminal_half();

    // `narrow_dir_held`'s holding ends the caretaker and returns its endpoints, `rmle`'s report
    // among them; the terminal half joins it below.
    let (dir_ep, file_shared, report, mut held) = fs_service::narrow_dir_held(
        fs_service::blk_server_image(),
        fs_service::fs_server_image()?,
        program("fs_subtree_caretaker").expect("no fs_subtree_caretaker in the initial ramdisk"),
        dir_name,
        filesystem_contract::dir::ALL,
    )?;

    spawn_rmle(
        image,
        file_name,
        term,
        term_out_phys,
        dir_ep,
        file_shared,
        report,
    );

    held.add_thread(console_tid);
    held.add_thread(line_editor_tid);
    held.add_region(ep_region);

    Some((
        Wiring {
            term,
            term_out_phys,
            dir: dir_ep,
            file_shared,
            report,
        },
        held,
    ))
}

/// What the terminal half leaves running under `rmle`: the endpoint region, the TERM endpoint,
/// `rmle`'s terminal-output page, and the two threads a [`Holding`] must reclaim.
struct TerminalHalf {
    ep_region: u64,
    term: RendezvousId,
    term_out_phys: u64,
    console_tid: crate::thread::ThreadId,
    line_editor_tid: crate::thread::ThreadId,
}

/// Mint the terminal's endpoints and pages, and spawn the fake console and `line_editor`.
fn start_terminal_half() -> TerminalHalf {
    // raw_mode_service::start's terminal, duplicated: reusing it would mean splitting it as
    // fs_service::narrow_dir is split from start_granted_dir.
    let term_img = program("line_editor").expect("no line_editor program in the initial ramdisk");
    // One page per endpoint and one spare, so reclaiming the region wakes both threads to die.
    let ep_region = crate::memory_region::create(4).expect("no endpoint region for rmle_service");
    let term = sched::create_rendezvous_from(ep_region).expect("no TERM endpoint");
    let conreq = sched::create_rendezvous_from(ep_region).expect("no CONREQ endpoint");
    let conrep = sched::create_rendezvous_from(ep_region).expect("no CONREP endpoint");
    let console_phys = crate::memory::alloc_zeroed()
        .expect("no frame for the fake console")
        .addr();
    let term_out_phys = crate::memory::alloc_zeroed()
        .expect("no frame for rmle's terminal-output page")
        .addr();
    let term_app_in_phys = crate::memory::alloc_zeroed()
        .expect("no frame for line_editor's unused app-input page")
        .addr();
    let console_tid = sched::spawn(move || {
        loop {
            sched::ipc_receive(conreq);
            sched::ipc_send(conrep, [0, 0, 0]);
        }
    })
    .expect("could not spawn the fake console");
    let line_editor_tid = sched::spawn(move || {
        run(
            term_img,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(term, Rights::READ),
                    rendezvous_cap(conreq, Rights::WRITE),
                    rendezvous_cap(conrep, Rights::READ),
                ],
                maps: &[
                    Mapping {
                        va: 0x0060_0000, // line_editor's own CONOUT_VA
                        phys: console_phys,
                        flags: Flags::user_data(),
                    },
                    Mapping {
                        va: 0x0080_0000, // line_editor's own APP_OUT_VA
                        phys: term_out_phys,
                        flags: Flags::user_rodata(),
                    },
                    Mapping {
                        va: 0x0090_0000, // line_editor's own APP_IN_VA
                        phys: term_app_in_phys,
                        flags: Flags::user_data(),
                    },
                ],
            },
        )
    })
    .expect("could not spawn line_editor");
    TerminalHalf {
        ep_region,
        term,
        term_out_phys,
        console_tid,
        line_editor_tid,
    }
}

/// Pack `file_name` into `rmle`'s argument words and spawn it with its three grants; returns the
/// endpoint its closing report arrives on.
fn spawn_rmle(
    image: &'static [u8],
    file_name: &str,
    term: RendezvousId,
    term_out_phys: u64,
    dir_ep: RendezvousId,
    file_shared: u64,
    report: RendezvousId,
) {
    let (lo, hi) = filesystem_contract::grant::pack_name(file_name.as_bytes())
        .expect("rmle's target name rides in two argument words; this one does not fit");
    let spec = filesystem_contract::grant::spec(file_name.len(), 0);

    sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; FILE_PAGES + RMLE_STACK_PAGES + 1];
        maps[0] = Mapping {
            va: TERM_OUT_VA,
            phys: term_out_phys,
            flags: Flags::user_data(),
        };
        let n = 1 + fs_service::map_channel(&mut maps[1..], FS_VA, file_shared, FILE_PAGES);
        for (k, m) in maps[n..n + RMLE_STACK_PAGES].iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = page_frame();
        }
        run(
            image,
            Spawn {
                arg0: spec,
                arg1: lo,
                arg2: hi,
                grants: &[
                    rendezvous_cap(term, Rights::WRITE),   // slot 0: the terminal
                    rendezvous_cap(dir_ep, Rights::WRITE), // slot 1: the directory
                    rendezvous_cap(report, Rights::WRITE), // slot 2: the closing report
                ],
                maps: &maps[..n + RMLE_STACK_PAGES],
            },
        )
    })
    .expect("could not spawn rmle");
}
