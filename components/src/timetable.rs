//! **The timetable: scheduled execution where every entry is a grant.** A cron whose entries are
//! grant expressions, planned at registration by `grant_plan::plan` and printed before the first
//! tick. Compromising this process yields the entries' summed endowments, not the system.
//!
//! It holds `timetable::contract`'s table and nothing else: the plan page, an untyped budget, the
//! supervision and reaping endpoints, `a1` an archive of exactly the programs the plan builds, and
//! `a2` the registration page of §222 (who holds a user's schedule). No directory, console, network
//! or device, and no clock page unless granted. Store mode (milestone 152's Fork 8 D) holds
//! `activation/` and `packages/` instead. See `notes/scheduled-execution/the-program.md`.

//! # BUGS
//!
//! - It has not adopted milestone 106's timer (milestone 674), so it spins, polls and reaps lazily.
//! - A hung child stops the whole timetable once the budget runs out; nothing kills it.
//! - The archive holds every image the plan builds: code, not authority (see the note).
//! - `--mem` entries run alone, and others due meanwhile run late, once (see the note).
//! - A calendar line fires up to one pass late; `Registry::next_deadline` covers only counter rows.
//! - A replacement's plan is cut at `registration::BODY_MAX`; the verdict word survives. Only the
//!   compiled-in document is audited.
//! - In store mode a skipped beat is recorded nowhere, and a job has nowhere to write output.
//! - In store mode an oversized program's refusal is unnamed, and a fire failure is only `E_BUDGET`.
//!
//! Name: ratified 2026-09-13 (calef), with `crates/timetable` and `components/timetable.conf`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use grant_plan::spawnproto;
use timetable::{Registry, contract, registration};
use user_mode_runtime::{cap_delete, exit, monotonic_nanos, reap, receive_fault, send, yield_now};

/// The document. Compiled in; see `BUGS`.
const CONFIG: &str = include_str!("../timetable.conf");

/// The output endpoint: the plan, and the summary. `byte_sink_contract` bytes. Unused with a
/// registration page; see [`PAGE`].
const OUT: u64 = contract::OUT_SLOT;
/// The budget every instance is made of, and what pays this loader's scratch mappings.
const BUDGET: u64 = contract::BUDGET_SLOT;
/// Handed to each instance as its slot 0, so a scheduled child can report its answer.
const CHILD_REPORT: u64 = contract::CHILD_REPORT_SLOT;
/// Placed in each instance's reserved fault slot, and what corpses are collected through.
const DEATHS: u64 = contract::DEATHS_SLOT;
/// Store mode's `activation/`, read-only (Fork 8 D of milestone 152).
const ACTIVATION: u64 = contract::ACTIVATION_SLOT;
/// Store mode's `packages/`, read-only.
const PACKAGES: u64 = contract::PACKAGES_SLOT;

/// **The registration page's address, or zero**, set once at `_start` from `a2`.
///
/// Nonzero makes this process silent on [`OUT`], because its registrar is a session blocked on
/// supervision that cannot drain a stream, and a `SEND` nobody takes would stop the loop. [`say`]
/// then writes nothing, and [`done`] leaves its code in the page instead (`timetable::contract`).
static PAGE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Pages per instance region. Enough for a small program's segments, its stack, its address-space
/// tables and its TCB, and the same number `components/src/spawner.rs` arrived at for the same job.
///
/// A per-instance region rather than one shared pool is what makes a single `MemoryRegion::DESTROY`
/// reclaim a whole dead child, which is the property the reap depends on.
const INSTANCE_PAGES: u64 = 48;

/// How many times to retry a reap whose region still holds something that can run.
///
/// The same safety net as `components/src/job_undertaker.rs`'s, and the same reasoning: a `NotPermitted`
/// is a fact about the region's other residents rather than about this corpse, one preemption is
/// enough, and running out is still a loud failure because a corpse that never becomes collectable
/// is a leak that ends this process's memory a few fires later with nothing to point at.
const REAP_ATTEMPTS: usize = 1024;

/// Verdict codes on [`OUT`]'s stream, so a spawn site that reads nothing else can still tell what
/// happened. The plan and the summary are text; these are the two ways the program ends.
use contract::{E_ARCHIVE, E_BUDGET, E_CONFIG, E_IMAGE, E_STORE, E_UNVOUCHED};

// The relation `grant_plan` cannot state without depending on `abi`, held here as every reader of
// the slot holds it (`components/src/swish.rs` does the same).
const _: () = assert!(spawnproto::RUN_UNVOUCHED_SLOT == abi::fault::FAULT_EP_SLOT - 1);
// No slot this program hands a job is the run-unvouched slot. The job's other capability is a
// region this program split for it, which cannot be the run-unvouched capability (see `_start`).
const _: () = assert!(CHILD_REPORT != spawnproto::RUN_UNVOUCHED_SLOT);

