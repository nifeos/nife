//! **The boot self-test's userspace half**: the checks that need a user program, run by a user
//! program, and the one verdict (milestone 1063 (the boot self-test's spawning checks run from
//! userspace)).
//!
//! The kernel runs the checks of its own internals and hands their tally over in the page
//! `boot_ladder::kernel_half` describes. The progenitor builds this program before the shell, with
//! a budget of its own, the boot's entropy service, the devices the kernel set aside and the three
//! programs it builds as bytes (`boot_ladder::self_test_program`). This program builds each child
//! from that budget and supervises it, writes each check's line into a transcript the progenitor
//! copies to the console, then the verdict (`boot_ladder::SELF_TEST`), and exits. Every check here
//! runs on every architecture, and one the machine has nothing for says `absent` and why. See
//! notes/self-test-checks.md, "The two halves".
//!
//! # BUGS
//!
//! - `budget` and `teardown` bracket the machine's free-frame count, sound only while nothing else
//!   on any core allocates or frees in their window.
//! - The draws in `trng` and `entropy` are calls with no deadline: a service that never answers
//!   holds the boot.
//! - `supervised` checks that the faulting pc lies in the child's code, not which instruction.
//!
//! Name: provisional. Considered `self_test` (says nothing of when it runs) and `boot_checks` (the
//! kernel runs checks too).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet
// tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)): each `[[bin]]` is its
// own crate root with one `_start`, and documenting an OS-facing ABI entry point is not what the
// lint is for.
#![allow(missing_docs)]
#![no_main]

use core::fmt::{self, Write};
use core::sync::atomic::Ordering;

use boot_ladder::kernel_half::{KernelHalf, flag, word};
use boot_ladder::self_test_program::{
    self as contract, entropy_source, header, image, image_virtual_address, message, slot,
};
use boot_ladder::verdict::{CheckLine, Outcome, Tally, check};
use supervision_contract::{ChildEndowment, Retention};
use user_mode_runtime::{
    Received, capability_delete, destroy_memory_region, is_granted, map_page_frame,
    notification_bind, notification_poll, reap, receive_bound, receive_fault,
    retype_into_kernel_object, send, survey, timer_arm, timer_cancel, yield_now,
};

user_mode_runtime::program_entry!(start);

const KERNEL_HALF_VA: u64 = address_space_map::pair_page(contract::KERNEL_HALF_VIRTUAL_ADDRESS);
const TRANSCRIPT_VA: u64 = address_space_map::pair_page(contract::TRANSCRIPT_VIRTUAL_ADDRESS);
const IMAGES_VA: u64 = address_space_map::pair_page(contract::IMAGES_VIRTUAL_ADDRESS);
const DISK_DMA_PEEK_VA: u64 =
    address_space_map::pair_page(contract::DIRECT_MEMORY_ACCESS_PEEK_VIRTUAL_ADDRESS);
const BLOCK_DRIVER_DMA_VA: u64 =
    address_space_map::pair_page(contract::BLOCK_DRIVER_DIRECT_MEMORY_ACCESS_VIRTUAL_ADDRESS);
const UNMAPPED_ADDRESS: u64 = address_space_map::pair_page(contract::UNMAPPED_VIRTUAL_ADDRESS);

/// `kernel_test_subject`'s role that reads its second argument and dies:
/// `fixtures/src/kernel_test_subject.rs`'s `READ_KERNEL`.
const READ_ADDRESS_ROLE: u64 = 1;
/// `block_driver`'s honest role, which reads `motd` and reports its first eight bytes:
/// `components/src/block_driver.rs`'s `VIRTIO_BLK`.
const BLOCK_DRIVER_READ_ROLE: u64 = 3;
/// What `motd` starts with on the archive the runners attach as a disk.
const MOTD_HEAD: &[u8; 8] = b"nife: re";

/// The depleter's budget, in pages.
const DEPLETER_PAGES: u64 = 24;
/// Pages a child's region holds over its image: its stack, its tables and two endpoints.
const CHILD_SLACK_PAGES: u64 = 64;
/// Bytes of the transcript kept for the verdict line, so a long transcript loses a check line and
/// never the verdict.
const VERDICT_RESERVE: usize = 256;
/// Where check lines stop.
const CHECK_LINES_END: usize = contract::TRANSCRIPT_BYTES - VERDICT_RESERVE;

