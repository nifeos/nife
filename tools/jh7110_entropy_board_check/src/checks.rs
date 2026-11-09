//! The checks, in the order the runbook numbers them, and the records each one prints.
//!
//! One boot runs them all, from the block's state as the board leaves reset to a capture of
//! 40,960 generations. Each poll is bounded by one second of the `time` CSR, so a block that never
//! answers ends a check with `timeouts` counted rather than a hang. notes/jh7110-entropy-board-check.md
//! lists every record and the pass criterion `results.py` applies to it.
//!
//! Name: provisional.

use device_tree_blob::{DeviceTreeBlob, Region};
use jh7110_entropy::registers::{
    AUTOMATIC_RESEED_AGE_REGISTER, AUTOMATIC_RESEED_REQUESTS_REGISTER,
    BUILD_CONFIGURATION_REGISTER, FEATURES_REGISTER, INTERRUPT_ENABLE_REGISTER,
    INTERRUPT_STATUS_REGISTER, MODE_REGISTER, STATUS_MODE_REGISTER, STATUS_REGISTER,
};
use jh7110_entropy::{
    COMPATIBLES, CONTROL_COMMAND_GENERATE, CONTROL_COMMAND_RESEED, GenerationOutcome,
    INTERRUPT_STATUS_AGE_ALARM, INTERRUPT_STATUS_DOCUMENTED_BITS_MASK,
    INTERRUPT_STATUS_LINEAR_FEEDBACK_SHIFT_REGISTER_LOCKUP, INTERRUPT_STATUS_RANDOM_NUMBER_READY,
    INTERRUPT_STATUS_REQUEST_ALARM, INTERRUPT_STATUS_SEED_DONE, MODE_256_BIT_OUTPUT,
    STATUS_256_BIT_OUTPUT, STATUS_GENERATING, STATUS_LAST_RESEED_MASK, STATUS_LAST_RESEED_SHIFT,
    STATUS_MISSION_MODE, STATUS_NONCE_MODE, STATUS_RESEEDING, STATUS_SEEDED,
    STATUS_SERVICE_REQUEST, generation_outcome, is_status_okay,
};

use crate::console::{Hex, configure, record};
use crate::device::Trng;
use crate::machine::ticks;

/// The record format's version, printed on `begin`; `results.py` refuses another.
const FORMAT: u32 = 1;
/// The crate commit the payload was built against, which `build.sh` passes in.
const CRATE_COMMIT: &str = match option_env!("BOARD_CHECK_CRATE_COMMIT") {
    Some(commit) => commit,
    None => "unknown",
};
/// Trials of each overlap scenario (check 4).
const OVERLAP_TRIALS: u32 = 1000;
/// Samples of each timing (check 6).
const GENERATE_SAMPLES: usize = 1000;
const RESEED_SAMPLES: usize = 100;
/// Generations in the long run (check 8).
const LONG_RUN_GENERATIONS: u32 = 1 << 20;
/// Capture lines (check 9): 40,960 generations of 32 bytes is 1.25 MiB, a quarter more than the
/// mebibyte the checker requires, so a few lines the serial link damages do not sink the check.
const CAPTURE_LINES: u32 = 40_960;
/// How long the age alarm is given to latch (check 7), in seconds.
const AGE_ALARM_SECONDS: u64 = 2;

/// Every `STAT` bit or field the crate names; anything outside it is unnamed.
const STATUS_NAMED: u32 = STATUS_NONCE_MODE
    | STATUS_256_BIT_OUTPUT
    | STATUS_MISSION_MODE
    | STATUS_SEEDED
    | STATUS_LAST_RESEED_MASK
    | STATUS_SERVICE_REQUEST
    | STATUS_GENERATING
    | STATUS_RESEEDING;

/// How one driver-path generation ended.
enum Generated {
    Ready([u8; 32]),
    Refused(GenerationOutcome),
    Timeout,
}