/// **What an admitted row is built from.**
enum Image {
    /// Parsed out of the archive this timetable was handed, once, at registration.
    Archived(elf::Elf<'static>),
    /// **An installed program** (store mode, Fork 8 D of milestone 152): only the name its line
    /// gave. At every fire the name is resolved in the live generation again and the line planned
    /// again against the manifest those bytes carry, as a bare word at the prompt is (calef's
    /// amendment of 2026-09-27 on #1377). So an upgraded program's job fires the new version, and a
    /// program removed or no longer trusted, which leaves the generation, stops firing.
    Installed(Installed),
}

/// The name an installed row was registered with. See [`Image::Installed`].
struct Installed {
    name: [u8; NAME_MAX],
    len: usize,
}

/// The longest program name store mode resolves; a prompt name is at most sixteen bytes.
const NAME_MAX: usize = 32;

/// The images an admitted row will be built from, one slot per entry.
type Images = [Option<Image>; timetable::MAX_ENTRIES];

/// **Where this timetable's programs come from**: an archive its spawn site narrowed, or the store.
enum Programs {
    Archive(nifefs::Fs<'static>),
    Store,
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(fires_wanted: u64, initial_ramdisk_len: u64, registration_page: u64) -> ! {
    PAGE.store(registration_page, core::sync::atomic::Ordering::Relaxed);
    // A scheduled job never holds the run-unvouched capability, gate D2 of §219 (how the shell
    // names an installed program to the spawner), so §220 (signed builds, and trusting a key is
    // scoped) reaches it. Checked once: this program makes no `RECEIVE_CAP` after `_start`.
    // Every probe here runs before anything is allocated, because a region split later could land
    // in the slot and read as held. See `notes/scheduled-execution/the-program.md`.
    let store = user_mode_runtime::is_granted(ACTIVATION);
    if user_mode_runtime::is_granted(spawnproto::RUN_UNVOUCHED_SLOT) {
        say(
            b"timetable: it holds the run-unvouched capability, which no scheduled job may hold, \
              so it runs nothing\n",
        );
        done(E_UNVOUCHED)
    }
    // Probed now for the same reason: before anything is allocated, an occupied slot 4 can only be
    // the clock page the spawn site placed there (`timetable::contract`).
    let holds_clock = user_mode_runtime::is_granted(contract::CLOCK_SLOT);

    // SAFETY: forwarded from user_mode_runtime::initial_ramdisk::bytes's own contract, the same one
    // `components/src/root_supervisor.rs` is started under. It named `components/src/builder.rs`
    // until milestone 295 retired that program; the contract is unchanged, only the sibling is.
    let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };

    // With a registrar, the timetable starts with nothing and waits to be told.
    let text = if registration_page == 0 { CONFIG } else { "" };
    let doc = match timetable::parse(text) {
        Ok(d) => d,
        // The line number rides in the low byte, so a wrong document is findable from the verdict
        // alone even when nobody is reading the text stream.
        Err(e) => {
            say(b"timetable: the document does not parse: ");
            say(e.message().as_bytes());
            say(b"\n");
            done(E_CONFIG | (e.line() as u64 & 0xff));
        }
    };

    // What this process holds, in the vocabulary registration checks against. The host test uses
    // the same `timetable::SHIPPED_HELD`, so the two cannot drift. A durable session places no
    // report endpoint (Fork 6 C of milestone 152 (durable delegation), ruled 2026-09-27 on #1377).
    let held = timetable::Held {
        report: user_mode_runtime::is_granted(CHILD_REPORT),
        clock: holds_clock,
        ..timetable::SHIPPED_HELD
    };

    let mut reg = Registry::register(&doc, held);

    // The plan, printed before anything fires.
    timetable::write_plan(&reg, &mut say);

    let programs = open_programs(store, registration_page, archive, &reg);
    let mut images = resolve_or_fail(&reg, &programs);

    // The plan and the running share one endpoint, and `system_tests/src/user/timetable_tests.rs`
    // reads to exactly this line.
    say(b"timetable: armed\n");

    reg.arm(monotonic_nanos());

    let mut tally = Tally {
        held,
        holds_clock,
        fired: 0,
        outstanding: 0,
        exits: 0,
        faults: 0,
    };
    tally.run(
        fires_wanted,
        registration_page,
        &mut reg,
        &mut images,
        &programs,
    );

    // Every child is collected before the report, so its counts are complete.
    tally.drain();
    tally.report()
}

/// **Where the programs come from**, refusing a store with no registration page, and the archive
/// audit printed beside the plan when there is no registrar.
fn open_programs(
    store: bool,
    registration_page: u64,
    archive: &'static [u8],
    reg: &Registry<'_>,
) -> Programs {
    if store && registration_page == 0 {
        say(b"timetable: it holds the store and no registration page, so it runs nothing\n");
        done(E_STORE)
    }
    let programs = if store {
        Programs::Store
    } else {
        match nifefs::Fs::parse(archive) {
            Ok(fs) => Programs::Archive(fs),
            Err(_) => done(E_ARCHIVE),
        }
    };

    // What the archive reaches, beside what the plan builds. It measures and cannot enforce: a
    // process cannot narrow its own endowment. Not with a registrar: see `BUGS`.
    if let (0, Programs::Archive(fs)) = (registration_page, &programs) {
        let mut audit = timetable::Audit::of(reg);
        for entry in fs.entries() {
            if let Some(name) = entry.name_str() {
                audit.saw(name);
            }
        }
        audit.write(&mut say);
    }
    programs
}

/// [`resolve`], or the verdict for the first program the archive does not carry.
fn resolve_or_fail(reg: &Registry<'_>, programs: &Programs) -> Images {
    // Resolved now, so a missing program fails at start-up; nothing after this looks anything up.
    match resolve(reg, programs) {
        Ok(images) => images,
        Err(i) => {
            if let Some(e) = reg.rows()[i].endowment() {
                say(b"timetable: no such program in the archive: ");
                say(e.program.name().as_bytes());
                say(b"\n");
            }
            done(E_IMAGE)
        }
    }
}

/// What every fire is built from, and the running counts: children fired, still outstanding,
/// and collected clean or faulted.
struct Tally {
    held: timetable::Held,
    holds_clock: bool,
    fired: u64,
    outstanding: u64,
    exits: u64,
    faults: u64,
}

impl Tally {
    /// The schedule loop: take any replacement document, then fire whatever is due, until
    /// `fires_wanted` fires have happened (forever when it is zero) or the document is emptied.
    fn run(
        &mut self,
        fires_wanted: u64,
        registration_page: u64,
        reg: &mut Registry<'static>,
        images: &mut Images,
        programs: &Programs,
    ) {
        // Which of the two document buffers the registry in force borrows from; see [`DOCUMENTS`].
        let mut current = 0usize;
        let mut answered = 0u64;

        while fires_wanted == 0 || self.fired < fires_wanted {
            if registration_page != 0
                && replace_if_asked(
                    registration_page,
                    &mut answered,
                    &mut current,
                    reg,
                    images,
                    programs,
                    self.held,
                )
            {
                // Emptied: nothing more fires, and what is running finishes below.
                break;
            }
            // The wall clock, once per pass, before anything is due: calendar rows are dormant while it
            // is unknown and re-armed when its generation moves (S3, `Registry::observe`).
            reg.observe(if self.holds_clock {
                wall_reading()
            } else {
                None
            });
            let now = monotonic_nanos();
            let mut any = false;
            while let Some(i) = reg.due(now) {
                any = true;
                // Checked inside the pass: several entries can come due at once, and one fire too
                // many blocks its child forever on a report nobody is left to take.
                if fires_wanted != 0 && self.fired >= fires_wanted {
                    break;
                }
                self.fire_row(reg, images, i);
            }
            if !any {
                // A yield, not a sleep, until milestone 674 (the last yield-spinners sleep on a
                // timer) adopts the `Timer` of milestone 106 (a wait that ends on either the
                // interrupt or the deadline); see `BUGS`.
                yield_now();
            }
        }
    }