/// **What the progenitor handed over, probed once**, before anything here retypes: a retype lands
/// in the first free slot, and an empty slot is how an absent device reads.
struct Grants {
    entropy: bool,
    trng: bool,
    disk: bool,
}

/// The images, as the header page lays them out.
struct Images {
    entropy_source: u64,
    bytes: [&'static [u8]; image::COUNT],
}

/// A timer and a notification bound to this thread, so a receive can end at a deadline.
struct Deadline {
    timer: u64,
    notification: u64,
}

/// **The transcript and the tally.** Every line goes into the page, and the progenitor hears where
/// it now ends.
struct Report {
    tally: Tally,
    at: usize,
}

/// The transcript page as a `core::fmt::Write`, dropping bytes past `limit`.
struct Page<'a> {
    at: &'a mut usize,
    limit: usize,
}

impl Write for Page<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if *self.at < self.limit {
                // SAFETY: the transcript frame is mapped read/write at `TRANSCRIPT_VA`, one page,
                // and `*self.at` is under `limit`, which is at most `TRANSCRIPT_BYTES`.
                unsafe { core::ptr::write_volatile((TRANSCRIPT_VA as *mut u8).add(*self.at), b) };
                *self.at += 1;
            }
        }
        Ok(())
    }
}

impl Report {
    /// Append `line` and a newline, stopping at `limit`, then tell the progenitor where the
    /// transcript ends. A cut line still ends in a newline.
    fn line(&mut self, limit: usize, line: impl fmt::Display) {
        let mut page = Page {
            at: &mut self.at,
            limit,
        };
        let _ = writeln!(page, "{line}");
        if self.at == limit {
            // SAFETY: as in `Page::write_str`; `limit - 1` is inside the page.
            unsafe { core::ptr::write_volatile((TRANSCRIPT_VA as *mut u8).add(limit - 1), b'\n') };
        }
        send(slot::REPORT, message::LINE, self.at as u64, 0);
    }

