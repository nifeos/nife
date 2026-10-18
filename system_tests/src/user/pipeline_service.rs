use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::*;
use crate::cap::{
    Capability, Rights, memory_region_lending_capability, page_frame_cap, rendezvous_cap,
};
use crate::sched::RendezvousId;

/// The `swish` binary's pipeline role (`components/src/swish.rs`).
const ROLE_PIPELINE: u64 = 3;

/// The VAs the shell hardcodes for its terminal pages. Must match components/src/swish.rs.
const OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_00c0_0000);
const LINE_VA: u64 = address_space_map::pair_page(0x0000_0000_00b0_0000);

/// The budget the shell mints its pipes out of. Each pipeline splits a region off this and
/// gives it back, so one number covers a script of several lines; it matches `system_initializer`'s grant.
const SH_BUDGET_PAGES: u64 = 128;

/// Pages of stack **below** the one `run` maps. A shell needs more than a hand-sized program
/// for `spawn_fs_client`'s measured reason, and milestone 50 added an array of planned
/// endowments to what it carries by value.
///
/// **Eleven since DECISIONS §67**, which is the fourth time this symptom has been a stack page
/// short and the second time it has been `run_pipeline`'s frame. `2>` put a **second** `FileOut`
/// there, and a `FileOut` carries a 256-byte staging buffer by value because the filesystem's write
/// unit is a page and the sink contract's is sixteen bytes. Seven pages overflowed by twenty-four
/// bytes: a data abort one word below the lowest mapped page, mid-script, `far` just under `sp`.
///
/// Four extra pages rather than one, deliberately. Every previous instance bought exactly enough and
/// the next change found the wall again; 16 KiB of address space is not worth a fifth bisect. The
/// number must stay level with `system_initializer`'s `CHILD_STACK_PAGES`, because a test wiring with
/// less headroom than the boot wiring finds faults the boot does not have, which is a bug in the
/// harness rather than a signal (notes/pipes/the-boot.md).
const SHELL_EXTRA_STACK: usize = 11;

/// What the kernel holds of a scripted shell.
pub struct Wiring {
    /// The terminal rendezvous the shell `CALL`s. The test serves it.
    pub term: RendezvousId,
    /// Where the shell's printed bytes land, so the test can read them out.
    pub out_phys: u64,
}

/// The collected transcript. A static because the terminal service and the assertions are the
/// same thread and the buffer outlives one call; sized for the whole script with room to spare,
/// so a transcript that ran long is truncated rather than overrunning.
///
/// **Eight kilobytes since milestone 67**, which folded that milestone's script onto the
/// redirection witness rather than wiring a seventh shell: every scripted shell here is a live
/// process whose frames are never reclaimed, and one more put `time_tests` over the frame pool
/// intermittently. Kernel `.bss` costs no frames, so the longer script is the cheap half of that
/// trade.
static TRANSCRIPT: spin::Mutex<[u8; 8192]> = spin::Mutex::new([0; 8192]);
static WRITTEN: AtomicUsize = AtomicUsize::new(0);

/// Set while a scripted shell's [`transcript`] is being collected; [`watch_for_a_killed_stage`]
/// runs while it is.
static SCRIPT_RUNNING: AtomicBool = AtomicBool::new(false);

/// **Fail the test the moment a user thread dies under a scripted shell**, instead of a minute later
/// as a watchdog "lost-wakeup hang".
///
/// The real boot reports a killed job to the shell: `job_undertaker` sends `JOB_FAULTED` on the
/// result endpoint, the shell prints its faulted sentence and destroys the pipe, and the rest of
/// the line ends. [`init_service`] has no undertaker, so here the line's other stages and the shell
/// wait forever on a stage that will never write (merge-group run 38506029610: `date` killed in
/// `date | wc`, then sixty seconds of every thread blocked). No script here expects a fault, because
/// this stub could never have reported one, so any rise in `USER_FAULTS` while a script runs is
/// the failure, and the kill report above this panic names the thread.
///
/// A `yield_now` loop, the shape `wait_for` uses, because the kernel has no timed wait a test can
/// take. It ends when the transcript is collected, or after `GIVE_UP_SECONDS` if nobody collects
/// it, so it never outlives the script into a test that faults on purpose.
fn watch_for_a_killed_stage() {
    use crate::arch::exceptions::USER_FAULTS;
    const GIVE_UP_SECONDS: u64 = 300;
    let before = USER_FAULTS.load(Ordering::Acquire);
    let deadline = crate::arch::timer::now() + GIVE_UP_SECONDS * crate::arch::timer::frequency();
    while SCRIPT_RUNNING.load(Ordering::Acquire) && crate::arch::timer::now() < deadline {
        if USER_FAULTS.load(Ordering::Acquire) != before {
            // The count rises before the kill report prints, on the dying thread's core. A fifth of
            // a second lets the report finish, so the two do not interleave on the console.
            let quiet = crate::arch::timer::now() + crate::arch::timer::frequency() / 5;
            while crate::arch::timer::now() < quiet {
                crate::sched::yield_now();
            }
            panic!(
                "a user thread was killed while a scripted shell ran (its kill report is above). \
                 This stub progenitor has no undertaker to tell the shell, so the line would hang."
            );
        }
        crate::sched::yield_now();
    }
}