    /// Fire row `i`, which has come due, unless it no longer resolves or no longer plans to fire.
    fn fire_row(&mut self, reg: &Registry<'_>, images: &Images, i: usize) {
        let Some(registered) = reg.rows()[i].endowment() else {
            return;
        };
        // An installed program is resolved and planned again now; when either fails the beat is
        // skipped, never retried (`BUGS`).
        let loaded;
        // `declared` is the manifest the job's bytes carry: an installed row's endowment is filed
        // under `grant_plan::IMAGE_ROW`, so nothing may be read off its `prog`.
        let (elf, e, declared) = match images[i].as_ref() {
            None => return,
            Some(Image::Archived(elf)) => (elf, registered, registered.program.manifest()),
            Some(Image::Installed(p)) => {
                let Some((elf, m)) = load_current(p) else {
                    return;
                };
                let timetable::Admission::Fires(e) = reg.plan_at_fire(i, m) else {
                    return;
                };
                loaded = elf;
                (&loaded, e, m)
            }
        };

        if e.memory_grant_pages > 0 {
            let clock = self.holds_clock && declared.clock;
            self.fire_exclusive(elf, e, clock);
            return;
        }

        // A full budget waits for one corpse and tries once more; a second failure is wiring.
        let clock = self.holds_clock && declared.clock;
        if !fire(elf, e.program_argument, self.held.report, clock) {
            if self.outstanding == 0 {
                // Nothing is out to wait for.
                say(b"timetable: the budget cannot back one instance\n");
                done(E_BUDGET)
            }
            collect(&mut self.exits, &mut self.faults);
            self.outstanding -= 1;
            if !fire(elf, e.program_argument, self.held.report, clock) {
                say(b"timetable: the budget cannot back one instance\n");
                done(E_BUDGET)
            }
        }
        self.outstanding += 1;
        self.fired += 1;
    }

    /// Fire a row with a `--mem` grant alone, and collect it before returning.
    fn fire_exclusive(&mut self, elf: &elf::Elf, e: grant_plan::Endowment, clock: bool) {
        // Alone: the next death on `DEATHS` must be this instance's, or it cannot be paired
        // with its grant (`notes/scheduled-execution/mem-entries.md`).
        self.drain();
        let Some(mem_slot) = fire_with_grant(
            elf,
            e.program_argument,
            e.memory_grant_pages,
            self.held.report,
            clock,
        ) else {
            say(b"timetable: the budget cannot back one instance\n");
            done(E_BUDGET)
        };
        self.fired += 1;
        collect_grant(&mut self.exits, &mut self.faults, mem_slot);
    }

    /// Collect every child still outstanding.
    fn drain(&mut self) {
        while self.outstanding > 0 {
            collect(&mut self.exits, &mut self.faults);
            self.outstanding -= 1;
        }
    }