    /// Record a check's outcome and write its line.
    fn record(&mut self, (name, index): (&'static str, usize), ok: bool, detail: fmt::Arguments) {
        let outcome = if ok { Outcome::Passed } else { Outcome::Failed };
        self.tally.record(index, outcome);
        self.line(
            CHECK_LINES_END,
            CheckLine {
                name,
                outcome,
                detail,
            },
        );
    }

    /// Record that the machine has nothing for the check to test, and why.
    fn absent(&mut self, (name, index): (&'static str, usize), why: fmt::Arguments) {
        self.tally.record(index, Outcome::Absent);
        self.line(
            CHECK_LINES_END,
            CheckLine {
                name,
                outcome: Outcome::Absent,
                detail: why,
            },
        );
    }
}

/// `check!(name)`: the name and its index in `boot_ladder::SELF_TEST_CHECKS`, a build error when
/// the list does not have it.
macro_rules! check {
    ($name:literal) => {
        ($name, const { check($name) })
    };
}

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let grants = Grants {
        entropy: is_granted(slot::ENTROPY),
        trng: is_granted(slot::RANDOM_NUMBER_GENERATOR),
        disk: [
            slot::DISK_TRANSPORT,
            slot::DISK_INTERRUPT,
            slot::DISK_DIRECT_MEMORY_ACCESS,
        ]
        .iter()
        .all(|&s| is_granted(s)),
    };
    // SAFETY: the progenitor mapped the kernel-half page read-only here, and `word::COUNT` words
    // are far under a page.
    let words = unsafe { core::ptr::read_volatile(KERNEL_HALF_VA as *const [u64; word::COUNT]) };
    // A page without the magic is a kernel that recorded nothing: its checks fail by name below.
    let half = KernelHalf::from_words(&words).unwrap_or_default();
    let images = images();
    let deadline = deadline();
    let mut r = Report {
        tally: half.tally,
        at: 0,
    };

    userspace(&mut r, &half, &images);
    budget(&mut r, &images, deadline.as_ref());
    supervised(&mut r, &images);
    teardown(&mut r, &images);
    blockdev(&mut r, &half, &images, &grants, deadline.as_ref());
    jh7110(&mut r, &half, &grants);
    entropy(&mut r, &half, &images, &grants);

    for name in r.tally.fail_unrecorded() {
        r.line(
            CHECK_LINES_END,
            CheckLine {
                name,
                outcome: Outcome::Failed,
                detail: format_args!("did not run: the kernel's half did not reach this program"),
            },
        );
    }
    let tally = r.tally;
    r.line(contract::TRANSCRIPT_BYTES, tally.verdict());
    send(slot::REPORT, message::DONE, r.at as u64, 0);
    user_mode_runtime::exit()
}

/// The image table the progenitor copied in.
fn images() -> Images {
    // SAFETY: the progenitor copied the header page to `IMAGES_VA` and each image after it at
    // `image_virtual_address`, all mapped in this space and never written again.
    let words = unsafe { core::ptr::read_volatile(IMAGES_VA as *const [u64; header::COUNT]) };
    let mut lengths = [0u64; image::COUNT];
    lengths.copy_from_slice(&words[header::IMAGE_BYTE_COUNTS..]);
    let bytes = core::array::from_fn(|i| {
        // SAFETY: as above; a zero length is an empty slice at a mapped address.
        unsafe {
            core::slice::from_raw_parts(
                image_virtual_address(&lengths, i) as *const u8,
                lengths[i] as usize,
            )
        }
    });
    Images {
        entropy_source: words[header::ENTROPY_SOURCE],
        bytes,
    }
}

/// A timer and a notification bound to this thread, or `None` when they cannot be made, in which
/// case a receive here has no deadline.
fn deadline() -> Option<Deadline> {
    let notification = retype_into_kernel_object(slot::BUDGET, abi::objtype::NOTIFICATION);
    let timer = retype_into_kernel_object(slot::BUDGET, abi::objtype::TIMER);
    if notification < 0 || timer < 0 {
        return None;
    }
    let (timer, notification) = (timer as u64, notification as u64);
    (notification_bind(notification, slot::OWN_THREAD) == 0).then_some(Deadline {
        timer,
        notification,
    })
}

/// The counter's rate, or a gigahertz when this process cannot know it, which only makes a wait
/// longer.
fn frequency() -> u64 {
    user_mode_runtime::cntfrq_checked()
        .filter(|&f| f != 0)
        .unwrap_or(1_000_000_000)
}

/// Counter ticks as microseconds, truncating.
fn microseconds(ticks: u64) -> u64 {
    ticks.saturating_mul(1_000_000) / frequency()
}

/// **Wait up to `seconds` for `done`, yielding between looks.** `true` once `done` held.
fn wait_until(seconds: u64, mut done: impl FnMut() -> bool) -> bool {
    let deadline = user_mode_runtime::now() + seconds * frequency();
    loop {
        if done() {
            return true;
        }
        if user_mode_runtime::now() >= deadline {
            return done();
        }
        yield_now();
    }
}

/// **Receive three words on `ep` within `seconds`**, or `None` at the deadline. With no
/// [`Deadline`], or a timer that will not arm, the receive has none.
fn receive_within(ep: u64, seconds: u64, d: Option<&Deadline>) -> Option<(u64, u64, u64)> {
    let armed = d.filter(|d| {
        let _ = notification_poll(d.notification);
        let at = user_mode_runtime::now() + seconds * frequency();
        timer_arm(d.timer, at, d.notification, 1) >= 0
    });
    let Some(d) = armed else {
        return Some(user_mode_runtime::receive(ep));
    };
    let got = match receive_bound(ep) {
        Received::Message(a, b, c) => Some((a, b, c)),
        Received::Notification(_) => None,
    };
    let _ = timer_cancel(d.timer);
    let _ = notification_poll(d.notification);
    got
}

/// The machine's free-frame count, from the statistics page the progenitor mapped.
fn free_frames() -> u64 {
    use machine_statistics_contract::word::FREE_FRAMES;
    use machine_statistics_contract::{PAGE_VA, counter};
    // SAFETY: the progenitor mapped the machine statistics page read-only at `PAGE_VA`.
    unsafe { counter(PAGE_VA, FREE_FRAMES) }.load(Ordering::Relaxed)
}

/// **One capability a child is given**, in the order its slots number them.
#[derive(Clone, Copy)]
enum Grant {
    /// `WRITE` on the child's report endpoint, which this program receives on.
    Report,
    /// One of this program's capabilities, with these rights.
    Ours(u64, u64),
}

/// **A child this program built and supervises**: its region, its supervision endpoint, its
/// report endpoint and its thread id, all from the one region a reap reclaims.
struct Child {
    region: u64,
    supervisor: u64,
    report: u64,
    tid: u64,
}

impl Child {
    /// Delete the slots that named the child's objects, once its region is gone.
    fn forget(self) {
        for s in [self.supervisor, self.report, self.region] {
            capability_delete(s);
        }
    }