/// The block, the clock, and what every `STAT` read so far has shown.
struct Run {
    trng: Trng,
    hertz: u64,
    status_seen: u32,
    last_reseed_values_seen: u32,
}

impl Run {
    /// Reads `STAT`, folding it into what the run has seen.
    fn status(&mut self) -> u32 {
        let status = self.trng.read(STATUS_REGISTER);
        self.note(status);
        status
    }

    fn note(&mut self, status: u32) {
        self.status_seen |= status;
        self.last_reseed_values_seen |= 1 << last_reseed(status);
    }

    /// A deadline one second from now, in ticks.
    fn deadline(&self) -> u64 {
        ticks() + self.hertz
    }

    /// Waits until `STAT` shows no generation and no reseed in flight, or a second passes.
    fn wait_idle(&mut self) -> bool {
        let deadline = self.deadline();
        while self.status() & (STATUS_GENERATING | STATUS_RESEEDING) != 0 {
            if ticks() > deadline {
                return false;
            }
        }
        true
    }

    /// Acknowledges `SEED_DONE`, asks for a reseed and polls `ISTAT` for `SEED_DONE`: whether it
    /// came, the ticks it took, and how many polls.
    fn reseed(&mut self) -> (bool, u64, u32) {
        self.trng.acknowledge(INTERRUPT_STATUS_SEED_DONE);
        let start = ticks();
        self.trng.command(CONTROL_COMMAND_RESEED);
        let mut polls = 0;
        loop {
            polls += 1;
            if self.trng.read(INTERRUPT_STATUS_REGISTER) & INTERRUPT_STATUS_SEED_DONE != 0 {
                let elapsed = ticks() - start;
                self.trng.acknowledge(INTERRUPT_STATUS_SEED_DONE);
                self.wait_idle();
                return (true, elapsed, polls);
            }
            if ticks() - start > self.hertz {
                return (false, ticks() - start, polls);
            }
        }
    }

    /// One generation the way a driver built on the crate does it: acknowledge `RAND_RDY`, write
    /// `GENERATE`, and hand each `(STAT, ISTAT)` poll to `generation_outcome` until it answers
    /// anything but `NotReady`, then acknowledge `RAND_RDY` after a `Ready`. The words are read
    /// only on a poll whose `ISTAT` shows `RAND_RDY`, since no other poll can decode to `Ready`.
    fn generate(&mut self) -> Generated {
        self.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
        self.trng.command(CONTROL_COMMAND_GENERATE);
        let deadline = self.deadline();
        loop {
            let status = self.status();
            let interrupt_status = self.trng.read(INTERRUPT_STATUS_REGISTER);
            let words = if interrupt_status & INTERRUPT_STATUS_RANDOM_NUMBER_READY != 0 {
                self.trng.random_numbers()
            } else {
                [0; 8]
            };
            match generation_outcome(status, interrupt_status, words) {
                GenerationOutcome::Ready(bytes) => {
                    self.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
                    return Generated::Ready(bytes);
                }
                GenerationOutcome::NotReady if ticks() <= deadline => {}
                GenerationOutcome::NotReady => return Generated::Timeout,
                other => return Generated::Refused(other),
            }
        }
    }

    /// Writes `GENERATE` and polls `ISTAT` alone for `RAND_RDY`, the tightest loop that can time
    /// a generation: the ticks and the polls, or `None` on a timeout. Leaves `RAND_RDY` latched.
    fn timed_generate(&mut self) -> Option<(u64, u32)> {
        let start = ticks();
        self.trng.command(CONTROL_COMMAND_GENERATE);
        let mut polls = 0;
        loop {
            polls += 1;
            if self.trng.read(INTERRUPT_STATUS_REGISTER) & INTERRUPT_STATUS_RANDOM_NUMBER_READY != 0
            {
                return Some((ticks() - start, polls));
            }
            if ticks() - start > self.hertz {
                return None;
            }
        }
    }
}