    /// Print the counts and finish.
    fn report(&self) -> ! {
        say(b"timetable: ");
        say_num(self.fired);
        say(b" fires, ");
        say_num(self.exits);
        say(b" clean exits, ");
        say_num(self.faults);
        say(b" faults\n");
        done(0)
    }
}

/// **Resolve every admitted row's program in the archive**, or the index of the first that is not
/// there. Nothing is parsed lazily later: a plan that names a missing program is caught here. A
/// store-mode registry was resolved before it was planned ([`resolve_installed`]), so this answers
/// it with no images, which is right only for the empty document a store-mode timetable starts with.
fn resolve(reg: &Registry<'_>, programs: &Programs) -> Result<Images, usize> {
    let mut images: Images = [const { None }; timetable::MAX_ENTRIES];
    let Programs::Archive(fs) = programs else {
        return Ok(images);
    };
    for (i, row) in reg.rows().iter().enumerate() {
        let Some(e) = row.endowment() else { continue };
        let Some(bytes) = fs.read(e.program.name()) else {
            return Err(i);
        };
        let Ok(elf) = elf::Elf::parse(bytes) else {
            return Err(i);
        };
        images[i] = Some(Image::Archived(elf));
    }
    Ok(images)
}

/// **Resolve every entry of a store-mode document in the live activation generation**, before
/// anything is planned: the manifest each program's bytes carry, which is what its line is planned
/// against, and the image it will be fired from. `Err(i)` at the first entry whose program is not
/// installed, cannot be read, or needs what no timetable endows (`timetable::schedulable`).
fn resolve_installed(
    doc: &timetable::Document<'_>,
) -> Result<([grant_plan::Manifest; timetable::MAX_ENTRIES], Images), usize> {
    let mut manifests = [grant_plan::NO_NOTE_MANIFEST; timetable::MAX_ENTRIES];
    let mut images: Images = [const { None }; timetable::MAX_ENTRIES];
    for (i, entry) in doc.entries().iter().enumerate() {
        let word = grant_plan::parse_run(entry.command).program;
        let (Ok(_), true) = (core::str::from_utf8(word), word.len() <= NAME_MAX) else {
            return Err(i);
        };
        let mut n = [0u8; NAME_MAX];
        n[..word.len()].copy_from_slice(word);
        let p = Installed {
            name: n,
            len: word.len(),
        };
        let Some((_, m)) = load_current(&p) else {
            return Err(i);
        };
        manifests[i] = m;
        images[i] = Some(Image::Installed(p));
    }
    Ok((manifests, images))
}

/// **What an installed row's name runs now**: the live generation's entry for it, staged and
/// parsed, and the manifest its bytes carry. `None` when the name no longer resolves, the bytes do
/// not hash to the entry, or they need what no timetable endows (`timetable::schedulable`).
fn load_current(p: &Installed) -> Option<(elf::Elf<'static>, grant_plan::Manifest)> {
    let name = core::str::from_utf8(&p.name[..p.len]).ok()?;
    let (len, _) = store::load(name)?;
    let elf = elf::Elf::parse(store::staged(len)).ok()?;
    // The note, or `None` for bytes that carry none; a note that cannot be read refuses.
    let declared = match elf
        .note(
            capability_manifest_note::OWNER,
            capability_manifest_note::MANIFEST,
        )
        .ok()?
    {
        None => None,
        Some(d) => Some(capability_manifest_note::decode(d).ok()?),
    };
    // Vouched: the live generation named these bytes, so they get what they declare, exactly as
    // the progenitor endows an installed program run at the prompt. No network: a timetable endows
    // none (`timetable::schedulable`), whatever the owner granted at the prompt.
    let m =
        grant_plan::image_manifest(declared, grant_plan::Vouch::Vouched { network: false }).ok()?;
    timetable::schedulable(&m).then_some((elf, m))
}

/// **The store, read through the two caretakers** store mode holds, over the channel whose page is
/// mapped at `timetable::contract::STORE_PAGE_VA`. Every call is a blocking `CALL`, so the page is
/// this process's between them.
mod store {
    use filesystem_contract::{PAGE, dir, fs};
    use installed_programs::{CURRENT, generation_name, lookup, parse_current};
    use timetable::contract;
    use user_mode_runtime::call;

    use super::{ACTIVATION, PACKAGES};

    /// Where a program's bytes are read to, one at a time. See [`staged`].
    static mut STAGING: [u8; contract::STAGING_BYTES] = [0; contract::STAGING_BYTES];

    /// **The first `len` bytes [`load`] staged.** Valid until the next [`load`], which is the only
    /// writer: this process is one thread, and every caller parses and builds from the slice before
    /// it loads again.
    pub fn staged(len: usize) -> &'static [u8] {
        // SAFETY: as above. `len` came from `load`, which never answers more than the buffer.
        let all: &'static [u8; contract::STAGING_BYTES] = unsafe { &*core::ptr::addr_of!(STAGING) };
        &all[..len]
    }

    fn page() -> &'static mut [u8] {
        // SAFETY: the spawn site maps one read/write page at STORE_PAGE_VA for this process's life
        // (`timetable::contract`), and nothing here holds two borrows of it at once.
        unsafe { core::slice::from_raw_parts_mut(contract::STORE_PAGE_VA as *mut u8, PAGE) }
    }