    /// Kill the child, reclaim its region and forget it.
    fn end(self) {
        end_region(self.region);
        self.forget();
    }

    /// The child's run state, from its supervision endpoint; a child the endpoint no longer lists
    /// is dead.
    fn state(&self) -> u64 {
        let (r, tid, state) = survey(self.supervisor, 0);
        if r < 0 || tid != self.tid {
            abi::survey::DEAD
        } else {
            state
        }
    }

    /// **Wait up to `seconds` for the child to die, then take its death message**: event, thread,
    /// pc and address. `None` while it is still alive.
    fn death(&self, seconds: u64) -> Option<Death> {
        if !wait_until(seconds, || self.state() == abi::survey::DEAD) {
            return None;
        }
        // A corpse parked as the endpoint's sender makes this return at once.
        let (event, tid, pc, address, ..) = receive_fault(self.supervisor);
        Some((event, tid, pc, address))
    }

    /// **Collect the dead child**, which reclaims its region; `false`, with the region destroyed
    /// instead, when the reap refused.
    fn collect(self) -> bool {
        let reaped = reap(self.supervisor, self.tid) == 0;
        if !reaped {
            end_region(self.region);
        }
        self.forget();
        reaped
    }
}

/// `DESTROY` a region, which kills whatever still runs in it, retrying while the kill lands.
fn end_region(region: u64) {
    let _ = wait_until(2, || destroy_memory_region(region) == 0);
}

/// **Build `bytes` as a supervised child in a region of its own and start it with `args`.** It gets
/// `grants` in slot order and `maps` mapped; its supervision endpoint sits in its fault slot.
fn spawn(
    bytes: &[u8],
    grants: &[Grant],
    maps: &[(u64, u64, u64)],
    args: [u64; 3],
) -> Result<Child, &'static str> {
    let elf = elf::Elf::parse(bytes).map_err(|_| "its image is not an ELF")?;
    let pages = (bytes.len() as u64).div_ceil(process_builder::PAGE) + CHILD_SLACK_PAGES;
    let region = process_builder::memory_region_split(slot::BUDGET, pages)
        .map_err(|_| "no region for it in this program's budget")?;
    let endpoints = (
        process_builder::retype_obj_from(region, abi::objtype::RENDEZVOUS),
        process_builder::retype_obj_from(region, abi::objtype::RENDEZVOUS),
    );
    let (Ok(supervisor), Ok(report)) = endpoints else {
        end_region(region);
        capability_delete(region);
        return Err("no endpoints in its region");
    };
    let child = Child {
        region,
        supervisor,
        report,
        tid: 0,
    };
    let mut capabilities = [(0u64, 0u64); 4];
    for (to, g) in capabilities.iter_mut().zip(grants) {
        *to = match *g {
            Grant::Report => (report, abi::rights::WRITE),
            Grant::Ours(s, rights) => (s, rights),
        };
    }
    let built = process_builder::build_child(
        slot::BUDGET,
        region,
        &elf,
        &ChildEndowment {
            capabilities: &capabilities[..grants.len().min(capabilities.len())],
            maps,
            fault: Some(supervisor),
            ..ChildEndowment::new(Retention::Nothing)
        },
    );
    let Ok(built) = built else {
        child.end();
        return Err("it could not be built");
    };
    if !process_builder::start_child(built, args[0], args[1], args[2]) {
        child.end();
        return Err("it would not start");
    }
    let (r, tid, _) = survey(supervisor, 0);
    if r < 0 {
        child.end();
        return Err("its supervision endpoint lists no thread");
    }
    Ok(Child { tid, ..child })
}