/// `STAT.LAST_RESEED`'s value.
fn last_reseed(status: u32) -> u32 {
    (status & STATUS_LAST_RESEED_MASK) >> STATUS_LAST_RESEED_SHIFT
}

fn bit(value: u32, mask: u32) -> u32 {
    u32::from(value & mask != 0)
}

fn outcome_name(outcome: &GenerationOutcome) -> &'static str {
    match outcome {
        GenerationOutcome::Ready(_) => "ready",
        GenerationOutcome::Lockup => "lockup",
        GenerationOutcome::NotReady => "not_ready",
        GenerationOutcome::Unseeded => "unseeded",
        GenerationOutcome::WrongOutputWidth => "wrong_output_width",
    }
}

fn generated_name(generated: &Generated) -> &'static str {
    match generated {
        Generated::Ready(_) => "ready",
        Generated::Refused(outcome) => outcome_name(outcome),
        Generated::Timeout => "timeout",
    }
}

/// Prints the minimum, median, 99th percentile and maximum of `samples` under `kind`.
fn distribution(kind: &str, samples: &mut [u32], timeouts: u32, polls_median: u32) {
    samples.sort_unstable();
    let n = samples.len();
    if n == 0 {
        record(format_args!(
            "timing kind={kind} samples=0 timeouts={timeouts}"
        ));
        return;
    }
    record(format_args!(
        "timing kind={kind} samples={n} timeouts={timeouts} minimum={} median={} p99={} maximum={} polls_median={polls_median}",
        samples[0],
        samples[n / 2],
        samples[(n * 99) / 100],
        samples[n - 1],
    ));
}

/// Runs every check on the block the device tree at `device_tree` describes, from hart `hart`.
pub(crate) fn run(hart: u64, device_tree: u64) {
    // SAFETY: `booti` hands the payload the device tree's physical address in `a1`, and the hart
    // runs with paging off, so the pointer is the blob. `from_ptr` checks its magic before trusting
    // its length.
    let tree = unsafe { DeviceTreeBlob::from_ptr(device_tree as *const u8) }.ok();
    let mut hertz_from_tree = 0;
    let mut hertz = 4_000_000;
    if let Some(tree) = &tree {
        let mut regions = [Region { start: 0, size: 0 }; 1];
        if let Ok(1) = tree.node_reg(b"serial@10000000", &mut regions) {
            let shift = match tree.node_prop(b"serial@10000000", b"reg-shift") {
                Ok(Some(&[a, b, c, d])) => u32::from_be_bytes([a, b, c, d]),
                _ => 0,
            };
            configure(regions[0].start, shift);
        }
        match tree.node_prop(b"cpus", b"timebase-frequency") {
            Ok(Some(&[a, b, c, d])) => {
                hertz = u64::from(u32::from_be_bytes([a, b, c, d]));
                hertz_from_tree = 1;
            }
            Ok(Some(&[a, b, c, d, e, f, g, h])) => {
                hertz = u64::from_be_bytes([a, b, c, d, e, f, g, h]);
                hertz_from_tree = 1;
            }
            _ => {}
        }
    }
    let model = u32::from(cfg!(feature = "model"));
    record(format_args!(
        "begin format={FORMAT} model={model} crate_commit={CRATE_COMMIT} hart={hart} device_tree={device_tree:#x} tree_parsed={} timebase_hertz={hertz} timebase_from_tree={hertz_from_tree}",
        u32::from(tree.is_some()),
    ));

    let mut found = None;
    if let Some(tree) = &tree {
        for compatible in COMPATIBLES {
            let mut regions = [Region { start: 0, size: 0 }; 1];
            if let Ok(1) = tree.node_reg_compatible(compatible, &mut regions) {
                let status = tree
                    .node_prop_compatible(compatible, b"status")
                    .ok()
                    .flatten();
                found = Some((regions[0], compatible, is_status_okay(status)));
                break;
            }
        }
    }
    let base = match found {
        Some((region, compatible, okay)) => {
            record(format_args!(
                "window trng={:#x} size={:#x} from_tree=1 compatible={} status_okay={}",
                region.start,
                region.size,
                core::str::from_utf8(compatible).unwrap_or("?"),
                u32::from(okay),
            ));
            region.start
        }
        None if model == 1 => {
            record(format_args!(
                "window trng=0x0 size=0x0 from_tree=0 compatible=model status_okay=1"
            ));
            0
        }
        None => {
            record(format_args!("refused reason=no_trng_node"));
            record(format_args!("end complete=0"));
            return;
        }
    };

    if model == 0 {
        if !clock_plan(hertz) {
            record(format_args!("end complete=0"));
            return;
        }
    } else {
        record(format_args!("clock_plan skipped=model"));
    }

    let mut run = Run {
        trng: Trng::new(base),
        hertz,
        status_seen: 0,
        last_reseed_values_seen: 0,
    };
    reset_state(&mut run);
    bring_up(&mut run);
    first_reseed(&mut run);
    two_generations(&mut run);
    overlap(&mut run);
    timing(&mut run);
    alarms(&mut run);
    long_run(&mut run);
    record(format_args!(
        "status_bits seen={:#010x} unnamed_seen={:#010x} last_reseed_values_seen={:#04x}",
        run.status_seen,
        run.status_seen & !STATUS_NAMED,
        run.last_reseed_values_seen,
    ));
    capture(&mut run);
    record(format_args!("end complete=1"));
}