/// **Wire a scripted shell and the progenitor service behind it.**
///
/// The endowment is deliberately the interactive one, slot for slot, because a witness with a
/// wider endowment would be proving something about a shell nobody runs.
pub fn start() -> Option<Wiring> {
    start_with(ROLE_PIPELINE, 0, None, None)
}

/// **The same shell, told to time what it runs** (milestone 86): [`start`]'s four slots plus, when
/// `clock` is `Some`, a `PageFrame` with **`READ`** at slot 4 and a read-only mapping of it.
///
/// The three states this can be started in are the three the shell has to answer for, and they are
/// three capability tables rather than three code paths:
///
/// - `Some(a published page)`: `time` reports a duration.
/// - `Some(a blank page)`: the frame is there and reads `UNKNOWN`, which is what a reader on a
///   machine with no believable RTC holds.
/// - `None`: no capability at all, so no mapping either, and the shell has to answer without
///   touching the address a clock would have been at.
///
/// `READ` and no `GRANT`, matching what the progenitor hands the shell: a witness with wider rights than
/// the boot wiring would be proving something about a shell nobody runs.
pub fn start_timing(clock: Option<u64>) -> Option<Wiring> {
    start_with(ROLE_TIMING, 0, None, clock)
}

/// The shell's timing role (`components/src/swish.rs`).
const ROLE_TIMING: u64 = 5;

/// Where the shell maps its own clock page, read-only. Must match `components/src/swish.rs`'s
/// `SH_CLOCK_VA` and `crates/system_initializer`'s. **Not** `date`'s `CLOCK_VA`: that is where a
/// *child* maps one, and the shell already has the terminal's output frame at that address.
const SH_CLOCK_VA: u64 = address_space_map::pair_page(0x0000_0000_00d0_0000);

/// **The same shell, one capability wider**: a directory at slot 4 and the page it shares with
/// the FS server (milestone 50's `>` and `<`).
///
/// This is what makes the pair of witnesses worth having. [`start`] and this call spawn the same
/// ELF from the same archive with the same four slots, and the only difference between a
/// `date > report.txt` that is refused and one that writes a file is whether slot 4 is
/// occupied. Neither behaviour is a branch in the program; both are facts about a capability table.
///
/// `dir` is `(the narrowed directory rendezvous, the physical frame it shares with the FS server)`,
/// which is what `fs_service::narrow_dir` hands back.
pub fn start_redirecting(dir: (RendezvousId, u64), rights: u64) -> Option<Wiring> {
    start_with(ROLE_REDIRECT, rights, Some(dir), None)
}

/// The shell's redirection role (`components/src/swish.rs`).
const ROLE_REDIRECT: u64 = 4;

/// Where an FS client maps the page it shares with the FS server (`fs_service`'s
/// `FILE_VA_CLIENT`, and `components/src/swish.rs`'s `FS_VA`).
const FS_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);