/// **A program runs at user level, reaches for a kernel address, and is killed; the kernel is
/// not.**
///
/// `kernel_test_subject` reads the kernel's own fault counter, at the address the kernel wrote into
/// its half: a page that exists and is not the program's, so it takes a permission fault. Its death
/// must reach its supervisor naming that address, and its corpse must be collected. The injected
/// build expects another address, so this check fails too.
fn userspace(r: &mut Report, half: &KernelHalf, images: &Images) {
    let bytes = images.bytes[image::KERNEL_TEST_SUBJECT];
    let address = half.word(word::KERNEL_VIRTUAL_ADDRESS);
    if bytes.is_empty() || address == 0 {
        r.record(
            check!("userspace"),
            false,
            format_args!(
                "no kernel_test_subject program or no kernel address, so no user program could run"
            ),
        );
        return;
    }
    let inject = half.has(flag::INJECT);
    let wanted = if inject { address + 8 } else { address };
    let child = match spawn(bytes, &[], &[], [READ_ADDRESS_ROLE, address, 0]) {
        Ok(c) => c,
        Err(why) => {
            r.record(
                check!("userspace"),
                false,
                format_args!("a program at user level could not be spawned: {why}"),
            );
            return;
        }
    };
    let Some((event, _, _, at)) = child.death(2) else {
        child.end();
        r.record(
            check!("userspace"),
            false,
            format_args!("a program at user level read {address:#x}: it was not stopped within 2s"),
        );
        return;
    };
    let faulted = event == abi::fault::EVENT_FAULT;
    let collected = child.collect();
    r.record(
        check!("userspace"),
        faulted && at == wanted && collected,
        format_args!(
            "a program at user level read {address:#x}: {}",
            if inject {
                "failure injected by the self_test_injection feature"
            } else if !faulted {
                "it died, but not of a fault"
            } else if at != wanted {
                "a fault was reported at another address"
            } else if !collected {
                "killed, but its corpse was not collected"
            } else {
                "killed, and the kernel survived"
            }
        ),
    );
}

/// **A process spends a budget it was handed, and the kernel allocates nothing for it.**
///
/// `memory_region_depleter` reports once loaded and again with the pages it mapped, and the
/// machine's free-frame count, read at both, must not move. Then its region is destroyed, which
/// kills it.
fn budget(r: &mut Report, images: &Images, d: Option<&Deadline>) {
    let bytes = images.bytes[image::BUDGET_SPENDER];
    if bytes.is_empty() {
        r.record(
            check!("budget"),
            false,
            format_args!("no memory_region_depleter program"),
        );
        return;
    }
    let Ok(spend) = process_builder::memory_region_split(slot::BUDGET, DEPLETER_PAGES) else {
        r.record(
            check!("budget"),
            false,
            format_args!("no {DEPLETER_PAGES}-page region could be carved"),
        );
        return;
    };
    // The depleter's slot 0 is the region it spends and slot 1 its report endpoint.
    let grants = [Grant::Ours(spend, abi::rights::SPEND_ONLY), Grant::Report];
    let outcome = spawn(bytes, &grants, &[], [0; 3]).map(|child| {
        let ready = receive_within(child.report, 2, d).map(|_| free_frames());
        let done = ready.and_then(|before| {
            receive_within(child.report, 2, d).map(|(mapped, ..)| (before, free_frames(), mapped))
        });
        child.end();
        done
    });
    end_region(spend);
    capability_delete(spend);
    match outcome {
        Err(why) => r.record(
            check!("budget"),
            false,
            format_args!("the process could not be spawned: {why}"),
        ),
        Ok(None) => r.record(
            check!("budget"),
            false,
            format_args!("the process did not report within 2s"),
        ),
        Ok(Some((before, after, mapped))) => r.record(
            check!("budget"),
            mapped > 0 && before == after,
            format_args!(
                "a process mapped {mapped} pages of a {DEPLETER_PAGES}-page region; free frames {before} -> {after}"
            ),
        ),
    }
}

/// A child's death message as its supervisor receives it: the event, the thread id, the pc and the
/// faulting address.
type Death = (u64, u64, u64, u64);

/// **One supervised child that faults, its death received, and its region reaped**: the round
/// `supervised` judges once and `teardown` twice. Returns the death message and the child's thread
/// id, or which step refused.
fn supervised_round(bytes: &[u8]) -> Result<(Death, u64), &'static str> {
    let child = spawn(bytes, &[], &[], [READ_ADDRESS_ROLE, UNMAPPED_ADDRESS, 0])?;
    let tid = child.tid;
    let Some(message) = child.death(2) else {
        child.end();
        return Err("the child did not die within 2s");
    };
    if !child.collect() {
        return Err("the corpse refused to be reaped");
    }
    Ok((message, tid))
}