/// Check 1's first half: ungate the TRNG's clocks and release its reset, in
/// `ENTROPY_BRING_UP`'s order, printing each word before and after. `false` if a step was refused
/// or the reset never read as released within a tenth of a second of `hertz` ticks.
fn clock_plan(hertz: u64) -> bool {
    use jh7110_clock_and_reset::{
        CLOCK_ENABLE, ENTROPY_BRING_UP, SYSTEM_TOP_GROUP, SYSTEM_TOP_GROUP_WINDOW, Step,
        is_deasserted,
    };
    let base = SYSTEM_TOP_GROUP_WINDOW.base_physical_address;
    let mut ok = true;
    for step in ENTROPY_BRING_UP {
        match *step {
            Step::EnableClock(index) => {
                let Some(offset) = SYSTEM_TOP_GROUP.clock_offset(index) else {
                    record(format_args!(
                        "clock_step kind=enable index={index} refused=1"
                    ));
                    ok = false;
                    continue;
                };
                let word = (base + offset) as *mut u32;
                // SAFETY: the STG window both published trees give, reached physically, and an
                // offset `clock_offset` bounded inside it. A read-modify-write of a word that holds
                // this one clock, as the kernel's driver does it.
                let (before, after) = unsafe {
                    let before = core::ptr::read_volatile(word);
                    core::ptr::write_volatile(word, before | CLOCK_ENABLE);
                    (before, core::ptr::read_volatile(word))
                };
                record(format_args!(
                    "clock_step kind=enable index={index} before={before:#010x} after={after:#010x}"
                ));
                ok &= after & CLOCK_ENABLE != 0;
            }
            Step::DeassertReset(id) => {
                let Some(line) = SYSTEM_TOP_GROUP.reset_bit(id) else {
                    record(format_args!("clock_step kind=deassert id={id} refused=1"));
                    ok = false;
                    continue;
                };
                let assert = (base + line.assert_offset) as *mut u32;
                let status = (base + line.status_offset) as *const u32;
                // SAFETY: as above; `reset_bit` bounded both offsets inside the window. The assert
                // word holds 32 resets, so only this line's bit is cleared.
                let (before, status_before) = unsafe {
                    let before = core::ptr::read_volatile(assert);
                    let status_before = core::ptr::read_volatile(status);
                    core::ptr::write_volatile(assert, before & !line.mask);
                    (before, status_before)
                };
                let start = ticks();
                let mut status_after;
                loop {
                    // SAFETY: as above.
                    status_after = unsafe { core::ptr::read_volatile(status) };
                    if is_deasserted(status_after, line.mask) || ticks() - start > hertz / 10 {
                        break;
                    }
                }
                let released = is_deasserted(status_after, line.mask);
                record(format_args!(
                    "clock_step kind=deassert id={id} assert_before={before:#010x} status_before={status_before:#010x} status_after={status_after:#010x} released={}",
                    u32::from(released)
                ));
                ok &= released;
            }
            _ => {
                record(format_args!("clock_step kind=unknown refused=1"));
                ok = false;
            }
        }
    }
    ok
}