    /// A request that names something: `name` into the page, then the call.
    fn named(ep: u64, verb: u64, at: u64, name: &[u8], w1: u64) -> i64 {
        if name.len() > PAGE {
            return -1;
        }
        page()[..name.len()].copy_from_slice(name);
        call(ep, fs::req(verb, at, name.len() as u64), w1).0 as i64
    }

    fn close(ep: u64, h: i64) {
        if h >= 0 {
            call(ep, fs::req(fs::CLOSE, h as u64, 0), 0);
        }
    }

    /// The whole of the file `h` into `out`, or `None` if a read fails or it does not fit.
    fn read_into(ep: u64, h: i64, out: &mut [u8]) -> Option<usize> {
        if h < 0 {
            return None;
        }
        let mut done = 0usize;
        loop {
            // One byte past a full buffer, so a file exactly its size is told from a larger one.
            let want = (out.len() - done).clamp(1, PAGE);
            let n = call(ep, fs::req(fs::READ, h as u64, want as u64), done as u64).0 as i64;
            if n < 0 {
                return None;
            }
            if n == 0 {
                return Some(done);
            }
            let n = n as usize;
            if done + n > out.len() {
                return None;
            }
            out[done..done + n].copy_from_slice(&page()[..n]);
            done += n;
        }
    }

    /// The live generation's text into `buf`, and its length.
    fn live_table(buf: &mut [u8; PAGE]) -> Option<usize> {
        let h = named(ACTIVATION, fs::OPEN, fs::ROOT, CURRENT.as_bytes(), 0);
        let mut line = [0u8; 32];
        let n = read_into(ACTIVATION, h, &mut line);
        close(ACTIVATION, h);
        let number = parse_current(core::str::from_utf8(&line[..n?]).ok()?)?;
        let mut digits = [0u8; 10];
        let name = generation_name(number, &mut digits);
        let h = named(ACTIVATION, fs::OPEN, fs::ROOT, name.as_bytes(), 0);
        let n = read_into(ACTIVATION, h, buf);
        close(ACTIVATION, h);
        n
    }

    /// **Stage the bytes a bare `name` runs**: the live generation's entry for it (never an owner's
    /// vouch, `installed_programs::lookup`), read from `packages/<name>/<version>/<program>`, and
    /// answered only if they hash to that entry's digest. `(length, digest)`.
    pub fn load(name: &str) -> Option<(usize, measured_boot::Digest)> {
        let mut table = [0u8; PAGE];
        let n = live_table(&mut table)?;
        let table = core::str::from_utf8(&table[..n]).ok()?;
        // The default row for the name, as a bare word at the prompt runs it (milestone 614 (two
        // installed versions of one program) folded the pointer into `lookup`), under
        // `packages/<name>/<version>/<program>`.
        let entry = lookup(table, name).ok()??;
        let (package, version) = (entry.package, entry.version);
        let walk = dir::READ | dir::DESCEND;
        let d1 = named(PACKAGES, fs::OPENDIR, fs::ROOT, package.as_bytes(), walk);
        let d2 = if d1 >= 0 {
            named(PACKAGES, fs::OPENDIR, d1 as u64, version.as_bytes(), walk)
        } else {
            -1
        };
        let f = if d2 >= 0 {
            named(PACKAGES, fs::OPEN, d2 as u64, entry.program.as_bytes(), 0)
        } else {
            -1
        };
        // SAFETY: see `staged`; this is the one writer, and no slice from it is live across here.
        let buf = unsafe { &mut *core::ptr::addr_of_mut!(STAGING) };
        let len = read_into(PACKAGES, f, buf);
        for h in [f, d2, d1] {
            close(PACKAGES, h);
        }
        let len = len?;
        (measured_boot::sha256(&buf[..len]) == entry.digest).then_some((len, entry.digest))
    }
}

/// **The two buffers a replacement's document is copied into**, alternately.
///
/// A [`Registry`] borrows its document, and the page cannot be it: the reply overwrites the page.
/// The replacement is registered in one buffer while the registry in force borrows the other
/// (§222's all-or-nothing sub-ruling). See `notes/scheduled-execution/the-program.md`.
static mut DOCUMENTS: [[u8; registration::BODY_MAX]; 2] = [[0; registration::BODY_MAX]; 2];