fn start_with(
    role: u64,
    arg: u64,
    dir: Option<(RendezvousId, u64)>,
    clock: Option<u64>,
) -> Option<Wiring> {
    let Some(image) = program("swish") else {
        crate::println!("start_with: no swish in the archive");
        return None;
    };
    // **Kept for the boot, deliberately.** `init_service` below is a kernel thread that loops on
    // `spawn_ep` forever, and nothing ends it, so reclaiming these would turn it into a busy loop
    // on a stale endpoint. They come from a region of their own so the three pages are charged to
    // the test that called this, rather than from the kernel's pool, whose 32-page chunk is charged
    // to whichever later test crosses its boundary.
    let Some(endpoints) = crate::memory_region::create(3) else {
        crate::println!("start_with: memory_region::create(3) for the endpoints refused");
        return None;
    };
    let term = crate::sched::create_rendezvous_from(endpoints).expect("no terminal rendezvous");
    let spawn_ep = crate::sched::create_rendezvous_from(endpoints).expect("no spawn rendezvous");
    let result = crate::sched::create_rendezvous_from(endpoints).expect("no result rendezvous");
    let Some(budget) = crate::memory_region::create(SH_BUDGET_PAGES) else {
        crate::println!("start_with: memory_region::create({SH_BUDGET_PAGES}) refused");
        return None;
    };
    // Both zeroed, so neither page starts with somebody else's bytes in it.
    let out_phys = match crate::memory::alloc_zeroed() {
        Some(f) => f,
        None => {
            crate::println!("start_with: memory::alloc_zeroed (out) refused");
            return None;
        }
    }
    .addr();
    let line_phys = match crate::memory::alloc_zeroed() {
        Some(f) => f,
        None => {
            crate::println!("start_with: memory::alloc_zeroed (line) refused");
            return None;
        }
    }
    .addr();
    WRITTEN.store(0, Ordering::SeqCst);
    SCRIPT_RUNNING.store(true, Ordering::Release);
    if crate::sched::spawn(watch_for_a_killed_stage).is_none() {
        crate::println!("start_with: spawn(watch_for_a_killed_stage) refused");
        return None;
    }

    // The progenitor first, so a shell that spawns before the service is listening merely blocks in a
    // rendezvous rather than failing.
    if crate::sched::spawn(move || init_service(spawn_ep, result)).is_none() {
        crate::println!("start_with: spawn(init_service) refused");
        return None;
    }

    let shell = crate::sched::spawn(move || {
        let (maps, n) = shell_mappings(out_phys, line_phys, dir, clock);
        let (grants, g, clock_slot) = shell_grants(term, spawn_ep, result, budget, dir, clock);
        run(
            image,
            Spawn {
                arg0: role,
                arg1: arg,
                arg2: clock_slot,
                grants: &grants[..g],
                maps: &maps[..n],
            },
        )
    });

    // **Say why it was refused, instead of handing the caller a bare `None`.**
    //
    // This was `})?;` until 2026-08-27, and the silence cost a whole investigation. The only
    // thing that can refuse this spawn is `sched::MAX_THREADS`, and when the aarch64 suite
    // reached that ceiling here the test above only ever learned that it had not got a shell.
    // Same silently-`?`-shaped bug `fs_service::spawn_fs_client` had, and the same fix.
    //
    // `dump_threads` goes with the line rather than after it because the table's contents are
    // the whole diagnosis: a leak and a boot that legitimately wants more threads than the
    // image allows are indistinguishable from the refusal alone, and telling them apart is
    // what took two full instrumented runs the first time. See `sched::MAX_THREADS`, whose doc
    // comment carries the measurement and the coupled constants.
    if shell.is_none() {
        crate::println!(
            "start_with: spawn(swish) refused; the thread table is full ({} of {} live). \
             See sched::MAX_THREADS.",
            crate::sched::thread_count(),
            crate::sched::MAX_THREADS,
        );
        crate::sched::dump_threads();
        return None;
    }

    Some(Wiring { term, out_phys })
}