/// Checks 1 to 3's evidence from before any write: every register the crate names that a read
/// cannot disturb, as the block leaves reset (or as firmware left it).
fn reset_state(run: &mut Run) {
    let status = run.status();
    let t = &mut run.trng;
    record(format_args!(
        "reset_state stat={status:#010x} istat={:#010x} mode={:#010x} smode={:#010x} ie={:#010x} features={:#010x} bconf={:#010x} auto_rqsts={:#010x} auto_age={:#010x} seeded={} last_reseed={} r256={}",
        t.read(INTERRUPT_STATUS_REGISTER),
        t.read(MODE_REGISTER),
        t.read(STATUS_MODE_REGISTER),
        t.read(INTERRUPT_ENABLE_REGISTER),
        t.read(FEATURES_REGISTER),
        t.read(BUILD_CONFIGURATION_REGISTER),
        t.read(AUTOMATIC_RESEED_REQUESTS_REGISTER),
        t.read(AUTOMATIC_RESEED_AGE_REGISTER),
        bit(status, STATUS_SEEDED),
        last_reseed(status),
        bit(status, STATUS_256_BIT_OUTPUT),
    ));
}

/// Check 2 and check 3's second half: the crate's documented bring-up, then what `ISTAT` still
/// holds after the clear and whether `STAT.R256` reads 1 after the mode write.
fn bring_up(run: &mut Run) {
    let t = &mut run.trng;
    t.write(AUTOMATIC_RESEED_AGE_REGISTER, 0);
    t.write(AUTOMATIC_RESEED_REQUESTS_REGISTER, 0);
    t.acknowledge(INTERRUPT_STATUS_DOCUMENTED_BITS_MASK);
    let residue = t.read(INTERRUPT_STATUS_REGISTER);
    let mode = t.read(MODE_REGISTER) | MODE_256_BIT_OUTPUT;
    t.write(MODE_REGISTER, mode);
    t.write(INTERRUPT_ENABLE_REGISTER, 0);
    let mode_after = t.read(MODE_REGISTER);
    let status = run.status();
    record(format_args!(
        "bring_up istat_residue={residue:#010x} mode={mode_after:#010x} stat={status:#010x} r256={}",
        bit(status, STATUS_256_BIT_OUTPUT),
    ));
}

/// Check 1's reseed: `SEED_DONE` by polling, then `STAT.SEEDED` and `LAST_RESEED`.
fn first_reseed(run: &mut Run) {
    let (done, elapsed, polls) = run.reseed();
    let status = run.status();
    record(format_args!(
        "first_reseed done={} ticks={elapsed} polls={polls} stat={status:#010x} seeded={} last_reseed={}",
        u32::from(done),
        bit(status, STATUS_SEEDED),
        last_reseed(status),
    ));
}

/// Check 1's generations: two driver-path generations, their bytes in full.
fn two_generations(run: &mut Run) {
    for index in 1..=2 {
        let start = ticks();
        let generated = run.generate();
        let elapsed = ticks() - start;
        match &generated {
            Generated::Ready(bytes) => record(format_args!(
                "generation index={index} outcome=ready ticks={elapsed} bytes={}",
                Hex(bytes)
            )),
            other => record(format_args!(
                "generation index={index} outcome={} ticks={elapsed}",
                generated_name(other)
            )),
        }
    }
}