/// **A faulting user-level thread's death reaches its supervisor as a message**, naming the
/// thread, a pc in its code and the address it read (DECISIONS §26 (the fault endpoint)).
fn supervised(r: &mut Report, images: &Images) {
    let bytes = images.bytes[image::KERNEL_TEST_SUBJECT];
    let Ok(elf) = elf::Elf::parse(bytes) else {
        r.record(
            check!("supervised"),
            false,
            format_args!("no kernel_test_subject program to supervise"),
        );
        return;
    };
    let ((event, tid, pc, address), child) = match supervised_round(bytes) {
        Ok(m) => m,
        Err(why) => {
            r.record(check!("supervised"), false, format_args!("{why}"));
            return;
        }
    };
    let in_code = elf.segments().any(|s| {
        let (start, end) = s.page_range(process_builder::PAGE);
        s.is_executable() && (start..end).contains(&pc)
    });
    let ok =
        event == abi::fault::EVENT_FAULT && tid == child && in_code && address == UNMAPPED_ADDRESS;
    r.record(
        check!("supervised"),
        ok,
        format_args!(
            "thread {child:#x} read {UNMAPPED_ADDRESS:#x}; its supervisor received event {event} tid {tid:#x} pc {pc:#x} ({}) addr {address:#x}, and reaped it",
            if in_code { "in its code" } else { "NOT in its code" },
        ),
    );
}

/// **Tearing a process down returns every frame it cost**: a second round of spawn, fault and
/// reap moves the machine's free-frame count by zero. Two rounds and the second judged, because a
/// first round pays first-use costs that are not leaks.
fn teardown(r: &mut Report, images: &Images) {
    let bytes = images.bytes[image::KERNEL_TEST_SUBJECT];
    let before = free_frames() as i64;
    let first = supervised_round(bytes);
    let after_first = free_frames() as i64;
    let second = supervised_round(bytes);
    let after_second = free_frames() as i64;
    let (first_cost, second_cost) = (before - after_first, after_first - after_second);
    match (first, second) {
        (Ok(_), Ok(_)) => r.record(
            check!("teardown"),
            second_cost == 0,
            format_args!(
                "two rounds of a supervised child spawned, faulted and reaped: the first cost {first_cost} frame(s), the second {second_cost}"
            ),
        ),
        (Err(why), _) | (_, Err(why)) => {
            r.record(check!("teardown"), false, format_args!("a round failed: {why}"));
        }
    }
}

/// **A user-level driver reads a file off a virtio disk, woken by the device's interrupt.**
///
/// `block_driver` gets disk 0's confined transport, its interrupt and a DMA page, parses nifefs
/// itself and reports `motd`'s first eight bytes; nothing else issues a virtio command. Absent on
/// a machine with no virtio disk, which is every boot `boot-check` makes and every board.
fn blockdev(r: &mut Report, half: &KernelHalf, images: &Images, g: &Grants, d: Option<&Deadline>) {
    if !g.disk {
        r.absent(
            check!("blockdev"),
            format_args!("no virtio block device on this machine, over MMIO or PCIe"),
        );
        return;
    }
    let transport = if half.has(flag::DISK_MEMORY_MAPPED) {
        "virtio-mmio"
    } else {
        "virtio-pci"
    };
    let bytes = images.bytes[image::BLOCK_DRIVER];
    if bytes.is_empty() {
        r.record(
            check!("blockdev"),
            false,
            format_args!("a {transport} disk, but no block_driver program"),
        );
        return;
    }
    if !map_page_frame(
        slot::DISK_DIRECT_MEMORY_ACCESS,
        DISK_DMA_PEEK_VA,
        false,
        slot::BUDGET,
    ) {
        r.record(
            check!("blockdev"),
            false,
            format_args!("a {transport} disk, but its DMA page would not map"),
        );
        return;
    }
    let offset = abi::virtio::DIRECT_MEMORY_ACCESS_REGION_BASE_PHYSICAL_ADDRESS_WORD_OFFSET;
    // SAFETY: just mapped read-only, one page; the offset is its last word, which the kernel wrote.
    let physical = unsafe { core::ptr::read_volatile((DISK_DMA_PEEK_VA + offset) as *const u64) };
    // `block_driver`'s slots: 0 its report, 1 the interrupt, 2 the transport.
    let grants = [
        Grant::Report,
        Grant::Ours(slot::DISK_INTERRUPT, abi::rights::READ),
        Grant::Ours(slot::DISK_TRANSPORT, abi::rights::WRITE),
    ];
    let maps = [(
        BLOCK_DRIVER_DMA_VA,
        slot::DISK_DIRECT_MEMORY_ACCESS,
        abi::address_space::MAP_RW,
    )];
    let child = match spawn(bytes, &grants, &maps, [BLOCK_DRIVER_READ_ROLE, physical, 0]) {
        Ok(c) => c,
        Err(why) => {
            r.record(
                check!("blockdev"),
                false,
                format_args!("a {transport} disk, but its driver could not be spawned: {why}"),
            );
            return;
        }
    };
    let reported = receive_within(child.report, 5, d).map(|(w, ..)| w);
    // It exits once it has reported.
    match child.death(2) {
        Some(_) => {
            let _ = child.collect();
        }
        None => child.end(),
    }
    let Some(head) = reported.map(u64::to_le_bytes) else {
        r.record(
            check!("blockdev"),
            false,
            format_args!("a user-level driver on the {transport} disk did not report within 5s"),
        );
        return;
    };
    r.record(
        check!("blockdev"),
        &head == MOTD_HEAD,
        format_args!(
            "a user-level driver read motd off the {transport} disk through its own nifefs parse: {head:?}"
        ),
    );
}