/// The shell's mappings: its output and line pages, its extra stack, then the FS page and the
/// clock page when this wiring has them. Returns the array and how many entries are in use.
fn shell_mappings(
    out_phys: u64,
    line_phys: u64,
    dir: Option<(RendezvousId, u64)>,
    clock: Option<u64>,
) -> ([Mapping; 4 + SHELL_EXTRA_STACK], usize) {
    // The extra stack goes in as ordinary mappings below the one `run` maps, which is the
    // shape `spawn_fs_client` uses and for the same measured reason: a shell carries a path
    // stack, a parsed line and now an array of planned endowments, all by value.
    let mut maps = [Mapping {
        va: OUT_VA,
        phys: out_phys,
        flags: Flags::user_data(),
    }; 4 + SHELL_EXTRA_STACK];
    maps[1] = Mapping {
        va: LINE_VA,
        phys: line_phys,
        flags: Flags::user_rodata(),
    };
    for (k, m) in maps[2..2 + SHELL_EXTRA_STACK].iter_mut().enumerate() {
        m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
        m.phys = crate::memory::alloc()
            .expect("no frame for the shell's stack")
            .addr();
        m.flags = Flags::user_data();
    }
    // The optional pages last, so a shell wired without either maps exactly what it did before
    // and the count is the only difference between the wirings.
    let mut n = 2 + SHELL_EXTRA_STACK;
    if let Some((_, file_shared)) = dir {
        maps[n] = Mapping {
            va: FS_VA,
            phys: file_shared,
            flags: Flags::user_data(),
        };
        n += 1;
    }
    if let Some(phys) = clock {
        // **`user_rodata`, and that is the read rung of DECISIONS §43 rather than tidiness**: a
        // shell that could write this page could set the wall clock, and the whole argument for
        // it holding one at all is that reading is not that.
        maps[n] = Mapping {
            va: SH_CLOCK_VA,
            phys,
            flags: Flags::user_rodata(),
        };
        n += 1;
    }
    (maps, n)
}

/// The shell's capability table, in slot order. Returns the array, how many slots are in use,
/// and the clock's slot (0 for none).
fn shell_grants(
    term: RendezvousId,
    spawn_ep: RendezvousId,
    result: RendezvousId,
    budget: u64,
    dir: Option<(RendezvousId, u64)>,
    clock: Option<u64>,
) -> ([Capability; 6], usize, u64) {
    // Filled in the order the shell's own constants name, because a `Spawn` fills a capability table from
    // zero: slot 0 the terminal, 1 the spawn channel, 2 the result channel, 3 the budget, then
    // whichever of the directory and the clock this wiring has.
    let mut grants = [rendezvous_cap(term, Rights::WRITE); 6];
    grants[1] = rendezvous_cap(spawn_ep, Rights::WRITE);
    grants[2] = rendezvous_cap(result, Rights::READ);
    grants[3] = memory_region_lending_capability(budget);
    let mut g = 4;
    if let Some((dir_ep, _)) = dir {
        grants[g] = rendezvous_cap(dir_ep, Rights::WRITE);
        g += 1;
    }
    // **The slot the clock landed in, handed over in `arg2`**, which is the same thing both
    // inits do and for the same reason: the number moves with what else this wiring granted, and
    // a shell that guessed would probe some other object and map a page that is not a clock.
    // Zero means none, and zero can never be a clock because slot 0 is the terminal everywhere.
    let mut clock_slot = 0u64;
    if let Some(phys) = clock {
        grants[g] = page_frame_cap(phys, Rights::READ);
        clock_slot = g as u64;
        g += 1;
    }
    (grants, g, clock_slot)
}

/// **Serve the terminal until the shell says it is finished**, collecting everything it printed.
///
/// One `OPERATION_WRITE` at a time, replied the way the real line discipline replies (the byte count),
/// because the shell blocks on that reply and a test that answered differently would be testing
/// a terminal nobody has.
pub fn transcript(w: &Wiring, sentinel: &[u8], out: &mut [u8]) -> usize {
    loop {
        let m = crate::sched::ipc_receive_cap(w.term);
        let (w0, slot) = (m[0], m[1]);
        let crate::cap::Object::Reply(caller) = crate::sched::current_cap(slot)
            .expect("the shell's terminal write carried no reply capability")
            .object
        else {
            panic!("the shell sent the terminal something that was not a CALL");
        };
        let n = line_editor::proto::len(w0);
        if line_editor::proto::operation(w0) == line_editor::proto::OPERATION_WRITE {
            let mut buf = TRANSCRIPT.lock();
            let at = WRITTEN.load(Ordering::SeqCst);
            let n = n.min(buf.len().saturating_sub(at));
            for i in 0..n {
                // SAFETY: the shell's output page, mapped read/write into it and named here
                // through the direct map; `n` is bounded by the page and by the buffer.
                buf[at + i] = unsafe {
                    core::ptr::read_volatile(
                        (mmu::phys_to_virt(w.out_phys) + i as u64) as *const u8,
                    )
                };
            }
            WRITTEN.store(at + n, Ordering::SeqCst);
        }
        crate::sched::ipc_reply(caller, [n as u64, 0]);
        crate::sched::delete_current_cap(slot).expect("consume the one-shot reply");

        let done = {
            let buf = TRANSCRIPT.lock();
            let len = WRITTEN.load(Ordering::SeqCst);
            len >= sentinel.len() && buf[len - sentinel.len()..len] == *sentinel
        };
        if done {
            SCRIPT_RUNNING.store(false, Ordering::Release);
            let buf = TRANSCRIPT.lock();
            let len = WRITTEN.load(Ordering::SeqCst).min(out.len());
            out[..len].copy_from_slice(&buf[..len]);
            return len;
        }
    }
}