/// **Answer a replacement, if the registrar has asked for one since the last answer.**
///
/// Everything that can refuse happens before anything changes: the length, the text, the parse,
/// the registration and the archive lookup. Only then is the registry in force swapped, armed with
/// [`Registry::arm_after`] so unchanged lines keep their beat, and its plan printed down [`OUT`]
/// and into the page. The reply word is written last, with release ordering, so a registrar that
/// sees it sees everything before it.
///
/// Returns `true` when the replacement was an empty document: the caller stops firing, drains what
/// is running, and exits ([`registration::STATUS_EMPTIED`] says why an idle timetable must not stay).
fn replace_if_asked(
    page: u64,
    answered: &mut u64,
    current: &mut usize,
    reg: &mut Registry<'static>,
    images: &mut Images,
    programs: &Programs,
    held: timetable::Held,
) -> bool {
    use core::sync::atomic::{AtomicU64, Ordering};
    // SAFETY: the spawn site mapped one writable page at `page` for exactly this protocol, and it
    // stays mapped for this process's life. Its first two header words are only ever accessed as
    // atomics, by both sides.
    let request = unsafe { &*((page + registration::REQUEST as u64) as *const AtomicU64) };
    // SAFETY: the same page and the same rule, for the reply word.
    let reply = unsafe { &*((page + registration::REPLY as u64) as *const AtomicU64) };
    // PAIR: the registrar's release store of the request word, after it staged the document.
    let word = request.load(Ordering::Acquire);
    let seq = registration::sequence(word);
    if seq == *answered {
        return false;
    }
    *answered = seq;
    // SAFETY: as above, the whole page is ours to read and write while the registrar waits for the
    // reply word, which is the protocol's one rule for the other side.
    let body =
        unsafe { core::slice::from_raw_parts_mut(page as *mut u8, registration::PAGE_BYTES) };
    // Every path answers in the page first and then speaks down `OUT`, never the other way round.
    let answer = |body: &mut [u8], status: u64, detail: u64, verdicts: u64, plan: usize| {
        body[registration::STATUS..registration::STATUS + 8].copy_from_slice(&status.to_le_bytes());
        body[registration::DETAIL..registration::DETAIL + 8].copy_from_slice(&detail.to_le_bytes());
        body[registration::VERDICTS..registration::VERDICTS + 8]
            .copy_from_slice(&verdicts.to_le_bytes());
        body[registration::PLAN_LEN..registration::PLAN_LEN + 8]
            .copy_from_slice(&(plan as u64).to_le_bytes());
        reply.store(seq, Ordering::Release);
    };

    if registration::operation(word) != registration::REPLACE {
        answer(body, registration::STATUS_UNKNOWN_OPERATION, 0, 0, 0);
        say(b"timetable: a registration asked for something other than a replacement\n");
        return false;
    }
    let len = u64::from_le_bytes(
        body[registration::LEN..registration::LEN + 8]
            .try_into()
            .unwrap(),
    );
    if len as usize > registration::BODY_MAX {
        answer(body, registration::STATUS_MALFORMED, 0, 0, 0);
        say(b"timetable: a replacement longer than the page, refused whole\n");
        return false;
    }
    let len = len as usize;
    let spare = 1 - *current;
    // SAFETY: `spare` is the buffer the registry in force does not borrow (see `DOCUMENTS`). The
    // registry that last borrowed it was replaced, and so dropped, before `current` moved off it.
    let text: &'static [u8] = unsafe {
        let buf = core::ptr::addr_of_mut!(DOCUMENTS[spare]).cast::<u8>();
        core::ptr::copy_nonoverlapping(body[registration::BODY..].as_ptr(), buf, len);
        core::slice::from_raw_parts(buf, len)
    };
    let Ok(text) = core::str::from_utf8(text) else {
        answer(body, registration::STATUS_MALFORMED, 0, 0, 0);
        say(b"timetable: a replacement that is not text, refused whole\n");
        return false;
    };
    let doc = match timetable::parse(text) {
        Ok(d) => d,
        Err(e) => {
            answer(body, registration::STATUS_PARSE, e.line() as u64, 0, 0);
            say(b"timetable: the replacement does not parse, and the schedule in force is unchanged: ");
            say(e.message().as_bytes());
            say(b"\n");
            return false;
        }
    };
    if doc.entries().is_empty() {
        answer(body, registration::STATUS_EMPTIED, 0, 0, 0);
        say(b"timetable: the document is empty, so this timetable exits once its running jobs finish\n");
        return true;
    }
    let resolved = match programs {
        Programs::Archive(_) => {
            let next = Registry::register(&doc, held);
            resolve(&next, programs).map(|images| (next, images))
        }
        Programs::Store => resolve_installed(&doc).map(|(manifests, images)| {
            (Registry::register_installed(&doc, held, &manifests), images)
        }),
    };
    let (mut next, next_images) = match resolved {
        Ok(r) => r,
        Err(i) => {
            answer(body, registration::STATUS_NO_IMAGE, i as u64, 0, 0);
            say(
                b"timetable: the replacement names a program this timetable cannot load, and the \
                  schedule in force is unchanged\n",
            );
            return false;
        }
    };

    // Committed from here: nothing below can refuse.
    let kept = next.arm_after(reg, monotonic_nanos());
    *reg = next;
    *images = next_images;
    *current = spare;

    // The reply goes into the page before anything goes down `OUT`. With a registrar `say` writes
    // nothing, so the page is the whole answer; the order matters only to a reader of both, which
    // must find the reply there by the time it sees the line.
    let mut plan = 0usize;
    timetable::write_plan(reg, &mut |bytes: &[u8]| {
        let room = registration::BODY_MAX - plan;
        let n = bytes.len().min(room);
        body[registration::BODY + plan..registration::BODY + plan + n].copy_from_slice(&bytes[..n]);
        plan += n;
    });
    let verdicts = registration::verdicts(reg, kept);
    answer(body, registration::STATUS_REPLACED, 0, verdicts, plan);
    timetable::write_plan(reg, &mut say);
    say(b"timetable: armed\n");
    false
}