/// Check 4: is `RAND_RDY` ever set while `STAT` reports a generation in flight? A snapshot counts
/// as `together` only when both `STAT` reads around its `ISTAT` read show `RAND_GENERATING`, and as
/// `together_loose` when either does.
fn overlap(run: &mut Run) {
    // Scenario 1: the driver path, with `RAND_RDY` acknowledged before each command.
    let (mut snapshots, mut together, mut loose, mut timeouts) = (0u64, 0u32, 0u32, 0u32);
    for _ in 0..OVERLAP_TRIALS {
        run.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
        run.trng.command(CONTROL_COMMAND_GENERATE);
        let deadline = run.deadline();
        loop {
            let s = run.trng.snapshot();
            run.note(s.status_before);
            run.note(s.status_after);
            snapshots += 1;
            let ready = s.interrupt_status & INTERRUPT_STATUS_RANDOM_NUMBER_READY != 0;
            let before = s.status_before & STATUS_GENERATING != 0;
            let after = s.status_after & STATUS_GENERATING != 0;
            together += u32::from(ready && before && after);
            loose += u32::from(ready && (before || after));
            if ready && !after {
                break;
            }
            if ticks() > deadline {
                timeouts += 1;
                break;
            }
        }
    }
    run.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
    record(format_args!(
        "overlap scenario=acknowledged trials={OVERLAP_TRIALS} snapshots={snapshots} together={together} together_loose={loose} timeouts={timeouts}"
    ));

    // Scenario 2: a second `GENERATE` written while the first's `RAND_RDY` still stands, the case
    // the crate's decoder answers `NotReady`.
    let (mut accepted, mut together, mut loose, mut changed, mut timeouts) = (0u32, 0, 0, 0, 0);
    for _ in 0..OVERLAP_TRIALS {
        if run.timed_generate().is_none() {
            timeouts += 1;
            run.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
            continue;
        }
        run.wait_idle();
        let words_before = run.trng.random_numbers();
        run.trng.command(CONTROL_COMMAND_GENERATE);
        let deadline = run.deadline();
        let mut seen_generating = false;
        let mut quiet = 0u32;
        loop {
            let s = run.trng.snapshot();
            run.note(s.status_before);
            run.note(s.status_after);
            let ready = s.interrupt_status & INTERRUPT_STATUS_RANDOM_NUMBER_READY != 0;
            let before = s.status_before & STATUS_GENERATING != 0;
            let after = s.status_after & STATUS_GENERATING != 0;
            together += u32::from(ready && before && after);
            loose += u32::from(ready && (before || after));
            if before || after {
                seen_generating = true;
                quiet = 0;
            } else {
                quiet += 1;
            }
            // Done once a generation was seen and has ended, or after 64 quiet snapshots in a row
            // with none seen, which is a block that ignored the command.
            if (seen_generating && quiet > 0) || quiet >= 64 || ticks() > deadline {
                break;
            }
        }
        accepted += u32::from(seen_generating);
        changed += u32::from(run.trng.random_numbers() != words_before);
        run.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
    }
    record(format_args!(
        "overlap scenario=stale_ready trials={OVERLAP_TRIALS} accepted={accepted} words_changed={changed} together={together} together_loose={loose} timeouts={timeouts}"
    ));
}