/// **The JH7110's clock controller ungated its TRNG, and the TRNG serves a client, timed.**
///
/// `clock` is the controller's before and after, as the kernel's bring-up recorded them in its
/// half. `trng` is two device generations that differ, drawn here through the confined driver, with
/// the bring-up and the draw timed. Absent on every machine but a JH7110.
fn jh7110(r: &mut Report, half: &KernelHalf, g: &Grants) {
    if !cfg!(target_arch = "riscv64") {
        for name in [check!("clock"), check!("trng")] {
            r.absent(name, format_args!("no JH7110 on this architecture"));
        }
        return;
    }
    if !half.has(flag::RANDOM_NUMBER_GENERATOR_FOUND) {
        let why = "this machine's tree names no JH7110 TRNG (neither starfive,jh7110-trng nor the vendor U-Boot's starfive,trng)";
        r.absent(
            check!("clock"),
            format_args!("{why}, so no clock was ungated"),
        );
        r.absent(check!("trng"), format_args!("{why}"));
        return;
    }
    let registers = half.word(word::RANDOM_NUMBER_GENERATOR_REGISTERS);
    let refusal = if half.has(flag::RANDOM_NUMBER_GENERATOR_PROGRAM_MISSING) {
        Some("the archive has no jh7110_entropy program")
    } else if !half.has(flag::RANDOM_NUMBER_GENERATOR_WIRED) {
        Some("its service would not wire")
    } else {
        None
    };
    if let Some(why) = refusal {
        for name in [check!("clock"), check!("trng")] {
            r.record(
                name,
                false,
                format_args!("JH7110 TRNG at {registers:#x}, but {why}"),
            );
        }
        return;
    }
    clock(r, half);
    if !half.has(flag::RANDOM_NUMBER_GENERATOR_ANSWERED) || !g.trng {
        r.record(
            check!("trng"),
            false,
            format_args!("JH7110 TRNG at {registers:#x}: its driver did not answer within 5s"),
        );
        return;
    }
    let report = half.word(word::RANDOM_NUMBER_GENERATOR_REPORT);
    let status = (half.word(word::RANDOM_NUMBER_GENERATOR_REPORT + 1) >> 32) as u32;
    let d = draw_twice(slot::RANDOM_NUMBER_GENERATOR);
    let bytes = d.first + d.second;
    r.record(
        check!("trng"),
        report == entropy_contract::READY && d.is_good(),
        format_args!(
            "JH7110 TRNG at {registers:#x} (tree says {}): report {report:#x}, STAT {status:#010x} ({}), draws {}/{} bytes, first {:02x}{:02x}{:02x}{:02x}.., differ {}; bring-up {} us, {bytes} bytes in {} us ({} bytes/s)",
            if half.has(flag::RANDOM_NUMBER_GENERATOR_STATUS_OKAY) { "okay" } else { "disabled" },
            mode_note(status, half.has(flag::RANDOM_NUMBER_GENERATOR_256_BIT)),
            d.first,
            d.second,
            d.head[0],
            d.head[1],
            d.head[2],
            d.head[3],
            d.differ,
            microseconds(half.word(word::RANDOM_NUMBER_GENERATOR_BRING_UP_TICKS)),
            microseconds(d.ticks),
            (bytes as u64).saturating_mul(frequency()).checked_div(d.ticks).unwrap_or(0),
        ),
    );
}