/// Build one instance in its own region and start it with `arg`; `false` if the budget or the
/// build refuses.
///
/// It is endowed only what the printed plan shows: with `report`, the report endpoint at slot 0
/// ([`timetable::Held::report`], false with a registrar); with `clock`, the clock page at slot 1,
/// `READ` only and mapped read-only at `contract::CLOCK_VA`, as the progenitor endows `date`. Its
/// supervision endpoint goes in the reserved fault slot, which `START` reads and clears. Nothing is
/// kept: since §32 (a supervisor may collect a corpse without being able to build one) the region
/// capability is not the reap. See `notes/scheduled-execution/the-program.md`.
fn fire(elf: &elf::Elf, arg: u64, report: bool, clock: bool) -> bool {
    let Ok(region) = supervision_protocol::memory_region_split(BUDGET, INSTANCE_PAGES) else {
        return false;
    };
    let mut placed = [(0, 0, 0); 3];
    let Ok(child) = supervision_protocol::build_child(
        BUDGET,
        region,
        elf,
        &supervision_protocol::ChildEndowment {
            placed: slots(&mut placed, report, clock, None),
            maps: clock_map(clock),
            fault: Some(DEATHS),
            ..supervision_protocol::ChildEndowment::new(supervision_protocol::Retention::Nothing)
        },
    ) else {
        // The child does not exist, so the pages go straight back to the budget.
        supervision_protocol::memory_region_destroy(region);
        return false;
    };
    if !supervision_protocol::start_child(child, 0, arg, 0) {
        supervision_protocol::memory_region_destroy(region);
        return false;
    }
    cap_delete(region);
    true
}

/// **Build and start a `--mem`-backed instance**, the exclusive sibling of [`fire`].
///
/// The grant is carved out of the instance's own region rather than out of [`BUDGET`] directly
/// (`BUGS` says why: it is the only thing that ever pairs a death with a grant), so the region is
/// sized `INSTANCE_PAGES + mem_pages` and the grant is a second split off *that*, leaving
/// `INSTANCE_PAGES` for the instance's own address space, frames, stack and TCB exactly as before.
///
/// Returns the grant's own capability, still held, on success. **This is deliberately not deleted
/// the way [`fire`] deletes `region`**: it is the caller's only way to reclaim the grant later, and
/// the caller is [`collect_grant`], called next and only next by this program's one call site.
fn fire_with_grant(
    elf: &elf::Elf,
    arg: u64,
    mem_pages: u64,
    report: bool,
    clock: bool,
) -> Option<u64> {
    let Ok(region) = supervision_protocol::memory_region_split(BUDGET, INSTANCE_PAGES + mem_pages)
    else {
        return None;
    };
    let Ok(mem_slot) = supervision_protocol::memory_region_split(region, mem_pages) else {
        supervision_protocol::memory_region_destroy(region);
        return None;
    };
    let mut placed = [(0, 0, 0); 3];
    let Ok(child) = supervision_protocol::build_child(
        BUDGET,
        region,
        elf,
        &supervision_protocol::ChildEndowment {
            // Slot 0: the report endpoint, when this timetable hands one out. Slot 1: the clock,
            // when the job gets one. Then the grant, narrowed to WRITE so the child may spend it
            // and not lend it (the same narrowing `system_initializer` gives a shell's `--mem`
            // delegation). Placed by number, so the grant's slot does not depend on slot 0.
            placed: slots(&mut placed, report, clock, Some(mem_slot)),
            maps: clock_map(clock),
            fault: Some(DEATHS),
            ..supervision_protocol::ChildEndowment::new(supervision_protocol::Retention::Nothing)
        },
    ) else {
        // `memory_region_destroy(region)` is owner authority over the whole region, not the supervised
        // reap `collect_grant` uses later: it reclaims `mem_slot` along with everything else here,
        // because nothing has been handed to a child yet for anyone else to still be holding.
        supervision_protocol::memory_region_destroy(region);
        return None;
    };
    if !supervision_protocol::start_child(child, 0, arg, 0) {
        supervision_protocol::memory_region_destroy(region);
        return None;
    }
    cap_delete(region);
    Some(mem_slot)
}

/// **A job's slots, by number** (milestone 152's Fork 6 C met milestone 129's clock at the merge
/// of 2026-10-03 (UTC)): slot 0 the report endpoint when this timetable hands one out, slot 1 the
/// clock page when the job gets one, then a `--mem` grant, `WRITE` only. Written into `out`, which
/// the caller owns, and the filled prefix returned. The progenitor's order.
fn slots(
    out: &mut [(u64, u64, u64); 3],
    report: bool,
    clock: bool,
    grant: Option<u64>,
) -> &[(u64, u64, u64)] {
    let mut n = 0;
    if report {
        out[n] = (0, CHILD_REPORT, abi::rights::WRITE);
        n += 1;
    }
    if clock {
        out[n] = (1, contract::CLOCK_SLOT, abi::rights::READ);
        n += 1;
    }
    if let Some(g) = grant {
        out[n] = (1 + clock as u64, g, abi::rights::WRITE);
        n += 1;
    }
    &out[..n]
}