/// Check 6: how long a generation and a reseed take, in ticks of `timebase_hertz`, with the cost
/// of reading the clock itself.
fn timing(run: &mut Run) {
    let mut cheapest = u64::MAX;
    for _ in 0..16 {
        let a = ticks();
        let b = ticks();
        cheapest = cheapest.min(b - a);
    }
    record(format_args!("timing kind=time_read minimum={cheapest}"));

    let mut samples = [0u32; GENERATE_SAMPLES];
    let mut polls = [0u32; GENERATE_SAMPLES];
    let (mut n, mut timeouts) = (0usize, 0u32);
    for _ in 0..GENERATE_SAMPLES {
        run.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
        match run.timed_generate() {
            Some((elapsed, count)) => {
                samples[n] = u32::try_from(elapsed).unwrap_or(u32::MAX);
                polls[n] = count;
                n += 1;
            }
            None => timeouts += 1,
        }
        run.wait_idle();
    }
    run.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
    polls[..n].sort_unstable();
    let polls_median = if n > 0 { polls[n / 2] } else { 0 };
    distribution("generate", &mut samples[..n], timeouts, polls_median);

    let mut samples = [0u32; RESEED_SAMPLES];
    let mut polls = [0u32; RESEED_SAMPLES];
    let (mut n, mut timeouts) = (0usize, 0u32);
    for _ in 0..RESEED_SAMPLES {
        let (done, elapsed, count) = run.reseed();
        if done {
            samples[n] = u32::try_from(elapsed).unwrap_or(u32::MAX);
            polls[n] = count;
            n += 1;
        } else {
            timeouts += 1;
        }
    }
    polls[..n].sort_unstable();
    let polls_median = if n > 0 { polls[n / 2] } else { 0 };
    distribution("reseed", &mut samples[..n], timeouts, polls_median);
}

/// Check 7: whether `ISTAT` bits 2 and 3 latch once `AUTO_AGE` or `AUTO_RQSTS` is nonzero, and
/// what `ISTAT` holds after each clear.
fn alarms(run: &mut Run) {
    // The request-count alarm: one request allowed between reseeds, then four generations.
    run.trng.acknowledge(INTERRUPT_STATUS_DOCUMENTED_BITS_MASK);
    run.trng.write(AUTOMATIC_RESEED_REQUESTS_REGISTER, 1);
    let readback = run.trng.read(AUTOMATIC_RESEED_REQUESTS_REGISTER);
    let mut seen = 0u32;
    let mut last_reseed_values = 0u32;
    for _ in 0..4 {
        let _ = run.timed_generate();
        run.wait_idle();
        seen |= run.trng.read(INTERRUPT_STATUS_REGISTER);
        let status = run.status();
        last_reseed_values |= 1 << last_reseed(status);
        run.trng.acknowledge(INTERRUPT_STATUS_RANDOM_NUMBER_READY);
    }
    run.trng.write(AUTOMATIC_RESEED_REQUESTS_REGISTER, 0);
    run.wait_idle();
    run.trng.acknowledge(INTERRUPT_STATUS_DOCUMENTED_BITS_MASK);
    let residue = run.trng.read(INTERRUPT_STATUS_REGISTER);
    record(format_args!(
        "alarm kind=requests written=1 readback={readback:#x} latched={} istat_seen={seen:#010x} seed_done_seen={} last_reseed_values={last_reseed_values:#04x} istat_residue={residue:#010x}",
        bit(seen, INTERRUPT_STATUS_REQUEST_ALARM),
        bit(seen, INTERRUPT_STATUS_SEED_DONE),
    ));

    // The age alarm: the smallest nonzero age, then up to `AGE_ALARM_SECONDS` of polling.
    run.trng.write(AUTOMATIC_RESEED_AGE_REGISTER, 1);
    let readback = run.trng.read(AUTOMATIC_RESEED_AGE_REGISTER);
    let start = ticks();
    let mut seen = 0u32;
    let mut latched_after = 0u64;
    while ticks() - start < AGE_ALARM_SECONDS * run.hertz {
        let interrupt_status = run.trng.read(INTERRUPT_STATUS_REGISTER);
        seen |= interrupt_status;
        if interrupt_status & INTERRUPT_STATUS_AGE_ALARM != 0 {
            latched_after = ticks() - start;
            break;
        }
    }
    run.trng.write(AUTOMATIC_RESEED_AGE_REGISTER, 0);
    run.wait_idle();
    run.trng.acknowledge(INTERRUPT_STATUS_DOCUMENTED_BITS_MASK);
    let residue = run.trng.read(INTERRUPT_STATUS_REGISTER);
    let status = run.status();
    record(format_args!(
        "alarm kind=age written=1 readback={readback:#x} latched={} latched_after_ticks={latched_after} istat_seen={seen:#010x} istat_residue={residue:#010x} stat={status:#010x}",
        bit(seen, INTERRUPT_STATUS_AGE_ALARM),
    ));
    // An automatic reseed may have run; seed again so the long run starts from a known state.
    let _ = run.reseed();
}