/// **The text the shell printed in response to `line`**, up to but not including the next
/// prompt.
///
/// The transcript is echoed prompts and answers, so slicing it this way is what lets an
/// assertion name the command it is about instead of counting lines. It lives here rather than
/// in one test module because both witnesses (the pipeline script and the redirection script)
/// read their transcripts the same way, and a second copy would be a second thing to keep true.
pub fn answer<'a>(t: &'a [u8], line: &[u8]) -> &'a [u8] {
    let mut needle = [0u8; 64];
    needle[0] = b'$';
    needle[1] = b' ';
    needle[2..2 + line.len()].copy_from_slice(line);
    needle[2 + line.len()] = b'\n';
    let needle = &needle[..3 + line.len()];
    let start = t
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap_or_else(|| {
            panic!(
                "the shell never ran {:?}. transcript:\n{}",
                core::str::from_utf8(line).unwrap(),
                core::str::from_utf8(t).unwrap_or("<not utf-8>"),
            )
        })
        + needle.len();
    let rest = &t[start..];
    let end = rest
        .windows(2)
        .position(|w| w == b"$ ")
        .unwrap_or(rest.len());
    &rest[..end]
}

/// Parse `wc`'s three numbers out of what the shell printed for it. `wc` prints
/// `lines words bytes` and a newline; the caller strips the shell's two-space prefix.
pub fn counts(said: &[u8]) -> (u64, u64, u64) {
    let text = core::str::from_utf8(said).expect("wc printed non-UTF-8");
    let mut it = text.split_ascii_whitespace().map(|w| {
        w.parse::<u64>()
            .unwrap_or_else(|_| panic!("wc printed {text:?}, which is not three numbers"))
    });
    let (l, w, b) = (
        it.next().expect("no line count"),
        it.next().expect("no word count"),
        it.next().expect("no byte count"),
    );
    assert!(
        it.next().is_none(),
        "wc printed more than three numbers: {text:?}"
    );
    (l, w, b)
}