/// The clock-and-reset controller's before and after, as the `clock` check.
fn clock(r: &mut Report, half: &KernelHalf) {
    if !half.has(flag::CLOCK_REPORTED) {
        r.record(
            check!("clock"),
            false,
            format_args!(
                "this tree names a TRNG but no clock-and-reset window was mapped, so nothing was ungated"
            ),
        );
        return;
    }
    let (before, after) = (half.word(word::CLOCK_BEFORE), half.word(word::CLOCK_AFTER));
    let released = half.has(flag::CLOCK_RELEASED);
    r.record(
        check!("clock"),
        half.has(flag::CLOCK_RUNNING) && released,
        format_args!(
            "JH7110 STG CRG at {:#x} ({}): clocks {:#010x},{:#010x} -> {:#010x},{:#010x}, reset {} after {} polls{}",
            half.word(word::CLOCK_WINDOW),
            if half.has(flag::CLOCK_FROM_TREE) { "from the tree" } else { "the published constant" },
            before as u32,
            (before >> 32) as u32,
            after as u32,
            (after >> 32) as u32,
            if released { "released" } else { "STILL HELD" },
            half.word(word::CLOCK_POLLS),
            if half.has(flag::CLOCK_ALREADY_UP) { "; the firmware had already done all of it" } else { "" },
        ),
    );
}

/// **Say what the TRNG's `STAT` says about its output width**: in 128-bit mode only `RAND0..RAND3`
/// carry an answer, so 16 of 32 reported bytes would not be device output. An all-zero `STAT` is a
/// register window that answered nothing. See notes/self-test-checks.md.
fn mode_note(status: u32, wide: bool) -> &'static str {
    if status == 0 {
        "the whole status register read zero, so this says nothing about the mode"
    } else if wide {
        "256-bit: all eight RAND words are the answer"
    } else {
        "128-BIT: only RAND0..3 are the answer, so 16 of every 32 bytes are not device output"
    }
}

/// What [`draw_twice`] measured.
struct Draw {
    first: usize,
    second: usize,
    zeros: bool,
    differ: bool,
    head: [u8; 4],
    ticks: u64,
}

impl Draw {
    /// Two full 32-byte draws, not all zeros, and different from each other: one draw proves only
    /// that something was returned, and a stuck register file, a driver serving its buffer twice
    /// and a device that never started all present as a repeat.
    fn is_good(&self) -> bool {
        self.first == 32 && self.second == 32 && !self.zeros && self.differ
    }
}

/// **Draw two 32-byte buffers from the entropy service at `service`, as a client**, timed.
fn draw_twice(service: u64) -> Draw {
    let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
    let start = user_mode_runtime::now();
    let first = user_mode_runtime::entropy::fill(service, &mut a).map_or(0, |()| a.len());
    let second = user_mode_runtime::entropy::fill(service, &mut b).map_or(0, |()| b.len());
    Draw {
        first,
        second,
        zeros: a.iter().all(|&x| x == 0),
        differ: a != b,
        head: [a[0], a[1], a[2], a[3]],
        ticks: user_mode_runtime::now().wrapping_sub(start),
    }
}

/// **The machine's entropy source serves a client random bytes.**
///
/// The JH7110 TRNG where the tree names one, else the boot's entropy service, whichever source the
/// progenitor's service draws from: the CPU's seed instruction (`RDSEED`, `RNDRRS`) or a virtio-rng
/// device. Two 32-byte draws that differ. A machine with no source reports this check absent.
fn entropy(r: &mut Report, half: &KernelHalf, images: &Images, g: &Grants) {
    let (service, source) = if half.has(flag::RANDOM_NUMBER_GENERATOR_FOUND) {
        if !g.trng {
            r.record(
                check!("entropy"),
                false,
                format_args!(
                    "the JH7110 TRNG is named by the tree, but its service did not answer"
                ),
            );
            return;
        }
        (slot::RANDOM_NUMBER_GENERATOR, "the JH7110 TRNG")
    } else if g.entropy {
        let source = match images.entropy_source {
            entropy_source::INSTRUCTION => "the CPU's seed instruction",
            entropy_source::DEVICE => "a virtio-rng device",
            _ => "the boot's entropy service",
        };
        (slot::ENTROPY, source)
    } else {
        r.absent(
            check!("entropy"),
            format_args!("no seed instruction, no JH7110 TRNG and no virtio-rng on this machine"),
        );
        return;
    };
    let d = draw_twice(service);
    r.record(
        check!("entropy"),
        d.is_good(),
        format_args!(
            "{source} served {}+{} bytes to a client, first {:02x}{:02x}{:02x}{:02x}.., the two draws differ: {}",
            d.first, d.second, d.head[0], d.head[1], d.head[2], d.head[3], d.differ,
        ),
    );
}

user_mode_runtime::panic_handler!();