/// Check 8: `LONG_RUN_GENERATIONS` driver-path generations, counting each outcome; on the first
/// `LFSR_LOCKUP`, whether a reseed alone clears the bit and whether the next generation serves.
fn long_run(run: &mut Run) {
    let (mut ready, mut lockups, mut unseeded, mut wrong_width, mut timeouts, mut zero) =
        (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
    let mut first_lockup = false;
    for index in 0..LONG_RUN_GENERATIONS {
        if index % (1 << 17) == 0 {
            record(format_args!("long_run_progress done={index}"));
        }
        match run.generate() {
            Generated::Ready(bytes) => {
                ready += 1;
                zero += u32::from(bytes == [0; 32]);
            }
            Generated::Refused(GenerationOutcome::Lockup) => {
                lockups += 1;
                let interrupt_status = run.trng.read(INTERRUPT_STATUS_REGISTER);
                let (done, _, _) = run.reseed();
                let after_reseed = run.trng.read(INTERRUPT_STATUS_REGISTER);
                run.trng
                    .acknowledge(INTERRUPT_STATUS_LINEAR_FEEDBACK_SHIFT_REGISTER_LOCKUP);
                let after_acknowledge = run.trng.read(INTERRUPT_STATUS_REGISTER);
                if !first_lockup {
                    first_lockup = true;
                    let next = run.generate();
                    record(format_args!(
                        "lockup index={index} istat={interrupt_status:#010x} reseed_done={} bit_after_reseed={} bit_after_acknowledge={} next_outcome={}",
                        u32::from(done),
                        bit(
                            after_reseed,
                            INTERRUPT_STATUS_LINEAR_FEEDBACK_SHIFT_REGISTER_LOCKUP
                        ),
                        bit(
                            after_acknowledge,
                            INTERRUPT_STATUS_LINEAR_FEEDBACK_SHIFT_REGISTER_LOCKUP
                        ),
                        generated_name(&next),
                    ));
                }
            }
            Generated::Refused(GenerationOutcome::Unseeded) => {
                unseeded += 1;
                let _ = run.reseed();
            }
            Generated::Refused(_) => wrong_width += 1,
            Generated::Timeout => {
                timeouts += 1;
                run.wait_idle();
            }
        }
    }
    record(format_args!(
        "long_run generations={LONG_RUN_GENERATIONS} ready={ready} all_zero={zero} lockups={lockups} unseeded={unseeded} wrong_output_width={wrong_width} timeouts={timeouts}"
    ));
}

/// Check 9: `CAPTURE_LINES` driver-path generations, each printed as one checksummed line, for
/// the statistics on the host.
fn capture(run: &mut Run) {
    let mut gaps = 0u32;
    for sequence in 0..CAPTURE_LINES {
        match run.generate() {
            Generated::Ready(bytes) => record(format_args!("capture {sequence} {}", Hex(&bytes))),
            other => {
                gaps += 1;
                record(format_args!(
                    "capture_gap {sequence} outcome={}",
                    generated_name(&other)
                ));
                if let Generated::Refused(_) = other {
                    let _ = run.reseed();
                    run.trng
                        .acknowledge(INTERRUPT_STATUS_LINEAR_FEEDBACK_SHIFT_REGISTER_LOCKUP);
                }
            }
        }
    }
    record(format_args!(
        "capture_end lines={CAPTURE_LINES} gaps={gaps}"
    ));
}