/// The clock page's mapping for a job that gets one, or none.
fn clock_map(clock: bool) -> &'static [(u64, u64, u64)] {
    const MAP: [(u64, u64, u64); 1] = [(
        contract::CLOCK_VA,
        contract::CLOCK_SLOT,
        abi::address_space::MAP_RO,
    )];
    if clock { &MAP } else { &[] }
}

/// **The wall clock as the registry wants it**, or `None` while the page says the time is unknown.
///
/// The page's offset plus the ambient counter is the wall time (§43 (reading the clock is a page)), taken to whole minutes, which
/// is the grammar's resolution. A `SET` publication is flagged, because S3 clears stamps on one.
fn wall_reading() -> Option<timetable::WallReading> {
    // SAFETY: the spawn site maps the clock page read-only at `contract::CLOCK_VA` whenever it
    // places the capability in `contract::CLOCK_SLOT`, which `_start` probed before calling this.
    let page = unsafe { clock_contract::ClockPage::new(contract::CLOCK_VA) };
    let r = page.read();
    if !clock_contract::state::is_known(r.state) {
        return None;
    }
    let wall = clock_contract::wall_nanos(r.offset_nanos, monotonic_nanos());
    Some(timetable::WallReading {
        minute: (wall / (60 * clock_contract::NANOS_PER_SEC)) as i64,
        generation: r.generation,
        set: r.state == clock_contract::state::SET,
    })
}

/// **Wait for the one instance [`fire_with_grant`] just started, and reclaim its grant.**
///
/// Callable only while that instance is the sole thing outstanding, which the one call site in
/// `_start` guarantees by draining everything else first and firing nothing else until this
/// returns. That is what makes the correlation sound: the next death on [`DEATHS`] cannot be
/// anyone else's, so `mem_slot` is destroyed unconditionally rather than guessed at from a refused
/// reap. Once it is gone, the corpse's region has no live resident left and an ordinary [`reap`]
/// reclaims the rest, the same as [`collect`].
fn collect_grant(exits: &mut u64, faults: &mut u64, mem_slot: u64) {
    let (event, tid, ..) = receive_fault(DEATHS);
    if event == abi::fault::EVENT_EXIT {
        *exits += 1;
    } else {
        *faults += 1;
    }
    // The grant is a live resident of the corpse's own region (`regions::destroy_outcome` refuses
    // a region with one), so it has to go before `reap` can succeed at all; nothing here needs to
    // try `reap` first and fail to learn that, because this call is only ever made about the one
    // instance that was built with a nested grant.
    supervision_protocol::memory_region_destroy(mem_slot);
    for _ in 0..REAP_ATTEMPTS {
        if reap(DEATHS, tid) == 0 {
            return;
        }
        yield_now();
    }
    user_mode_runtime::trap()
}

/// **Block until one child dies, then collect it**, counting whether it finished or crashed.
///
/// The two events are counted apart for the reason DECISIONS §26 delivers both: a crash is a fact
/// about a scheduled job worth reporting, and a clean exit is the normal end of one. A scheduler
/// that reported only a total would hide exactly the number an operator wants.
///
/// The kernel is the only sender on this endpoint (§26 clears the child's fault slot at `START`), so
/// the tid is trustworthy without a badge.
fn collect(exits: &mut u64, faults: &mut u64) {
    let (event, tid, ..) = receive_fault(DEATHS);
    if event == abi::fault::EVENT_EXIT {
        *exits += 1;
    } else {
        *faults += 1;
    }
    for _ in 0..REAP_ATTEMPTS {
        if reap(DEATHS, tid) == 0 {
            return;
        }
        yield_now();
    }
    user_mode_runtime::trap()
}

/// Write bytes down the output endpoint, `byte_sink_contract`-framed. Nothing, with a registration
/// page: see [`PAGE`].
fn say(bytes: &[u8]) {
    if PAGE.load(core::sync::atomic::Ordering::Relaxed) != 0 {
        return;
    }
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(OUT, w0, w1, w2);
        rest = &rest[n..];
    }
}

/// A decimal number down the same stream.
fn say_num(v: u64) {
    let mut digits = [0u8; 20];
    let mut n = 0;
    let mut v = v;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 {
            break;
        }
    }
    let mut out = [0u8; 20];
    for i in 0..n {
        out[i] = digits[n - 1 - i];
    }
    say(&out[..n]);
}

/// End the stream, report the verdict, and stop.
///
/// The `byte_sink_contract` end-of-stream comes first so a reader draining text sees a stream that ended
/// rather than one that stopped, and the verdict word after it so a spawn site reading one word
/// still learns how this went.
///
/// With a registration page the code goes into the page's exit word instead, with release ordering,
/// and nothing is sent: the registrar reads it after it has reaped this process.
fn done(code: u64) -> ! {
    let page = PAGE.load(core::sync::atomic::Ordering::Relaxed);
    if page != 0 {
        // SAFETY: the registration page is mapped writable at `page` for this process's life
        // (`_start`'s `a2`), and its exit word is only ever accessed atomically.
        let word = unsafe {
            &*((page + registration::EXIT as u64) as *const core::sync::atomic::AtomicU64)
        };
        word.store(
            registration::EXITED | code,
            core::sync::atomic::Ordering::Release,
        );
        exit();
    }
    send(OUT, byte_sink_contract::eof(), 0, 0);
    send(OUT, code, 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