/// **The progenitor, as the shell sees it**: the spawn protocol, including milestone 50's two delegated
/// capabilities.
///
/// The whole of what the operators added is here, and it is small: receive an rendezvous, and put
/// it where the result rendezvous would have gone. Nothing decides what is behind it, because
/// nothing here can find out.
///
/// **What this fallback does not implement**: a directory grant (`Wiring::dir`, milestone 31 phase
/// 3) and a screen-narrowed tail's completion rendezvous (`Wiring::screen`, DECISIONS §106). Both
/// need a capability this stub has nothing behind (a file service to attenuate, a fault target to
/// install), so neither bit is drained off the wire here, and a line that set one would desync the
/// two sides' shared count of delegated capabilities. No guest test routes either shape through
/// this path today; both are exercised only against the real progenitor, by `script/swish-check`.
fn init_service(spawn_ep: RendezvousId, result: RendezvousId) -> ! {
    loop {
        let m = crate::sched::ipc_receive(spawn_ep);
        let (w0, w1, w2) = (m[0], m[1], m[2]);
        let prog = grant_plan::Prog::from_id(grant_plan::spawnproto::prog_id(w0));
        let arg = grant_plan::spawnproto::program_argument(w1);
        let wiring = grant_plan::spawnproto::wiring(w2);

        // In the protocol's order. A capability received but never expected, or expected and
        // never sent, deadlocks both sides, which is why the order is the contract.
        let sink = if wiring.sink {
            take_rendezvous(spawn_ep)
        } else {
            None
        };
        let source = if wiring.source {
            take_rendezvous(spawn_ep)
        } else {
            None
        };
        // **A declared second stream is received and closed on the child's behalf** (DECISIONS
        // §67). This wiring grants a child its ordinary slots and nothing at a named one (a `Spawn`
        // fills from zero and has no way to name slot eight), so `date` here finds an empty
        // diagnostic slot and says what it has to say in-band, exactly as it did before the second
        // stream existed. **So this is the fallback path under test**, and the real one is
        // `script/swish-check`, which runs `user/src/system_initializer.rs`.
        //
        // Receiving the capability is what keeps the two sides in lockstep, and closing the stream
        // is what stops the shell waiting for an end-of-stream from a program that was never handed
        // the rendezvous. The close goes on **its own thread**, and that is not tidiness: the shell
        // drains diagnostics only after every stage of the line is spawned, so a blocking send here
        // would still be waiting when the shell sent the next stage's request, and both would stop.
        if wiring.diagnostics
            && let Some(ep) = take_rendezvous(spawn_ep)
        {
            crate::sched::spawn(move || {
                crate::sched::ipc_send(ep, [byte_sink_contract::eof(), 0, 0]);
            });
        }
        // A `--mem` grant is received and dropped: no stage of the pipeline script asks for
        // one, and receiving it anyway is what keeps the two sides in lockstep if one ever
        // does. `user/src/system_initializer.rs` is where a budget actually reaches a child.
        if grant_plan::spawnproto::memory_grant_pages(w2) > 0 {
            let m = crate::sched::ipc_receive_cap(spawn_ep);
            let _ = crate::sched::delete_current_cap(m[1]);
        }

        let image = prog.and_then(|p| program(p.name()));
        let started = match image {
            Some(image) => {
                // Slot 0 is the output: the result rendezvous, or the sink the shell delegated.
                // Slot 1 is the input source when there is one. That is the whole difference.
                let out = sink.unwrap_or(result);
                // **The argument goes in `arg1`, not `arg0`**, which is where both real inits put
                // it (`thread_control_block_start(tcb, 0, arg, 0)`) and where `components/src/least_authority_demo.rs` reads it. This
                // said `arg0` until milestone 86, and nothing failed, because no line in the
                // pipeline or redirection scripts ever spawned a program that takes one: `date`,
                // `wc` and `echo` all take none, and `least_authority_demo 9 | wc` is refused at the prompt. The
                // first script to type `time least_authority_demo 3` got `3*3 = 0` back.
                crate::sched::spawn(move || match source {
                    Some(src) => run(
                        image,
                        Spawn {
                            arg0: 0,
                            arg1: arg,
                            arg2: 0,
                            grants: &[
                                rendezvous_cap(out, Rights::WRITE),
                                rendezvous_cap(src, Rights::READ),
                            ],
                            maps: &[],
                        },
                    ),
                    None => run(
                        image,
                        Spawn {
                            arg0: 0,
                            arg1: arg,
                            arg2: 0,
                            grants: &[rendezvous_cap(out, Rights::WRITE)],
                            maps: &[],
                        },
                    ),
                })
                .is_some()
            }
            None => false,
        };

        // A redirected child's answer goes somewhere else, so the shell has nothing to read and
        // the progenitor owes it an ack. Unredirected, the child's own message is the shell's single read.
        if wiring.sink {
            crate::sched::ipc_send(
                result,
                [
                    if started {
                        grant_plan::spawnproto::SPAWN_OK
                    } else {
                        grant_plan::spawnproto::SPAWN_FAILED
                    },
                    0,
                    0,
                ],
            );
        } else if !started {
            crate::sched::ipc_send(result, [grant_plan::spawnproto::SPAWN_FAILED, 0, 0]);
        }
    }
}

/// Take one delegated capability and read the rendezvous out of it. The slot is dropped straight
/// away: what the progenitor needs is the *name* of the rendezvous, and holding the capability afterwards
/// would fill a capability table over a long session for nothing.
fn take_rendezvous(ep: RendezvousId) -> Option<RendezvousId> {
    let m = crate::sched::ipc_receive_cap(ep);
    let slot = m[1];
    let cap = crate::sched::current_cap(slot).ok()?;
    let _ = crate::sched::delete_current_cap(slot);
    match cap.object {
        crate::cap::Object::Rendezvous(id, _) => Some(id),
        _ => None,
    }
}
